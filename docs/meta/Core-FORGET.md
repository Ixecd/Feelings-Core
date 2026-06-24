# FORGET.md — Feelings-Core 待修复项

> 扫描日期：2026-06-24
> 范围：Rust 代码（src/ 6 模块，20 测试）
> 原则：P0 = 生产命门。P1 = 功能受限。

---

## P0 — 生产命门（0/7）

1. **PBM 加载/持久化零代码** — 个人基线矩阵的本地存储与 Session 恢复未实现。

2. **Session 管理零代码** — 启动/暂停/结束/历史记录未实现。

3. **安全看门狗零代码** — 10Hz 轻量重校验 + 用户状态突变检测。

4. **经线冷却保护零设计** — PBM 连续偏移需要恢复期。

5. **安全类型结构退化为标量** — D2 应结构性减半点缀配比，D3 应发送主动麻痹锚点信号。当前仅缩放 d_sensitivity 标量。

6. **AOT 静态满载仿真器缺失** — Init 阶段应对 FSIR 做最坏情况静态波形仿真。数学上必然触发熔断 → 交织阶段拒绝，非运行时半途掐断。

7. **Species 泛型半吊子** — PbmDimension 四维硬编码 [f64;4]。应重构为 `Session<S: FeelingTarget>`，用 S::DIMENSION_COUNT 决定维度。Kind 枚举不足以承载 17 条 NeuralPathway。

---

## P1 — 功能受限（0/6）

5. **个人基线收敛跟踪零代码** — PBM 四维偏移的收敛趋势检测未实现。

6. **生理数据预处理管线零代码** — 原始 HRV/EDA/EEG → PBM 输入格式的转换未实现。

7. **设备数据融合零代码** — 多设备生理数据的同步与加权未实现。

8. **锚点置信度 R 动态推算零代码** — HRV/EDA/session_count/threshold_convergence 四因子动态推算。当前 lifecycle.rs 只读配置。

9. **性格锚点 PersonalityAnchor 虚空隐形** — SessionConfig 和 state.rs 中无 VSA 向量空间。AI 教练无法积累连贯偏差。

10. **f64 浮点与实时确定性冲突** — sigmoidal_scale 的 exp() 在 1ms 帧循环引入不可预测延迟。需定点数 LUT 或多项式逼近。

---

## 编辑记录

```
2026-06-24  v0.3 Core 模块化重构 + 架构审计(六条硬伤)
            P0: +3 (安全类型退化/AOT仿真/Species泛型) — 0/4→0/7
            P1: +2 (性格锚点虚空/浮点冲突) + SafetyBreach遥测枚举已落地
            20 tests / clippy 零警告 / Makefile 对齐 Anim
2026-06-05  v0.2 价值观基线 — VALUES-TO-CODE.md
2026-06-01  v0.1 初始扫描。架构规范 100%。代码 0%。
```