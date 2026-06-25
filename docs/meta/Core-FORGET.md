# FORGET.md — Feelings-Core 待修复项

> 扫描日期：2026-06-25（全量更新）
> 范围：Rust 代码（src/ 9 模块，49 tests）
> 原则：P0 = 生产命门。P1 = 功能受限。

---

## P0 — 生产命门（7/7 ✅ 全部关闭）

1. ✅ **PBM 持久化** — `PbmStore` + JSON serde，`~/.feelings/pbm/{species}/`，3 tests。

2. ✅ **Session 管理** — `SessionManager<D,S>` 封装 tick/end/breach + 冷启动恢复 + 自动持久化。4 tests。

3. ✅ **安全看门狗** — `SessionWatchdog` 10Hz 重校验。停滞检测 + 能量突变 + 帧窗口溢出 + 防御升级建议 + 自愈衰减。5 tests。

4. ✅ **经线冷却** — `CooldownTracker` 每维度独立 6h 冷却 + 前 5min 严格衰减 0.2 + 线性恢复 + 防御层级渐进降级 + escalate 无间隔限制 + 浮点边界卡槽。11 tests。

5. ✅ **D2/D3 结构性防御** — `GroundingSignal<D>` + D3 主动麻痹锚点 + `SafetyBreach` 遥测枚举。v0.4 已闭合。

6. ✅ **AOT 仿真器** — 6 shape 帧级积分 + `preflight_check`。v0.4 已闭合。

7. ✅ **Species 泛型** — `FeelingTarget` trait + `NeuroEnergyTracker<D,S>` + `Session<D,S>` + `HumanDimension↔PbmDimension` 桥接。v0.4 已闭合。

---

## P1 — 功能受限（0/6）

1. **个人基线收敛跟踪零代码** — PBM 四维偏移的收敛趋势检测未实现。

2. **生理数据预处理管线零代码** — 原始 HRV/EDA/EEG → PBM 输入格式的转换未实现。

3. **设备数据融合零代码** — 多设备生理数据的同步与加权未实现。

4. **锚点置信度 R 动态推算零代码** — HRV/EDA/session_count/threshold_convergence 四因子动态推算。当前 lifecycle.rs 只读配置。

5. **性格锚点 PersonalityAnchor 虚空隐形** — 无 VSA 向量空间。AI 教练无法积累连贯偏差。

6. **f64 浮点与实时确定性冲突** — sigmoidal_scale 的 exp() 在 1ms 帧循环引入不可预测延迟。需定点数 LUT 或多项式逼近。

---

## ADR 设计文档（4 篇新增）

| ADR | 主题 | 状态 |
|---|---|---|
| 014 | 网络帧协议 + FrameCodec + PbmStorage trait + SyncEventSource | 定稿 |
| 015 | 缓存分层——KVCache + Priority-LRU + ThreadLocalCache L1 | 定稿 |
| 016 | 存储全景 + 并发模型 + 自研替代 Redis + DeltaBuffer + channel 背压 | 定稿 |
| 017 | PolyStore——编译期异构存储引擎（7 种 slot × 7 种结构） | 定稿 |

ADR 007 补充：分支预测两种通路（慢通路专注/沉浸 vs 快通路恐惧/惊吓）+ 震慑中间态。

---

## 模块全景

```
src/
├── lib.rs
├── main.rs
├── bridge.rs               PbmDimension↔HumanDimension 桥接
├── config.rs               CoreConfig + validate_dimensions
├── species.rs              FeelingTarget trait + Human/Psittacine/Canine/Feline
├── pbm/
│   ├── mod.rs
│   ├── state.rs            ColdStartGuard/DampingState/DampingMatrix/DefenceLevel
│   ├── convergence.rs      Sigmoidal + d_sensitivity
│   └── persist.rs          PbmStore + PbmSnapshot（JSON 持久化）
├── tracker/
│   ├── mod.rs
│   ├── bucket.rs           NeuroEnergyTracker<D,S> + UserSafetyProfile
│   └── coupling.rs         SafetyBreach + CrossDim + GlobalBucket
├── session/
│   ├── mod.rs
│   ├── lifecycle.rs        Session<D,S> + SessionPhase + SessionConfig
│   ├── manager.rs          SessionManager<D,S> + PbmStorage + 冷启动 + 持久化
│   ├── watchdog.rs         SessionWatchdog 10Hz 重校验 + 自愈衰减
│   ├── cooldown.rs         CooldownTracker 6h 维度冷却 + 渐进恢复
│   ├── grounding.rs        GroundingSignal<D> D3 主动麻痹锚点
│   ├── anchor.rs           PersonalityAnchor 三种预设
│   └── aot.rs              AOT 仿真器 6 shape + preflight_check
├── personalize/
│   ├── mod.rs
│   ├── damping.rs          re-export 占位 → 逻辑已移至 pbm/state.rs
│   └── sigmoidal.rs        re-export 占位 → 逻辑已移至 pbm/convergence.rs
└── dsir/
    ├── mod.rs
    └── route.rs            多设备路由骨架（0%）
```

---

## 编辑记录

```
2026-06-25  P0 全部闭合 (7/7)。P0#1-P0#4 落代码：
            PbmStore 持久化 + SessionManager 生命周期 +
            SessionWatchdog 10Hz 重校验(冷启动+自愈衰减+防御联动) +
            CooldownTracker 6h 冷却(渐进恢复+浮点边界+escalate无间隔)。
            ADR 014-017 定稿（网络帧/缓存/存储+并发/PolyStore）。
            ADR 007 补充（两种通路+震慑中间态）。
            49 tests / clippy 零警告。

2026-06-24  v0.4 架构审计五项 + 泛型重构落地
            P0 5/6/7 闭合 + PBM 桥接 + 时钟单调性守卫。
            26 tests green。

2026-06-24  v0.3 Core 模块化重构 + 架构审计(六条硬伤)
            20 tests / clippy 零警告。

2026-06-05  v0.2 价值观基线 — VALUES-TO-CODE.md

2026-06-01  v0.1 初始扫描。架构规范 100%。代码 0%。
```
