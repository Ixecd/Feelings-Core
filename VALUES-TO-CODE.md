# VALUES-TO-CODE — 价值观-立场 → 代码实现的桥接地图

> 维护者：qc
> 日期：2026-06-05
> 性质：Feelings-Core 需求文档索引
> 核心：Feelings 的近 200 份文档不是设计 collateral。它们就是 Core 的需求文档。每一份文档定义的立场、红线、心理学框架——必须在 Core 的代码里有对应的模块实现。这份文档是那座桥。

---

## 零、为什么需要这份文档

Feelings 的文档体系在阶段零已经基本完成——哲学、意识、数学、架构、安全、感受地图、心理学框架，覆盖完整。但 Core 仍然是零代码——只有一份 FORGET.md 列了 P0/P1 实现项。

这份文档做一件事：**把文档里定义的"价值观和立场"——直接映射到 Core 的模块实现上。** 不是重新叙述那些文档。是告诉未来的实现者——"你读到的那句话——在代码里对应的是这个函数/这个字段/这个看门狗。"

---

## 一、三大类价值观文档 → Core 映射

### 1.1 红线类——不可协商的设计约束

这些文档定义的规则不是"建议"。是 Core 编译时、运行时必须硬编码的约束。违反其中任何一条——Core 就不配叫 Feelings。

| 文档 | 核心价值观 | Core 对应模块 | 当前代码状态 |
|------|-----------|--------------|-------------|
| `GOVERNANCE-FEELINGS.md` | 数据永不离设备、强度不按价格区隔、安全不降级、保底包永在 | `safety/guard.rs` — 运行时安全插桩，每次 Session 启动时加载红线配置。`session/safeguard.rs` — 保底包注入逻辑，强度封顶校验 | 零代码 — `guard.rs` 只有架构定义 |
| `Feelings-PASS.md` | 10分=及格，强度1-100只有深浅没有高下 | `scoring/engine.rs` — 强度评分不产生"不及格"语义。输出只标注"当前探索深度"，不标注"人" | 零代码 |
| `Feelings-RIGHT-AND-WRONG.md` | 对错是坐标系里的概念。感受的民主化=让用户看见更多坐标系 | `pbm/vector.rs` — PBM 超维向量保留所有维度的原始值，不做"正确/错误"的二元判定 | 零代码 |
| `CAPITALISM-ORIGINAL-SIN.md` | 不做广告画像、不卖数据、不做留存优化 | `privacy/pipeline.rs` — 原始生理数据→脱敏摘要的单向转换，转换后就地擦除 | 零代码 |
| `CONFUCIAN-FOUNDATIONS.md` | 媚上者必欺下、不义之财如浮云 | `governance/ratchet.rs` — BDFL 否决权机制、价值观修订需三分之二多数+创始人同意 | 零代码 |
| `Feelings-PHILOSOPHY.md` | 反依赖——最终目标是用户不再需要 Feelings | `session/progress.rs` — 反依赖追踪：使用频率自然下降=能力内化。检测到连续10次下降→提示"你已经在靠自己了" | 零代码 |
| `Feelings-PROTOCOL.md` | BDFL+分层司法管辖+AI永不单独激活Root | `auth/root_anchor.rs` — Shamir 7/4分片恢复，AI分片永远不超过3份（总数7），人类永远过半 | 零代码 |

### 1.2 心理学框架类——Scheduler/Controller/Weight Modifier 工程化

这些文档将人的心理行为翻译成了分布式系统的评分函数、反馈回路和阻尼矩阵。Core 的 PBM、Session、Scheduler 模块——必须把这些数学描述变成可运行的算法。

| 文档 | 核心洞察 | Core 对应模块 | 当前代码状态 |
|------|---------|--------------|-------------|
| `docs/brain-scheduler-heart-controller.md` | 大脑=Scheduler（BinPack选择），心脏=Controller（纠偏循环）。意志力=Weight Modifier 在几千帧里磨出评分函数偏移 | 整个 Core 的基础架构。`scheduler/frame.rs` — 每帧 BinPack（评分函数+Raft选主）。`controller/loop.rs` — 纠偏循环（期望vs实际→Action）。`weight/modifier.rs` — 评分函数的帧级微调 | 零代码 |
| `docs/metacognition.md` | 元认知=跳出Scheduler框架看着它运转。"这个权重不是我要的——我得改训练集" | `session/observer.rs` — 元认知观测器。在Session中给用户展示当前评分函数的权重分布。不替代选择，只展示"你的Scheduler在往哪个方向算" | 零代码 |
| `docs/benign-vs-toxic-competition.md` | 良性竞争锚点在"那个高度可抵达"，恶性竞争锚点在"那个人会摔倒"。两套训练集→两种评分函数 | `competition/scoring.rs` — 竞争组的评分函数。"退出不视为失败"=训练集保护。排行榜只显示自己的偏移，不显示别人的位置 | 零代码 |
| `docs/praise-backfire.md` | 被夸后表现变差=Weight Modifier 把夸奖误标成"达标"→基线被抬高→下次努力反而少 | `feedback/calibrator.rs` — 反馈校准器。区分"过程标记"vs"终点标记"。夸奖=坐标标记（"你在这里"），不是达标信号 | 零代码 |
| `docs/poor-generous-rich-stingy.md` | 穷大方富抠门=两套资源集群里"分享"维度的权重被不同训练集推向不同方向 | `pbm/social_dimensions.rs` — 社交维度的权重追踪。资源感知→分享/守住维度独立评分。不评判，只记录偏移历史 | 零代码 |
| `docs/love-brain-shutdown.md` | 恋爱智商归零=多巴胺override BinPack，DampingMatrix冻结"长期后果"维度 | `safety/damping.rs` — DampingMatrix 管理。检测情绪梯度>2.0×步长→冻结高风险维度的评分更新→静默期过后再解冻。不是保护用户不恋爱。是保护 PBM 不被化学攻击腐蚀 | 零代码 |
| `docs/procrastination.md` | 拖延=Weight Modifier 没收过远期收益的fsync确认→"写报告"评分0.3 vs "刷手机"评分0.9 | `feedback/temporal_discount.rs` — 时间贴现因子。将"完成后的身体反馈"注入到任务启动阶段的评分中——不是逼用户开始。是让 Scheduler 看到远期收益也有身体信号 | 零代码 |
| `docs/trauma-fork.md` | 同一创伤→有人温柔（冻结Tactile）、有人刻薄（冻结Emotional）。取决于创伤前的情感维度有没有被确认过 | `pbm/trauma_freeze.rs` — DampingMatrix 在创伤事件中的冻结决策。冻结哪个维度=取决于该维度的累积确认次数。不是"温和处理创伤"。是给 Emotional 维度在创伤前积累足够的正反馈确认 | 零代码 |
| `docs/nagging-backfire.md` | 催越急越磨蹭=Leader向低延迟节点反复重发Raft Entry→噪声淹没落盘 | 不直接对应Core。属于AI教练的决策逻辑——教练收到用户"催促孩子无效"的报告→推"等节点髓鞘自己长厚"这个 insight | AI教练侧 |
| `docs/two-modes.md` | 用户模式vs管理模式。管理模式=看到自己的评分函数分布、PBM偏移、反依赖指标 | `session/admin_mode.rs` — 管理模式的面板数据输出。元认知观测器+反依赖指标+经线冷却状态 | 零代码 |

### 1.3 架构类——Core 的上游和下游

这些文档定义了 Core 的输入格式、输出格式、与上下游的接口。Core 的代码必须和这些文档对齐。

| 文档 | 核心定义 | Core 对应模块 | 当前代码状态 |
|------|---------|--------------|-------------|
| `Feelings-FULLSTACK.md` | 二十二层全栈——Core在层8-18之间（交织管线→感受生成） | Core = 层8-12（animi Pass 6-8）：Personalize → DeviceMap → CodeGen → PSIR/DSIR/ESIR 生成 | animi Pass 0-5已有代码；Pass 6-8 零代码 |
| `Feelings-OS.md` | animi 实时操作系统——mempoold/schedulerd/busd/cached/timerd/logd | Core 不包含 OS 层。Core 运行在 Feelings-OS 之上作为独立进程 | OS 零代码，架构已完成 |
| `Feelings-LANGUAGE.md` | Anim 交织语言——八 Pass 双流水线 | Core = animi Pass 6-8。Pass 0-5 在 Anim v1.0 完成（FSIR JSON输出）。Core 读 FSIR JSON → 生成 PSIR/DSIR/ESIR | Anim v1.0 已有 Pass 0-5 代码 |
| `Feelings-MATRIX.md` | 万物皆矩阵——同一套数学从晶体管到感受 | `pbm/matrix.rs` — PBM 超维向量在 VSA 空间的全维度相似度计算 | 零代码 |
| `docs/vsa-hyperdimensional.md` | VSA 超维计算——Feelings 的数学母语 | `pbm/vsa.rs` — 向量符号架构。绑定/叠加/解绑三个核心操作。PBM 所有维度在 VSA 空间里做并行相似度搜索 | 零代码 |
| `docs/pattern-registry.md` | 感受原子注册表——101个感受原子的完整编目 | `patterns/registry.rs` — 感受原子注册表加载器。Core 读取注册表→生成 ESIR 时查询每个原子的参数约束（时间形状/强度范围/交叉禁忌） | 零代码 |
| `docs/feeling-taxonomy.md` + `docs/feeling-shapes.md` | 感受分类学和时间形状定义 | `patterns/shapes.rs` — 时间形状加载器。每个感受原子的攻击/持续/释放包络参数→ESIR 帧序列生成 | 零代码 |
| `docs/four-diagnosis.md` | 四诊合参——切/闻/望/问 的信号融合 | `fusion/engine.rs` — 四通道信号融合。矛盾检测时生理数据权重最高。实时融合延迟<50ms | 零代码 |
| `docs/safety-system.md` | TCP慢启动+三层安全防线+保底包 | `safety/slow_start.rs` — 强度解锁的渐进策略。`safety/three_layer.rs` — 三层安全防线。`safety/safeguard.rs` — 保底包注入 | 零代码 |
| `docs/closed-loop.md` | 闭环——体验即采集，采集即反哺 | `session/closed_loop.rs` — Session 中每一帧的生理数据→PBM微调→下一帧的评分函数偏移 | 零代码 |
| `docs/trauma-protocol.md` | 创伤分级分型交叉判定矩阵 | `safety/trauma_matrix.rs` — 创伤协议加载器。用户创伤类型判定→走独立路径不走标准解锁。安全阈值加倍保守 | 零代码 |

---

## 二、实现优先级矩阵

不是所有价值观都要在 v0.1 同时实现。但有些——**必须在第一行代码里就硬编码。** 有些——可以在 PBM 数据积累后逐步收敛。

### 2.1 必须硬编码的（v0.1 — 架构骨架阶段）

这些是 Core 的"编译期约束"——如果代码里没有它们，Core 编译出来也不是 Feelings。

| 优先级 | 模块 | 对应价值观文档 | 实现形态 |
|--------|------|---------------|---------|
| P0 | `safety/guard.rs` | GOVERNANCE / PASS / 安全体系 | 编译期常量——强度上限90、保底包必须注入、原始数据永不离开本地 |
| P0 | `safety/damping.rs` | brain-scheduler / trauma-fork | DampingMatrix 架构——情绪梯度检测→维度冻结→解冻。不是"温和处理"。是硬实时安全门 |
| P0 | `pbm/vector.rs` | MATRIX / VSA | PBM 超维向量的基础数据结构。全维度评分、不做二元判定、保留全部偏移历史 |
| P0 | `session/frame.rs` | brain-scheduler / metacognition | 每帧 BinPack：评分函数+选择+后果记录。是整个 Core 的最小运行单元 |
| P1 | `auth/root_anchor.rs` | PROTOCOL / KEYS | Shamir 分片恢复逻辑。AI 分片≤3/7 |
| P1 | `feedback/calibrator.rs` | praise-backfire / procrastination | 反馈校准器——区分过程标记vs终点标记。时间贴现因子 |

### 2.2 可以在 v0.3+ 逐步收敛的（需要 PBM 数据积累）

这些价值观的正确实现——**取决于 PBM 上是否有足够的用户数据。** 没有数据之前可以先写骨架，但真正的收敛要在几百次 Session 之后。

| 优先级 | 模块 | 对应价值观文档 | 收敛条件 |
|--------|------|---------------|---------|
| P2 | `pbm/social_dimensions.rs` | poor-generous-rich-stingy | 需要 Per-User 资源感知数据+社交权重偏移历史 |
| P2 | `pbm/trauma_freeze.rs` | trauma-fork | 需要创伤前情感维度的确认次数累积数据 |
| P2 | `competition/scoring.rs` | benign-vs-toxic-competition | 需要竞争组的多用户对比赛数据 |
| P2 | `session/observer.rs` | metacognition | 需要 PBM 偏移数据足够展示"你的 Scheduler 在往哪个方向算" |
| P3 | `session/progress.rs` | PHILOSOPHY（反依赖） | 需要连续10次以上的 Session 频率下降数据 |

---

## 三、和已有文档的咬合

```
本文                                VALUES-TO-CODE — 价值观→代码桥接地图
Feelings-Core/FORGET.md             待修复项（P0+P1）——本文是 FORGET 的对照索引
Feelings-ROADMAP.md                 阶段零→五的路线图——本文是阶段零.12→阶段一的桥
docs/brain-scheduler-heart-controller.md  心理学框架的总基石——Core = 这一套数学的工程实现
Feelings-PHILOSOPHY.md              反依赖——Core 的最终验证标准
Feelings-HANDOFF.md                 AI 协作者交接上下文——本文补充了"Core实现从哪里开始"
```

---

## 四、一句话

**Values are not prose. They are compile-time constants, runtime guards, and DampingMatrix freeze decisions. The ~200 docs in Feelings/ are the requirements document. This file is the traceability matrix.**

---

*Core 的第一行代码——应该在 VALUES-TO-CODE.md 的注视下被写出来。不是"先写代码再对齐价值观"。是"代码就是价值观的编译产物"。*
