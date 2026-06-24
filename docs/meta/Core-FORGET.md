# FORGET.md — Feelings-Core 待修复项

> 扫描日期：2026-06-24（修订：2026-06-24）
> 范围：Rust 代码（src/ 6 模块，20 测试）
> 原则：P0 = 生产命门。P1 = 功能受限。
> 价值观基线：**已完成**。所有实现决策以 VALUES-TO-CODE.md 为索引。

---

## P0 — 生产命门（0/4）

1. **PBM 加载/持久化零代码** — 个人基线矩阵的本地存储与 Session 恢复未实现。FIXME: v0.3。

2. **Session 管理零代码** — Session 启动/暂停/结束/历史记录未实现。FIXME: v0.3。

3. **安全看门狗零代码** — Session 运行中用户状态突变的 10Hz 轻量重校验未实现。FIXME: v0.3。

4. **经线冷却保护零设计** — 经线建立后需要恢复期，不能连续焊、不能连续扯。FIXME: v0.4。

---

## P1 — 功能受限（0/4）

5. **个人基线收敛跟踪零代码** — PBM 四维偏移的收敛趋势检测未实现。

6. **生理数据预处理管线零代码** — 原始 HRV/EDA/EEG → PBM 输入格式的转换未实现。

7. **设备数据融合零代码** — 多设备生理数据的同步与加权未实现。

8. **锚点置信度 R 动态推算零代码** — R 不是静态配置——是设备从生理数据里推出来的。
   - HRV 基线偏离 → 副交感张力——越高越稳
   - EDA/GSR 波动幅度 → 情绪反应性——越小越稳
   - Session 累计次数 → 数据密度——越多越信
   - PBM 范围收敛速度 → 阈值收窄越快——锚越定
   - R < 0.3 → effective_cap = 20 的硬上限保留
   - 当前 `lifecycle.rs` 的 `anchor_confidence` 字段只读配置——缺动态推算函数 `compute_anchor_confidence()`
   FIXME: v0.3。

---

## 编辑记录

```
2026-06-24  v0.3 Core 模块化重构
            - src/config.rs — CoreConfig 全参数化 + Kind(物种) + validate()
            - src/pbm/state.rs — ColdStartGuard/DampingMatrix/DefenceLevel/SessionLabel
            - src/pbm/convergence.rs — sigmoidal_scale + d_sensitivity(从配置读)
            - src/tracker/bucket.rs — NeuroEnergyTracker 从 CoreConfig 构造
            - src/tracker/coupling.rs — 跨维度耦合 σ + 全局桶
            - src/session/lifecycle.rs — Session + 帧窗口 + 低锚cap
            - 梯度阈值/防御敏感系数/漏桶参数全部不再硬编码
            - 20 tests / clippy 零警告 / Makefile(dev)对齐 Anim
            - FORGET 迁至 docs/meta/Core-FORGET.md
            - P1 #8 新增——锚点置信度 R 动态推算

2026-06-05  v0.2 价值观基线
            - VALUES-TO-CODE.md 创建——三大类文档→Core模块映射+优先级矩阵

2026-06-01  v0.1 初始扫描。架构规范 100%。代码 0%。
```