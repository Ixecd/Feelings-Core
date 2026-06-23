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
├── config.rs          CoreConfig——所有可调参数的单一定义点
├── pbm/               个人基线矩阵
│   ├── state.rs       ColdStartGuard / DampingMatrix / DampingState / DefenceLevel
│   ├── convergence.rs sigmoidal_scale / PbmColdStartCoefficients / d_sensitivity
├── personalize/       Pass 6 — FSIR × PBM → PSIR
├── tracker/           ADR 011 — NeuroEnergyTracker 四维漏桶
│   ├── bucket.rs      漏桶状态机 + UserSafetyProfile
│   ├── coupling.rs    跨维度耦合 σ + 全局桶
├── session/           Session 生命周期
│   ├── lifecycle.rs   SessionId / SessionConfig / 帧窗口计数器
│   ├── cooldown.rs    跨 Session 6h 冷却
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

---

## 四、与 Feelings-OS 的接口

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

## 关联文档

- `docs/design/002-ir-architecture.md` — FSIR/PSIR/DSIR/ESIR 四层 IR
- `docs/design/003-pass-pipeline.md` — Pass 6-8 在 Core 里的职责
- `docs/design/011-neuro-leaky-bucket.md` — NeuroEnergyTracker 四维漏桶
- `docs/design/012-signal-rhythm-and-dynamic-cap.md` — N/M 恢复帧 + 动态 cap
- `docs/design/013-pipeline-hooks.md` — 运行时 Hook 机制
- `../Anim/docs/design/009-math-and-constraints.md` — Anim 侧数学约束规范
- `../Anim/docs/design/016-device-capability-trait.md` — DSIR 设备路由
