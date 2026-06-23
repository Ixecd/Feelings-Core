# ADR 013: Pipeline + Hook 架构——核心流转 + 可插拔钩子

> 状态：定稿
> 日期：2026-06-15
> [Core] 7 个 Hook 中 5 个归属 Core (low_anchor_cap / leaky_bucket / monotony / cross_dim / cold_start)。2 个归属 Anim (static_safety / oi_smoothing)。Hook 机制本身 (trait/Pipeline/Ctx) 归属 Anim。
> 对应：ADR 011/012 代码落地后的复杂度控制

---

## 动机

### 当前问题

Anim 管线目前是硬编码顺序调用：

```
main.rs:
  lex → parse → typeck → rule::check → safety::check_with_scale → guard::inject → fsir
```

这个模式在过去 12 个 ADR 的迭代中逐步积累了四个结构性问题：

**1. `personalize()` 签名膨胀。** Pass 6 的核心函数当前承载：

```
personalize(fsir, registry, pbm, tracker) → PsirDoc
```

其中 `PbmState` 有 12 个字段（guard / damping_gradients / previous_frozen / cold_start_window / session_label / coeffs / user_cap / pbm_updated_at / defence_level / anchor_confidence / damping_state / current_pbm_values / safety_profile / previous_intensities / monotony_counters / rhythm_template / config），每加一个安全机制就膨胀一次。漏桶、脱敏、跨维度耦合、冷启动、阻尼——五个独立关注点全部挤在同一个函数体里。

**2. `main.rs` 管线硬编码。** 新增一个 Pass 或安全检查需要手动在 `main.rs` 插入调用行。没有统一的注册/禁用机制。测试时想跳过某个 Pass 只能注释代码。

**3. 测试隔离困难。** 想单独测漏桶行为，必须构造完整的 `PbmState` + `Registry` + `FsirDoc` — 即使漏桶完全不需要这些上下文。Hook 系统的回归测试依赖整套管线，无法局部验证。

**4. 钩子逻辑散落。** 漏桶的 `intake_and_verify` 在 `personalize()` 末尾调用，脱敏检测也在 `personalize()` 里，跨维度耦合的 `verify_cross_dimension` 定义在 tracker 上但没人调用它（需要在每帧所有维度 intake 之后手动调用）。实际上 `verify_cross_dimension` 在当前代码路径中从未被执行——因为它需要"所有维度都完成本帧 intake"的信息，而这个信息在单维度逐个 intake 的当前架构中不存在。

### 为什么现在做

011+012 刚落地，代码还热着。现在重构成本最低——拖到 013/014 再加更多机制之后，`personalize()` 会变成不可维护的巨石函数。

---

## 决策

Anim 管线从"硬编码顺序调用"重构为 **"核心流转 + 可插拔钩子"**。

```
┌─────────────────────────────────────────────────────────┐
│                    Pipeline Core                         │
│  .anim → tokens → AST → FSIR → PSIR → DSIR → ESIR       │
│  每一步之间是可注册的 Hook 注入点（PipelineStage）         │
└─────────────────────────────────────────────────────────┘
                           │
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
      AfterParse      AfterTypeCheck   AfterPersonalize
      ──────────      ──────────────   ────────────────
      (空)            rule::check       monotony_detector
                      (StaticSafety)    cross_dim_verify
                                      ────────────────
           ▲               ▲               ▲
           │               │               │
      OnSessionStart   AfterIntensityScale  OnSessionEnd
      ──────────────   ───────────────────  ────────────
      cold_start       low_anchor_cap       pbm_persist
                       leaky_bucket.intake  (待实现)
```

核心思想：
- **核心管线只做数据流转**，不可变——`.anim → tokens → AST → FSIR → PSIR → DSIR → ESIR`
- **每个安全/检测/校准机制 = 一个独立的 PipelineHook**，在自己的注入点注册
- **Hook 列表从 YAML 配置构建**，Session 启动时一次性初始化，运行期不变
- **全部静态组合**——不用 dyn trait，编译期确定所有 hook 调用链，零虚函数开销

---

## PipelineStage 枚举

管道阶段——对应当前 Pass 之间和内部的注入点。

```
PipelineStage:
  AfterParse          词法+语法完成，AST 就绪
  AfterTypeCheck      Pass 1 类型检查完成——静态安全规则在此注入
  AfterIntensityScale Pass 3 强度缩放完成、cap 硬下限已生效——漏桶 intake 在此注入
  AfterPersonalize    Pass 6 PSIR 生成后、return Ok 前——脱敏检测、跨维度耦合在此注入
  OnSessionStart      Session 启动时——冷启动守护
  OnSessionEnd        Session 结束时——PBM 持久化
```

每个阶段有自己的 `Ctx` 子集——只暴露该阶段能访问的数据。不允许跨阶段偷看未就绪的 IR。

---

## PipelineHook trait

```rust
/// 管线钩子——在 PipelineStage 注入点被调用。
///
/// Hook 不拥有状态——有状态 hook（如漏桶）通过 Ctx 的内部可变性访问。
/// Hook 列表在 Session 启动时从 config 一次性构建，运行期不变。
pub trait PipelineHook {
    /// hook 名称——对应 YAML config.hooks.<name>。
    fn name(&self) -> &str;

    /// 注册在哪个阶段。
    fn stage(&self) -> PipelineStage;

    /// 执行 hook。ctx 提供该阶段可读写的全部数据。
    fn run(&self, ctx: &Ctx) -> Result<(), AnimiError>;
}
```

## Ctx

```rust
/// Hook 上下文——只读管线数据 + 有状态 hook 的内部可变性。
///
/// 每个阶段只暴露该阶段已就绪的数据。例如 AfterParse 阶段 ctx.ast 存在但 ctx.fsir 不存在。
pub struct Ctx<'a> {
    pub stage: PipelineStage,

    // 只读管线数据（随阶段逐步就绪）
    pub ast: Option<&'a FeelingSource>,
    pub fsir: Option<&'a FsirDoc>,
    pub psir: Option<&'a PsirDoc>,

    // 只读配置
    pub config: &'a AnimConfig,

    // 有状态 hook 通过 RefCell 实现内部可变性
    // ——不污染 Ctx 的只读语义，不影响其他 hook。
    pub tracker: RefCell<Option<NeuroEnergyTracker>>,

    // 脱敏检测跨帧状态
    pub previous_intensities: RefCell<Option<[u32; 4]>>,
    pub monotony_counters: RefCell<Option<[u32; 4]>>,
}
```

每个 hook 只能读写自己声明的字段——不互相污染。`RefCell` 保证运行时借用检查，编译期不可变语义。

---

## 已有机制的 Hook 化映射

| 当前实现 | Hook 名称 | Stage | 有状态？ |
|---------|----------|-------|:---:|
| `rule::check()` | `static_safety` | AfterTypeCheck | ❌ |
| `safety::low_anchor_cap()` | `low_anchor_cap` | AfterIntensityScale | ❌ |
| `NeuroEnergyTracker.intake_and_verify()` | `leaky_bucket_intake` | AfterIntensityScale | ✅ tracker |
| `monotony_detector` | `monotony` | AfterPersonalize | ✅ counters |
| `verify_cross_dimension()` | `cross_dim_coupling` | AfterPersonalize | ✅ tracker |
| `ColdStartGuard` | `cold_start` | OnSessionStart | ❌ |
| PBM 持久化（待实现） | `pbm_persist` | OnSessionEnd | ❌ |
| `guard::inject()` (oi 平滑) | `oi_smoothing` | AfterIntensityScale | ❌ |

### 有状态 Hook 的处理

`leaky_bucket_intake` / `monotony` / `cross_dim_coupling` 三者共享 tracker 状态。当前架构中它们被拆分到三个 Hook 是因为：
- `leaky_bucket_intake` 在 PSIR 生成前运行（拿到 sigmoidal 缩放后的强度）
- `cross_dim_coupling` 在所有维度都完成 intake 之后运行
- `monotony` 在 PSIR 生成后运行（拿到最终 applied_max）

在 Hook 架构中，它们通过 `ctx.tracker` 共享同一个 `NeuroEnergyTracker` 实例。调用顺序由 PipelineStage 保证：AfterIntensityScale → AfterPersonalize。

---

## YAML 配置映射

```yaml
# configs/default.yaml 新增段
hooks:
  static_safety:      { enabled: true }
  low_anchor_cap:     { enabled: true }
  leaky_bucket_intake: { enabled: true }
  monotony:           { enabled: true }
  cross_dim_coupling: { enabled: true }
  cold_start:         { enabled: true }
  oi_smoothing:       { enabled: true }
```

- 每个 hook 的 `enabled` 独立控制——测试时可只开一个 hook
- 不覆盖现有参数段（`leaky_bucket.*`、`monotony.*` 保留原位）——hooks 段只管启用/禁用
- `#[serde(default)]` 保证向后兼容——不传 hooks 段全部默认启用

---

## 管线构建（main.rs 视角）

重构前：
```rust
die(animi::rule::check(&ast, &registry, &config));
let scaled = die(animi::safety::check_with_scale(&ast, user_cap, &config));
let smoothing = die(animi::guard::inject(&ast, &config));
// ... 每个 Pass 单独调用
```

重构后：
```rust
let pipeline = animi::pipeline::Pipeline::build(&config)?;
let ctx = Ctx::new(&ast, &config);
pipeline.run_stage(PipelineStage::AfterTypeCheck, &ctx)?;
// build() 内部从 config.hooks 读取启用列表，按 stage 分组
// run_stage() 遍历该 stage 的所有已启用 hook 并顺序执行
```

`Pipeline::build()` 做的事：
1. 读 `config.hooks`
2. 对每个 `enabled: true` 的 hook 实例化对应的 struct
3. 按 `PipelineStage` 分组——得到一个 `HashMap<PipelineStage, Vec<Box<dyn PipelineHook>>>`（或静态组合的等效结构）
4. 返回不可变的 Pipeline 实例——整个 Session 生命周期内不变

---

## 和现有 Config 系统的咬合

- `AnimConfig` 新增 `hooks: HooksConfig` 字段，`#[serde(default)]` 保证向后兼容
- 现有 `configs/default.yaml` 的参数保持原位——只新增顶层 `hooks:` 段
- `hooks` 段不影响现有 `leaky_bucket.*` / `monotony.*` / `caps.*` 等参数段——它们是不同层级的配置：
  - `hooks.leaky_bucket.enabled` = 这个 hook 是否启用
  - `leaky_bucket.max_dt_seconds` = 如果启用，参数是什么

---

## 不做的事

- **不引入 dyn trait 动态分发**——全部静态组合。`Pipeline::build()` 在启动时构建 hook 列表，之后不做动态派发。每个 hook struct 是编译期已知的具体类型。
- **不引入第三方框架**——不依赖任何 DI/IoC 容器、不依赖任何 macro 扩展、不用 `inventory` 或 `linkme` 做自动注册。
- **不搞热加载**——hook 列表在 Session 启动时从 config 一次性构建，运行期不可变。
- **不改管线数据流的语义**——`.anim → tokens → AST → FSIR → PSIR → DSIR → ESIR` 的顺序不变。Hook 插在它们之间，不是替代它们。
- **不追求"每个 hook 独立编译"**——所有 hook 在同一个 crate 内编译。代码隔离依赖命名约定和 code review，不依赖模块边界。

---

## 和 Pass 编号的咬合

当前的 Pass 编号（0a/0b/1/2/3/4/5/6/7/8）保持不变。Hook 不是新的 Pass——Hook 是 Pass 之间和内部的横切关注点。编号体系不因 Hook 架构而改变：

```
Pass 0a-0b  lex + parse           ← 核心流转
    │
    ├── PipelineStage::AfterParse
    │
Pass 1      typeck                ← 核心流转
    │
    ├── PipelineStage::AfterTypeCheck   → static_safety hook
    │
Pass 2      (已被 rule::check 替代——rule 从 Pass 降为 hook)
Pass 3      safety::check_with_scale   ← 核心流转
    │
    ├── PipelineStage::AfterIntensityScale → low_anchor_cap / leaky_bucket / oi_smoothing hooks
    │
Pass 4      guard::inject（降为 hook）
Pass 5      fsir                   ← 核心流转
    │
Pass 6      personalize            ← 核心流转
    │
    ├── PipelineStage::AfterPersonalize  → monotony / cross_dim_coupling hooks
    │
Pass 7      devicemap              ← 核心流转
Pass 8      codegen                ← 核心流转
```

Pass 2 (rule) 和 Pass 4 (guard) 从独立的 Pass 降为 hook。Pass 编号 2 和 4 保留为"预留位"——不再跳号。

---

## 实现 — v1.2 已落地（2026-06-15）

- `src/pipeline.rs` — `PipelineStage` 六阶段 / `Ctx` / `PipelineHook` trait / `Pipeline` + 7 个 hook 全部实现
  · `static_safety` / `low_anchor_cap` / `oi_smoothing` / `leaky_bucket_intake` / `monotony` / `cross_dim_coupling` / `cold_start`
- `main.rs` 管线改用 Pipeline — `Pipeline::build(&config)` + `run_stage(AfterTypeCheck)` / `run_stage(AfterIntensityScale)`
- `configs/default.yaml` hooks 段 — 每个 hook 独立 `enabled` 开关
- `AnimConfig` 新增 `hooks: HooksConfig` 字段
- 全部静态组合，零 `dyn` 开销。137 测试全绿。

## 和已有文档的咬合

```
本文                                    ADR 013——Pipeline + Hook 架构设计
docs/design/003-pass-pipeline.md         九 Pass 交织管线——Hook 插在 Pass 之间，不替代 Pass
docs/design/011-neuro-leaky-bucket.md    ADR 011——漏桶 → leaky_bucket_intake hook
docs/design/012-signal-rhythm-and-dynamic-cap.md  ADR 012——脱敏 → monotony hook
configs/default.yaml                     hooks 段——每个 hook 的 enabled 开关
src/safety.rs                            NeuroEnergyTracker / UserSafetyProfile ——通过 ctx.tracker 暴露
src/personalize.rs                       personalize() ——Hook 化后不再直接调用 tracker/intake/verify
```

---

*管线是经线。Hook 是纬线。只在一帧交叉。交叉之后各自继续。*
