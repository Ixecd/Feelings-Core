# ADR 011: 时域能量积分与生物漏桶安全——Neuro-Leaky Bucket

> 状态：定稿
> 日期：2026-06-12
> [Core] 全文归属 Feelings-Core。四维漏桶需 Session 级状态 + 个人 Profile 参数。Core 未出生前由 Anim safety.rs 暂代。

---

## 动机

Anim 的单帧安全体系——Pass 2/3 的静态 cap——只能拦截"超限的一帧"。但持续数千帧低强度合法信号——每帧都在 cap 内——受体持续饱和——神经递质无法完成重摄取——长期强度耗竭风险。这就像长期专注一件事——第1天强烈充实——第100天——同样的专注——不再带来满足——等控器在满足太久之后——阈值上移——需要更深的冲击才能重新感知存在。离散型重复（每帧合法→逐帧叠加）和连续型攀升（信号持续不断→不间断饱和）——两种模式都绕过单帧上限——需要在时间窗口上追踪累积能量。

Anim 的安全不能只靠单帧上限。需要在时间窗口上追踪累积能量——当累积超过人体生理耐受阈值时——触发强制保护帧。

---

## 决策

引入**生物漏桶算法（Neuro-Leaky Bucket）**——替代传统滑动窗。每一帧的强度叠加到漏桶——漏桶持续泄漏（模拟神经递质重摄取/代谢清除）——累积能量超过临界阈值→熔断→触发 Pass 7 强制保护帧。

### 核心公式

```
Energy_t = max(0, Energy_{t-1} + Intensity_t - LeakRate × Δt)
```

| 参数 | 含义 | 单位 |
|------|------|------|
| Energy_t | 当前帧累积能量 | (0 ~ CriticalThreshold) |
| Energy_{t-1} | 上一帧累积能量 | |
| Intensity_t | 当前帧实际输出强度（Pass 3 缩放后） | 强度分 (0-100) |
| LeakRate | 用户神经恢复基线——每秒自然代谢的能量 | 强度分/秒 |
| Δt | 帧间隔 | 秒（1ms 帧 = 0.001s） |

### 漏桶 vs 滑动窗

```
滑动窗                           漏桶
──────                           ──

第 1 秒伤害 = 第 30 秒伤害        静默帧→能量自然下降——模拟代谢
跨过窗口边界→伤害骤清零            没有窗口边界——没有跳变
人体不是这样代谢                  人体正是这样代谢的
```

### 非线性泄漏——防 PWM 脉宽调制攻击

线性漏桶有一个致命弱点：在阈值边缘——攻击者可通过精心调制的脉冲（高强度 N 帧→归零释放 M 帧→高强度 N 帧→……）在 CriticalThreshold 附近形成高频锯齿拉锯——永不触发熔断——但用户的神经通路持续处于准饱和状态。

```
PWM 攻击路径：
   强度 80——推高能量到 ~95% 阈值
   强度 0——漏掉——降回 ~85%
   强度 80——再推高——到 ~95%
   → 反复。永不熔断。持续准饱和。
```

这不是理论攻击——这和神经突触的非线性重摄取同构。真实神经递质的清除不是线性——是近似指数衰减——dE/dt = -kE——浓度越高——清除越快。

修正：将 LeakRate 从常数改为**能量相关的非线性函数**——能量越高——漏得越快——越难在阈值边缘维持不动。

```
LeakRate(E) = LeakRate_baseline × (1.0 + γ × E)

E      = Energy_t / CriticalThreshold （归一化能量 0~1）
γ      = 非线性系数（默认 1.0）
```

| E | LeakRate 倍数 | 效果 |
|---|-------------|------|
| 0.0 | 1.0× | 静默状态——基线泄漏 |
| 0.5 | 1.5× | 半饱和——加速泄漏——主动防御 |
| 0.9 | 1.9× | 临界态——几乎 2 倍泄漏——很难 PK 边缘 |
| 1.0 | 2.0× | 饱和自动溢出——天然越过阈值 |


### 时钟挂起——max_dt_limit 防虚假清零

公式强依赖 Δt。如果系统因 GC、I/O 阻塞或宿主机调度延迟导致主线程挂起——恢复时 Δt 暴增——`LeakRate × Δt` 极大——`max(0, ...)` 将累积能量瞬间清零。但用户在这段阻塞期间——神经通路并未因软件卡住而自动复原——如果底层的硬件缓冲仍在继续输出——防御将进入了真空状态。

```
修正：引入 max_dt_limit。如果 Δt > 100ms——
   不执行大跨度泄漏——保留上一帧能量——
   或触安保持——"不知道这段真空期你经受了什么，先别动。" 
```

### 定点数精度——防高帧率截断和时钟抖动

```
LeakRate × Δt 在高帧率（< 100μs）下——如果 Energy 用 u32 定点数——
单帧泄漏量可能因四舍五入被截断为零。
漏桶"只进不出"——正常安全信号被误判成累积超标。

时钟抖动——Δt 趋近零——Intensity_t 重复堆叠——泄漏来不及扣——误熔断。

修正：
  Energy 使用 u64 或 f64——LeakRate 放大至少 10^6 倍——
  确保单帧微小的泄漏量在时域上能被正确保留。
  Intensity_t 同样被归一化到当前帧步长时间戳——
  或由 Feelings-OS busd 在总线层对帧率做硬件锁频——
  不靠"希望 Δt 正确"——靠"不允许 Δt 不合法"。
```

### 绝对不应期保护窗——饱和度 > 90% 时禁止继续累加

当前的 max(0, ...) 只防能量跌破零——没有防"神经元在饱和之后需要一段绝对不应期才能恢复"。如果连续高频能量在漏桶已接近 CriticalThreshold 时依然保持低压补帧——能量可能维持在 ~90% 以上持续数千帧——神经元被锁在准饱和态——无法进入不应期——无法完成胞吞/去敏后的复位。

```
修正：当任一维度的 Energy > 0.9 × CriticalThreshold 时——
  强制启动不应期保护窗——
  该维度的 Intensity_t 不再叠加进漏桶——改为取零。
  保护窗持续 N 帧（默认 500 帧——500ms）——
  给神经通路预留绝对不应期的物理关闭窗口——
  直到能量低于 0.5 × CriticalThreshold 时自动退出。
  N 由 Core 根据创伤分型和 R 动态调节。
```

## 数学定义

### 1. 四维独立漏桶

不同神经通路代谢速度不同。不是全局一个 LeakRate。

```
d ∈ {Visceral, Emotional, Tactile, Auditory}

Energy_{t,d} = max(0.0, Energy_{t-1,d} + Intensity_{t,d} - LeakRate_d × Δt)
```

内脏通路——恢复慢——LeakRate 低。情绪通路——恢复快——LeakRate 高。触觉和听觉居中。四维独立累积——互不污染——一个维度熔断不影响其他维度继续。

### 2. Profile 驱动的动态参数矩阵

LeakRate 和 CriticalThreshold 不是静态常量。由用户安全档案动态决定。

```
Profile             LeakRate_d         CriticalThreshold_d     语义
────────────────────────────────────────────────────────────────────────
标准健康             基线速度            高置信度充裕值            允许高频连击——正常代谢
                    如 2.0/s            如 600

低锚 R < 0.3         减半                 极低                      脆弱神经元——极易堆积——
                     如 1.0/s             如 120                    轻微连击即熔断

Trauma v1 敏感源      近零                  崩塌级                    对该维度几乎丧失代谢能力——
                     如 0.05/s            如 30                      连续点缀刺激→立即强制保护帧

Trauma v2             减半                 低                         部分恢复能力——阈值为标准1/3
                     如 1.0/s             如 200

Trauma v3             近零                  极低                      几乎无代谢——任何连续刺激熔断
                     如 0.05/s            如 60
```

参数由 Feelings-Core PBM 提供——Anim 只做执行。和 `adulthood-as-anchor.md` 的 R 分段完全咬合。

### 2b. 跨维度耦合——维度轮询攻击防御

四维独立漏桶有一个结构弱点：维度轮询攻击——听觉 1000 帧→触觉 1000 帧→情绪 1000 帧→内脏 1000 帧。每个维度的漏桶各自漏水——互不知对方在挨打——永远不触发单维度阈值——但用户的自主神经已经被轮番轰了几千帧——垮了。

生物学上——中枢神经耐受力是系统性的——情感耗竭会直接压低躯体和听觉的耐受阈值。不能把四个维度的等控器当成完全独立的管道。

```
修正：全局耦合系数 σ 。

任一维度能量升高 → 按比例压低其他三个维度的 CriticalThreshold。
Energy_{t,d} 每上升 10% →
    其他维度 EffectiveThreshold_{t,j} = CriticalThreshold_j × (1.0 - σ × E_d)
    σ = 跨维度耦合系数（默认 0.3）
```

不是撤销四维独立性——是让它们感知彼此在集体被推这一事实。

### 2c. 全局总耦合能耗漏桶——防交替震荡攻击

σ 耦合系数只压低其他维度的 CriticalThreshold——但如果攻击者在内脏和情绪之间精确交替（内脏 500 帧→情绪 500 帧→内脏 500 帧…），每个维度的独立漏桶在冷却期各自漏水——σ 压低的阈值仍高于各自的当前值——四个单通道全部合法——但用户的自主神经已经被轮番轰了几万帧——垮了。

```
修正：在四维独立漏桶之上——加一个全局总耦合能耗漏桶（Global Combined Bucket）。

Energy_global = α × Σ(Energy_d)，d ∈ {Visceral, Emotional, Tactile, Auditory}
  α = 全局耦合系数（默认 0.6）。

Global Threshold = Σ(CriticalThreshold_d) × β
  β = 全局容忍系数（默认 0.7）。

任一维度能量升高 → 全局桶同步累积。
全局桶 > Global Threshold → 触发熔断——不管单通道是否合法。

不是检查"哪个维度挨打了"——是检查——
  "这台身体——在被总量推到哪了"。

α 和 β 冷启动 ——
  这两个系数目前完全是设计估值——没有临床数据。
  如果 α 偏高——全局桶过早熔断——正常跨维度体验被频繁打断。
  如果偏低——轮询攻击仍然能绕过。
  前 20-30 个 Session 的用户体验——取决于这两个数字打得多准。
  修正：α 从保守端 0.3 起步——β 从保守端 0.9 起步——
  每 Session 结束后由本地闭环自适应规则缓慢上调——
  不一开始就用 0.6/0.7 这两个中间值去赌。
  保守起步——逐渐放开——比激进起步再回调——安全得多。

冷启动收敛的局部最优陷阱 ——
  前几次 Session 的跨维度耦合事件日志会驱动 α/β 的增量修正——
  如果前几次 Session 碰巧是极端场景（如 Trauma v3 用户首次佩戴的强应激反应）——
  α 可能被过早推高——全局桶从此长期过度敏感——后续正常体验被频繁打断。
  或者——前几次 Session 都是低强度安静场景——α 被压得过低——
  轮询攻击仍然能绕过——直到足够多的异常事件积累才触发修正。
  这是所有自适应算法共有的冷启动局部最优陷阱——不因为参数保守就能完全消除。
  缓解：前 10 个 Session 的 λ（学习率）进一步压低至 0.02——
  让收敛曲线更平——不给前几次 Session 的极值事件过多权重。
  但不完全能消除——在真实数据回来之前——这仍旧是系统性冷启动风险。
```

### 3. 熔断条件

```
∀ d: Energy_{t,d} ≤ CriticalThreshold_d(profile)

任一维度 Energy_{t,d} > CriticalThreshold_d
    → 抛出 AnimiError::SafetyBreach
    → 触发 Pass 7 强制保护帧
```

### 4. Pass 7 强制保护帧——指数衰减包络

熔断后——不准跳变。用指数衰减包络在 3-5s 内引导回安全基线。

```
FlushIntensity(τ) = Cap × e^(-λ × τ)

τ    = 熔断发生后累积干预时间
λ    = 强制平复衰减系数（默认 1.0）
Cap  = 当前帧强度上限
```

和已有 OiSmoothing 四帧衰减同方向——但漏桶熔断后用指数衰减——更贴合自主神经的自然消退曲线——不给等控器第二个惊恐阶跃。

第一阶段（0-3s）：指数衰减——Cap→Cap×e^(-3λ)→约 5% 峰值。
第二阶段（3-5s）：维持最低安全基线——迷走神经最低张力+CT 纤维基础触觉基线。
第三阶段（5s+）：缓慢爬坡恢复——每帧+1 强度分——30 帧后回到 ~30——然后 Feelings-Core 重新接管。

---

## 架构注入点

```
Pass 6 personalize()
    ↓ PSIR 生成完成
    ↓ tracker.intake_and_verify(intensity, dimension, profile)
    ├── Ok → return Ok(psir)
    └── Err(SafetyBreach) → 触发熔断
            ↓
            Pass 7 CodeGen / Feelings-OS busd
            读取熔断标志
            ↓
            注入指数衰减保护帧——3-5s 内引导回安全基线
```

### NeuroEnergyTracker 结构

```rust
// src/safety/tracker.rs

pub struct NeuroEnergyTracker {
    /// 四维累积能量（独立漏桶）
    cumulative_energy: HashMap<PbmDimension, f64>,
    /// 上一帧时间戳（用于 Δt 计算）
    last_tick_ns: u64,
}

impl NeuroEnergyTracker {
    /// 每帧调用——intake 当前帧强度 + 应用漏桶泄漏 + 校验阈值
    pub fn intake_and_verify(
        &mut self,
        intensity: u32,
        dim: PbmDimension,
        profile: &UserSafetyProfile,
    ) -> Result<(), AnimiError> {
        let now = monotonic_ns();
        let dt = (now - self.last_tick_ns) as f64 / 1_000_000_000.0;
        self.last_tick_ns = now;

        let leak = profile.leak_rate(dim);
        let threshold = profile.critical_threshold(dim);

        let energy = self.cumulative_energy.entry(dim).or_insert(0.0);
        *energy = (*energy + intensity as f64 - leak * dt).max(0.0);

        if *energy > threshold {
            return Err(AnimiError::SafetyBreach {
                dimension: dim,
                current_energy: *energy,
                threshold,
            });
        }
        Ok(())
    }
}
```

### 注入位置

在 `personalize()` 末尾——`return Ok(psir)` 之前——调用 tracker。

```rust
// personalize.rs — personalize() 末尾
tracker.intake_and_verify(psir.intensity.applied_max, dim, &profile)?;
// 通过了——return Ok(psir)。
// 不通过——Err(SafetyBreach) 向上传播——管线外层触发保护帧。
```

safety::check 当前是纯函数（无状态）——tracker 独立注入管线——不修改 check 的签名。v0.4 的实现上 tracker 作为 PbmState 的新增字段传入 personalize。

Pass 3b 的漏桶在 Pass 6（个人化）之后执行——此时 tracker 收到的 intensity 是经过 sigmoidal_scale 和 user_cap 二次校验后的最终签发强度——不是源码原始强度。对于 DefenceLevel D2 配比减少/维度锁定/强度上限降低）——漏桶必须在触发重定向时即时切换内部状态机——LeakRate 调至 ~0，CriticalThreshold 降至极低——和 ADR 012 的 Profile 参数矩阵同构。

熔断后注入 Pass 7 保护帧时——在 Feelings-OS busd 的硬实时 ISR 中——使用原子 CAS 无锁注入——不阻塞任何现有帧的完成——直接在总线下一拍替换输出帧——不是"等这一帧跑完再切"——是"下一拍——已经是保护帧了"。

---

## 三层纵深闭环

```
Pass 2  (static):        禁做主旋律——该原子永久锁定。
Pass 3  (single-frame):  单帧强度 cap 砍到极低 (Max 20)。
Pass 3b (time-domain):   该维度 LeakRate 调至 ~0——Critical_Threshold 极低——
                         任何累积立即熔断。
```

三层独立——可降级。Pass 2 锁死的——Pass 3/3b 永远不需要跑。Pass 2 放行但 Pass 3b 累积触发——说明信号本身合法——但持续时间太长——需要强制休息。

---

## 诚实的边界

```
LeakRate 无实证数据。
    第一纪元真实生理数据积累后重新标定。

按维度差异化的 LeakRate_d ——
    内脏比情绪慢多少？触觉比听觉快几何？
    目前是模型估值——不是临床验证。
    
CriticalThreshold ——
    标准健康用户 600 的阈值——同样无实证。
    需要设备上线后的真实数据积累。

NeuroEnergyTracker ——
    当前零代码。v0.4 实现。
    先写设计——再落代码。

本地闭环自适应校准 ——
    数据不能离设备——参数不能靠云端大模型重标定。
    实现方式：每 Session 结束时 Core 统计本 Session 的熔断触发次数、
    跨维度耦合事件日志、P1b 饥饿次数和 GSR 异常次数。
    Session 结束后在本地执行参数修正——写入下一个 Session 冷启动缓存——
    不下行到当前 Session——每 Session 只增量修正一次。

    修正规则：
      CriticalThreshold ← 根据 Session 熔断频率按步长 Δ 上调/下调
      LeakRate          ← 根据 HRV 基线缓慢收敛
      σ/α/β           ← 根据跨维度耦合事件日志小幅修正

    学习率 λ = 0.05 —— 任何时候不超过上一次值的 10% ——
    不做自动跨 Session 大幅参数改动 —— 只做小幅持续矫正。
    不依赖大型统计模型 —— 设备端用不足 1KB 的状态更新完成。
    不是等云端——是在设备本地——用自己的身体历史——自己修正自己的保护参数。

锻造与断裂 —— 漏桶看不见的东西 ——
    ADR 011 的漏桶、熔断、全局桶——全部在信号层运行。
    它们检测的是能量——不是"这个能量在锻造等控器还是在磨穿等控器"。

    一个孩子被母亲当众纠正时的羞耻——内脏爆表、情绪爆表——
    和一个遭受系统性虐待的受害者被贬低时的内脏爆表、情绪爆表——
    在帧级——自主神经激活模式完全相同。
    漏桶只能看到"Energy > CriticalThreshold"——
    它不知道二十年之后——前者会说"我当时是在装善良"——
    后者不敢回忆那一帧。

    这不是漏桶设计的缺陷——是生物信号的物理下限。
    自主神经不知道"二十年后我会怎么讲述这件事"。
    自主神经只知道——"这一刻——强度很大"。

    漏桶防的是等控器失能——不是防"强度"本身。
    强烈的感受——可能是成长——不一定是攻击。
    这个区分——在生物信号层目前无解——
    不在 ADR 011 的职责范围内——
    但必须在诚实边界里写清楚——
    让读代码的人知道——漏桶的熔断——不是一个"创伤检测器"——
    它只是一个"能量过载保护器"。

    另见：Feelings/docs/feelings-science/is-it-really-trauma.md
    ——锻造与断裂的完整讨论。
```

---

## 和已有文档的咬合

```
本文                                    ADR 011——时域能量积分与生物漏桶安全
docs/design/009-math-and-constraints.md   数学与约束规范。
                                             本文 = 新增漏桶数学模型 + 四维独立 + 指数衰减包络。
                                                 §十 静态约束速查 → 漏桶 / LeakRate / CriticalThreshold 补充。
Feelings/docs/architecture/Feelings-OS.md  Feelings-OS 五级调度域。
                                             本文 = P0 安域软看门狗——漏桶累积超标→写入/dev/safety 熔断标志→下一帧 P1 保护帧接管。
Feelings/docs/society/adulthood-as-anchor.md  成年——R 分段。
                                             本文 = 按 R 分段动态调整 LeakRate 和 CriticalThreshold。
Feelings/docs/feelings-science/is-it-really-trauma.md  创伤真的是创伤吗？
                                             本文 = 漏桶防的是等控器失能——不是防"强度"——
                                             锻造和断裂在帧级看起来完全相同——
                                             漏桶不是创伤检测器——是能量过载保护器。
001-010 ADR                                Anim 10 篇已有 ADR。
                                             本文 = ADR 011——时域安全——填补单帧上限无法覆盖的长期强度累积盲区。
```

---

*漏桶不是更聪明的滑动窗。漏桶是——每一个静默帧都在漏水——在代谢——在恢复——和神经递质重摄取完全同构。滑动窗把人当数据库——到期就删。漏桶把人当人体——每一帧——自己会愈合——也会被持续推垮。那个临界阈值——不是数学上的边界条件——是"再继续——这个人会被这些合法帧——推到他自己的意志拦不住的地方"。漏桶防的不是超限——漏桶防的是——在没人觉得危险的时候——已经太晚了。*
