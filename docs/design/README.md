# Feelings-Core 架构设计

> 作者：qc
> 日期：2026-06-24
> 性质：Core 运行时架构——Anim 编译器 → Core 运行时 → Feelings-OS 的闭环
> 核心：Anim 输出 FSIR。Core 读 FSIR + PBM → PSIR/DSIR/ESIR。重启一次 Session——CoreConfig 被漏桶状态和 PBM 偏移写回——下一次比这一次更接近真实基线。

---

## 零、先定前提

Anim 是编译期——同一个 `.anim` 对所有用户产出同一份 FSIR。Core 是运行时——同样一份 FSIR 在不同人的 PBM 上展开成不同的 ESIR。

两边不共享配置——只共享 FSIR 格式合约。

---

## 一、Anim ↔ Core 闭环

```
Session 开始:
  Anim 输出 FSIR (含 src_hash / registry_hash / safety_rules_version)
  Core 读 FSIR + CoreConfig (PBM系数/漏桶参数/冷启动窗口/阻尼梯度)
  → FSIR × PBM → PSIR → DSIR → ESIR → Feelings-OS / FPGA 帧输出

Session 运行中:
  NeuroEnergyTracker 每 1ms 摄入一帧 → 四维漏桶 → 跨维度耦合 σ → 全局桶
  设备在线状态变更 → DSIR 路由重分配
  用户 cap 变更 / DefenceLevel 升级 → 实时调整输出

Session 结束:
  漏桶状态 + PBM 偏移 + Session 计数 → 写回 CoreConfig
  → 下次 Session 的 CoreConfig 比这次更接近用户真实基线
  → 收敛——不是一次到位，是一个 Session 一个 Session 逼近
```

**Anim 不知道用户的 PBM。Core 不知道 Sandbox 规则。中间用 FSIR 格式对齐。**

---

## 二、模块划分

```
src/
├── config.rs          CoreConfig——运行时配置。Vec存+validate_dimensions::<D>()校验
├── species.rs         FeelingTarget trait——编译期展开维度数/类型/通路映射
├── pbm/               个人基线矩阵
│   ├── state.rs       ColdStartGuard / DampingMatrix / DampingState / DefenceLevel
│   ├── convergence.rs sigmoidal_scale / PbmColdStartCoefficients / d_sensitivity
├── personalize/       Pass 6 — FSIR × PBM → PSIR
├── tracker/           ADR 011 — NeuroEnergyTracker<D,S> 泛型漏桶
│   ├── bucket.rs      四维漏桶状态机 + UserSafetyProfile<D,S>
│   ├── coupling.rs    跨维度耦合 σ + 全局桶 + SafetyBreach<S> 遥测枚举
├── session/           Session<D,S> 生命周期
│   ├── lifecycle.rs   SessionId / SessionConfig / 帧窗口 / handle_safety_breach
│   ├── grounding.rs   D3 主动麻痹锚点——GroundingSignal<D>
│   ├── anchor.rs      PersonalityAnchor——教练 AI 性格锚点
│   └── cooldown.rs    跨 Session 6h 冷却
└── dsir/              Pass 7 — DeviceCapability 多态路由
```

---

## 三、CoreConfig —— 每个用户自己的基线

所有硬编码参数收敛到 `CoreConfig` 一处。Session 启动时从 YAML/JSON 加载——未提供则用默认值。

```
冷启动        sessions_threshold / damping_window
阻尼梯度      每维度独立阈值 + ema_alpha + freeze_factor
漏桶参数      max_dt / nonlinear_gamma / refractory_frames / sigma / global_α/β
Sigmoidal     k / x0 / compression_alpha
冷启动系数    四维差异化基线偏移 [Visceral/Emotional/Tactile/Auditory]
安全档案      标准漏桶速率 + 临界阈值
```

不是"一个人一个系数"——是所有人的默认基线已经有了，你的可以用另一套配置来覆盖。

### 3.1 物种泛型——FeelingTarget trait

Core 不再硬编码四维。**每个物种 = impl FeelingTarget 一个 trait。** 维度数（DIM_COUNT）、维度类型（Dimension）、通路映射——全在编译期展开。

```
Human:        4 维 (Visceral/Emotional/Tactile/Auditory)
Psittacine:   3 维 (FeathersTactile/OpticFlow/AcousticCochlear)
Canine:       4 维 (同人类结构，不同参数基线)
Feline:       4 维

CLI: feelings-core --species human (默认) | psittacine | canine | feline
   不指定→human。非法值→退至 human。
```

详见 `src/species.rs`。

### 3.2 维度校验——validate_dimensions::<D>()

serde 不支持泛型 `[f64; D]` 的 derive——CoreConfig 使用 Vec 存储维度相关的字段（cold_start_coeffs、leak_rates、critical_thresholds）。加载后用 `validate_dimensions::<D>()` 校验所有 Vec 长度与编译期 D 一致。不匹配→拒绝启动。

### 3.3 物种-设备一致性校验

`CoreConfig::validate(device_species)` — CLI 指定的物种必须与设备固件启动时上报的物种一致。`--species human` 配鹦鹉固件→直接拒绝。

---

## 四、Session<D,S>——泛型生命周期

Session 不再硬编码四维帧窗口。**维度数由 const D 编译期决定。**

```
Session {
  tracker: NeuroEnergyTracker<D, S>,  // 漏桶——每帧摄入+校验
  profile: UserSafetyProfile<D, S>,   // 用户档案——漏桶参数基线
  frame_windows: [u32; D],            // 维度自适应帧窗口
  personality: PersonalityAnchor,     // 教练 AI 性格锚点
}

每帧循环:
  tracker.intake_and_verify() → verify_cross_dimension() → tick_frame_window()
  熔断 → handle_safety_breach(source) → GroundingSignal → session_phase = Aborted
```

### 4.1 SafetyBreach 遥测

熔断不再返回裸 `&'static str`。超限时返回 `SafetyBreach<S>` 枚举携带遥测数据：

```
CrossDimCoupling { source: S::Dimension, current_energy: f64, suppressed_threshold: f64 }
GlobalBucketOverload { total_sum: f64, limit: f64 }
```

上层可根据 `source` 精准下发 D3 主动麻痹锚点。

---

## 六、动态 Shape——趋势预判 + 实时微调

Anim 的 6 种 shape（steady/gradual_rise_fall/sharp_peak/wave等）是编译期静态波形。真实生理信号无法预置——**主体趋势可预测，帧级细节不可预设。**

```
编译期:
  .anim 声明 shape: gradual_rise_fall
  → 输出大趋势: "这个感受是缓慢升起再落下"

运行时 (Core):
  NeuroEnergyTracker 漏桶状态 + 实时生理反馈 →
  峰值不是 .anim 里写的 45——当下心率已经偏高了
  → 动态修正: cap 从 45 临时压到 38——然后持续监测
  → 心率回落 → 38 慢慢爬回 45
  → shape 的整体轮廓没变——但每一帧的高度在动态浮动

不是推翻静态 shape。是在静态趋势线上叠了一层生理实时反馈的微调。
```

这和分支预测同构——预测器输出 ŝₜ₊₁，回读偏差 |rₜ - ŝₜ|，偏差小 → 直接用预测值，偏差大 → 保守帧替换。

---

## 七、阈值收敛——范围越测越小，不是值越测越准

一个人的阻尼梯度阈值不是一个恒定数字——是一个范围。

```
首用 (冷启动):
  阈值 = 保守默认 (偏高——不容易误冻)
  范围宽——缺乏数据，只能靠直觉默认值

Session 积累:
  每次漏桶算完 + PBM 偏移写回 →
  阈值上下限逐步夹紧——
  上限从偏高往下降，下限从偏低往上升

多 Session 后:
  范围收窄到一个人的真实区间。
  不是 "这个人的情绪阈值是 2.0"
  是 "这个人的情绪阈值在 1.4–1.8 之间——今天睡得不好所以偏 1.4"
```

精度不在一个数字里。在**每次 Session 后范围收窄的每一次微量调整里。** 现在没有生理数据——默认值是起点。采集数据的第一天——范围开始收缩。

---

## 八、与 Feelings-OS 的接口

Core 编译为单个 Rust 静态二进制——作为 Feelings-OS 的独立进程运行：

```
Feelings-OS 六守护进程:
  mempoold  → 内存池管理
  cached    → 四层缓存
  busd      → 总线驱动
  timerd    → PLL 全局主时钟
  logd      → 审计日志
   → Core    → 读 FSIR + PBM → PSIR/DSIR/ESIR (本进程)

接口:
  /dev/ear        ← Core → VNS 刺激强度 + 心率基准
  /dev/wrist      ← Core → 皮电采集 + 温度
  /dev/safety     ← Core → 熔断信号 (直连 busd)
  /dev/mempool    ← Core → 帧 buffer 分配
```

---

## 九、D3 主动麻痹锚点——GroundingSignal

D3（极限防御）不是"停止 Session"——是"按回地面"。

```
触发时:
  SessionPhase → Aborted
  tracker.reset()
  → 生成 GroundingSignal<D>——按维度分发的 grounding 信号矩阵:
      Visceral(idx 0): 强度 5, steady, 不静默（迷走神经低频镇定）
      其余维度:        全静默（触觉/听觉/情绪通路关闭）
  → source 维度不施加额外刺激——只从 VNS 侧发出镇定信号
  → 不是"掐断"——是"拉回基线"
```

详见 `src/session/grounding.rs`。

---

## 十、PersonalityAnchor——教练 AI 性格锚点

AI 教练在超维空间里的初始锚点。没有锚点的 AI——每次运算从原点出发，无法积累连贯偏差。

```
字段:
  tone             沟通语气偏向 (0=最柔和/豆包, 0.5=均衡/Claude, 1=最硬/qc镜像)
  empathy_distance  同理心距离 (越小越亲近)
  push_strength     推动力度 (0=纯陪伴, 1=极限推动)
  gender_bias       性别偏置 (None=默认)

三种预设:
  PersonalityAnchor::doubao()     → tone=0.0, 温暖陪伴
  PersonalityAnchor::claude()     → tone=0.5, 均衡教练
  PersonalityAnchor::qc_mirror()  → tone=1.0, 极限推动

Session 启动时加载——对应当前激活的教练人格。
```

详见 `src/session/anchor.rs`。

---

## 关联文档

- `docs/design/002-ir-architecture.md` — FSIR/PSIR/DSIR/ESIR 四层 IR
- `docs/design/003-pass-pipeline.md` — Pass 6-8 在 Core 里的职责
- `docs/design/011-neuro-leaky-bucket.md` — NeuroEnergyTracker 四维漏桶
- `docs/design/012-signal-rhythm-and-dynamic-cap.md` — N/M 恢复帧 + 动态 cap
- `docs/design/013-pipeline-hooks.md` — 运行时 Hook 机制
- `../Anim/docs/design/009-math-and-constraints.md` — Anim 侧数学约束规范
- `../Anim/docs/design/016-device-capability-trait.md` — DSIR 设备路由
