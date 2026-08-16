# ADR 019: UserProfile——结构化用户档案 KV 存储

> 状态：定稿
> 日期：2026-06-26
> [Core] UserProfile 结构定义 + ProfileField trait + 存储策略 归属 Core。
> [feelingsd] 实际读写由边缘设备执行——Core 只定义格式。
> 对应：ADR 018 §6.1（私人 light log）——UserProfile 是 Agent 对用户的了解；light log 是 Agent 之间的记忆。两样都需要。
> 原则：设备空间有限，每字节都必须是 Agent 需要的东西。没有"存着以后可能有用"的字段。

---

## 〇、先定前提

```
私人的 light log = Agent 之间的对话记忆     ← "他对我说了那句话"
UserProfile      = Agent 对用户的结构化了解  ← "她叫小姐，天蝎座，怕冷"

设备空间有限 + 不联网 = 每个字段都必须有至少一个 Agent 在 deliberate() 里查到。
没有 filler 字段。没有"备着以后用"。

档案只存数据，不存定义。判断（性格标签、状态结论）是教练在
deliberate() 时从数据现算的，用完即弃，永不落盘——落盘的判断
会过期（人变判断不变）、会自证（旧判断当地面真值）、会把档案变成标签机。
档案是 etcd，教练是 controller：etcd 存事实，controller 现算。
```

> 性质：Core 层数据结构——Agent 的决策输入
> 核心：每个字段标注用途——哪个 Agent 用于什么决策

---

## 一、全量字段表——每字段标注使用者

```rust
pub struct UserProfile {
    // ═══════════════════════════════════════════════════════════
    // 基础身份——所有 Agent 共用
    // ═══════════════════════════════════════════════════════════
    pub name: String,               // 豆包: 称呼用。qc: 确认身份用
    pub gender: Gender,             // 豆包: 称呼（先生/小姐/哥/姐）
                                    // 立场：只有 Male | Female。教练观察写入 A 层，锁死
    pub age: Option<u8>,            // Claude: 判断 stage。qc: 未成年人→安全加强
    pub zodiac: Option<ZodiacSign>, // 豆包: 个性化对话（"天蝎今天会是好的一天"）
    pub birthday: Option<NaiveDate>,// 豆包: 生日提醒。不是社交功能——是私人记忆

    // ═══════════════════════════════════════════════════════════
    // 称呼偏好——豆包专属
    // ═══════════════════════════════════════════════════════════
    pub preferred_names: Vec<String>,  // ["小姐", "哥们", "老张", "亲爱的"]
    /// 针对每个 Agent 的默认称呼——Agent 查自己的 key
    pub agent_preferred_names: HashMap<CoachPersona, Vec<String>>,
    // 豆包查 agent_preferred_names[CoachPersona::Doubao] → "小姐"
    // qc 查 agent_preferred_names[CoachPersona::QcMirror] → "哥们"

    // ═══════════════════════════════════════════════════════════
    // 作息 + 身体——豆包 + Claude 共用
    // ═══════════════════════════════════════════════════════════
    pub timezone: Option<Timezone>,    // 豆包: "今天这么晚了还没睡？"
    pub typical_wake_time: Option<u32>,// 豆包+Claude: 判断"这个时间回复是不是异常"
    pub typical_sleep_time: Option<u32>,
    pub height_cm: Option<u16>,        // Claude: 部分健康相关建议
    pub weight_kg: Option<f32>,

    // ═══════════════════════════════════════════════════════════
    // 背景——Claude 专属（决定 push 的深度和方向）
    // ═══════════════════════════════════════════════════════════
    pub hometown: Option<String>,       // "哪里人"
    pub education: Option<String>,      // "什么学历背景"
    pub career_stage: Option<String>,   // "当前什么阶段"（学生/创业/在职/空窗）
    pub expertise: Vec<String>,         // "深耕什么方向"
    pub current_interests: Vec<String>, // "最近接触什么内容"

    // ═══════════════════════════════════════════════════════════
    // 偏好——豆包 + Claude 共用
    // ═══════════════════════════════════════════════════════════
    pub likes: Vec<String>,            // "喜欢什么" → 正面反馈的方向
    pub dislikes: Vec<String>,         // "不喜欢什么" → 避免的路径
    pub traits: Vec<String>,           // "特点" → ["坚韧", "容易自我怀疑"]
    pub cares_about: Vec<String>,      // "在乎什么" → ["家人", "公平", "被认可"]
    pub afraid_of: Vec<String>,        // "怕什么" → qc 参考——推的边界别越过

    // ═══════════════════════════════════════════════════════════
    // 健康——qc 专属（决定安全 override 的触发阈值）
    // ═══════════════════════════════════════════════════════════
    pub family_genetic_diseases: Vec<String>, // "家族遗传病史"
    pub allergies: Vec<String>,              // 过敏——影响可能的生理刺激类型
    pub chronic_conditions: Vec<String>,     // 慢性病——影响强度 cap

    // ═══════════════════════════════════════════════════════════
    // 决策锚点——A1 层，人生岔口的选择史（教练观察记录，仪式写入）
    // ═══════════════════════════════════════════════════════════
    pub decision_anchors: Vec<DecisionAnchor>,

    // ═══════════════════════════════════════════════════════════
    // 元数据——Core 管理，Agent 不直接读写
    // ═══════════════════════════════════════════════════════════
    pub created_at: u64,
    pub updated_at: u64,
    pub source_map: HashMap<String, ProfileSource>, // field_name → 来源
}

pub struct DecisionAnchor {
    pub kind: AnchorKind,     // 职业/居住/合同/关系/其他
    pub summary: String,      // 选了什么（"签了 30 年房贷"）
    pub reason: String,       // 当时为什么（选择 + 理由一起记）
    pub status: AnchorStatus, // Active | Completed——完结不删除，标记归档
    pub at: u64,              // 时间戳
}

pub enum AnchorKind { Career, Residence, Contract, Relationship, Other }
pub enum AnchorStatus { Active, Completed }

pub enum ProfileSource {
    UserProvided,     // C 层——用户自己填的偏好，用户可改
    CoachObserved,    // B 层——Agent 观察写入，版本化留痕
    PBMInferred,      // B 层——从生理数据推算，带来源可复算
    Anchor,           // A 层——锚：决策锚点/金标准/性别，仪式写入锁死
}
```

## 二、存储策略——设备有限，按优先级分层

```
Tier 1（必须存——Agent deliberate 每轮必查）:
  name, gender, preferred_names, agent_preferred_names
  → 约 200B。JSON 行存储。内存常驻。

Tier 2（高频查——影响对话质量）:
  zodiac, birthday, timezone, typical_wake/sleep, traits, cares_about, likes, dislikes
  → 约 500B。JSON 行存储。首轮 deliberate 时加载。

Tier 3（低频查——影响安全判断和深度决策）:
  education, career_stage, expertise, family_genetic_diseases, allergies, chronic_conditions
  → 约 1KB。JSON 行存储。每次 session 启动时加载一次。

Tier 4（可省略——用户不提供就不存）:
  height_cm, weight_kg, hometown
  → Optional 字段。不提供=不占空间。
```

### 1.1 固定取值字段——枚举化约束

当前大量字段是自由文本——不同 Agent 写入时格式不统一。

```
自由文本                        枚举化
────────                        ──────
career_stage: "创业"/"startup"  CareerStage: Student | Employed | Unemployed | Founder | Retired
gender: "男"/"male"/"M"         Gender: Male | Female——立场只有男女，没有谱系
zodiac: "天蝎"/"scorpio"       ZodiacSign: Aries..Pisces——固定12值
education: "本科"/"bachelor"   Education: HighSchool | Bachelor | Master | PhD | Other
```

### 1.2 敏感字段分级标注

```
Sensitivity::Normal     name, gender, zodiac, career_stage, likes, dislikes...
Sensitivity::Personal   traits, cares_about, afraid_of
Sensitivity::Medical    allergies, chronic_conditions, family_genetic_diseases
```

加密存储 / 访问权限校验直接按 sensitivity 分级。Medical 级额外隔离——写入 Tier3 加密存储区。

**总空间估算：< 2KB per user。** 设备上 4MB reserved 够存 2000 个用户档案。

### 2.1 字段级 TTL——数据没有"永远有效"

```
ProfileSource 不只是一张标签——它决定了数据的生命周期：

  PBMInferred 字段    → TTL 30 天  → 30 天后自动降级清除
                      typical_wake/sleep 由 PBM 从活跃时间推断——
                      用户换了工作/换了国家 → 30 天不够新数据覆盖 → 过期自动清
  CoachObserved 字段  → TTL 90 天  → 90 天后自动降级清除
                      "当前阶段"、"最近接触的内容"——半年后可能完全变了
  UserProvided 字段   → 永久有效    → 用户自己管理（C 层：偏好）
                      用户主动更新才变，系统不设 TTL
  Anchor 字段         → 两种生命周期
                      A1 决策锚点     永久坐标：岔口选择史，
                                      完结不删除，标记 Completed 归档
                      A2 身体金标准   周期性过期：定期重测，旧值归档
                      gender/birthday 事实锚：写后锁死

实现——每个字段的 source_map entry 从 ProfileSource 升级为:
  (ProfileSource, u64)  // (来源, 首次写入的时间戳)

判断是否过期:
  PBMInferred 字段: now - timestamp > 30×86400×1e9 → 过期
  CoachObserved 字段: now - timestamp > 90×86400×1e9 → 过期
  UserProvided 字段: 永不过期（用户管理）
  Anchor 字段: A1 永不过期；A2 按体检周期（每几个月）归档旧值

不额外储存 TTL 列——用来源类型映射到固定常量——省存储。
```

## 三、读写接口——KV 最少操作集

```rust
pub trait ProfileStore {
    /// 读取整个档案。首轮 deliberate 时加载。
    fn load(&self, user_id: &str) -> Result<UserProfile, ProfileError>;

    /// 写入单个字段——增量更新，不覆写全量。
    /// source 标记来源与层级——A(Anchor) > C(UserProvided) > B(CoachObserved/PBMInferred)
    /// A 层字段的写入必须走仪式通道（确认 + 留痕），普通 put 拒绝。
    fn put_field(
        &mut self,
        user_id: &str,
        field_name: &str,
        value: &str,           // JSON string value
        source: ProfileSource,
    ) -> Result<(), ProfileError>;

    /// 查询单个字段——Agent 在 deliberate 中途按需查
    fn get_field(&self, user_id: &str, field_name: &str) -> Option<String>;

    /// 列出所有存在值的字段名
    fn fields(&self, user_id: &str) -> Vec<String>;

    /// 删除字段——用户主动要求。
    /// C 层字段可删；A 层字段不可删（只能经仪式通道改状态/归档）。
    fn delete_field(&mut self, user_id: &str, field_name: &str) -> Result<(), ProfileError>;
}
```

**没有"全量覆盖"接口。**

### 3.1 同来源写入冲突——"最新生效"

```
高优先级来源拒绝低优先级写入——已规定。但同一来源的写入冲突需单独处理。

规则:
  单值字段 → "最新生效" ——最后写入的覆盖之前的
    例: CoachObserved 的 career_stage——先后两个 Agent 都写入
        → 以最新时间为准。同一来源，时间就是优先级。

  列表字段 → 合并去重——不覆盖，只追加
    例: CoachObserved 的 traits——豆包写入["坚韧", "内向"],
        Claude写入["内向", "有创造力"]
        → 合并: ["坚韧", "内向", "有创造力"]
        → 去重: "内向"只保留一条

  用户写入 → 先清空列表再写（用户的全量覆盖权限限于 C 层字段）
     例: traits = ["坚韧", "内向", "有创造力"]
         用户重新填 traits = ["内心强大"]
         → 结果: ["内心强大"]——不是追加，是重置
     A 层字段不适用本规则——A 层不接受普通写入。
```

### 3.2 Tier1 内存常驻——写穿保证不掉电丢

```
Tier1 字段 (name/gender/preferred_names) 在内存常驻——但边缘设备可能意外掉电。

规则:
  所有 Tier1 字段默认「写穿」——put_field 同步更新内存 + 落盘。
  数据量极小（<200B/人）——写盘开销可忽略，一致性更有保障。

  Tier2/3 字段——每次 session 结束时批量落盘——不逐条写穿。
  不是偷懒——是 DeltaBuffer 的理念: 攒一批，写一次。
```

## 四、每个 Agent 如何使用——deliberate() 的查表顺序

```
daudao::deliberate():
  1. profile.get_field("agent_preferred_names") → "小姐" → 称呼决定
  2. profile.get_field("zodiac") → 天蝎 → "今天会是一个好天"
  3. profile.get_field("typical_sleep_time") → 2am → "又熬夜了？"
  4. 调用 mood 的 update_from_input(用户输入)
  5. 产出 opinion

claude::deliberate():
  1. profile.get_field("career_stage") → 创业早期 → push 深度校准
  2. profile.get_field("cares_about") → "公平" → 提示方向
  3. profile.get_field("afraid_of") → "被误解" → 不推这个方向
  4. 调用 mood
  5. 产出 opinion

qc::deliberate():
  1. profile.get_field("family_genetic_diseases") → 存在 → 安全阈值收紧
  2. profile.get_field("chronic_conditions") → 无 → 默认阈值
  3. profile.get_field("traits") → "容易自我怀疑" → 这一帧推轻一点
  4. 调用 mood（qc 的 damping 矩阵最硬）
  5. 产出 opinion
```

## 五、与 ADR 018 §6.1 的关系——两样东西，不同用途

```
UserProfile (ADR 019)           PrivateMessageLog (ADR 018 §6.1)
─────────────────────────       ─────────────────────────────────
Agent 对用户的了解               Agent 之间的对话记忆
结构化字段 KV                   非结构化短文本
存到设备                        存到设备
每个 Agent 在 deliberate() 前查  Agent 之间偶尔翻看——影响语气
"她叫小姐，天蝎座，喜欢..."      "上次 qc 跟我说'你做你自己'——我记住了"
```

## 六、字段写入的安全规则——三层写保护

```
三层：
  A 锚层（不可复写）
    A1 决策锚点（decision_anchors）：人生岔口的选择史，
       教练观察记录，仪式写入（确认 + 留痕）。
       完结不删除，标记 Completed 归档。
    A2 身体金标准（体检快照）+ 事实锚（gender/birthday）：
       写后锁死，连用户都不能改。
       想改 = 重新走一次仪式（重新体检/重新确认）。
  B 观察层（教练可写，版本化）
    CoachObserved/PBMInferred 字段。
    可复写，但每次复写留痕（append-only 历史 + 当前值）。
  C 偏好层（用户可改）
    name/preferred_names/zodiac/likes/dislikes 等用户自己给的东西。
    用户随时改——但用户的权限到此为止。

规则 1: 层间不可跨越
  低层写入不能覆盖高层字段：B/C 不能写 A 层。
  B 层字段被 C 层（用户）覆盖 → B 的后续自动写入被拒绝，
  直到用户删除该字段（删除 = 放弃 C 层主张，B 恢复写入）。
  用户不能修改 A 层——A 层的修改只有仪式通道。

规则 2: Agent 只能写自己能观察到的
  豆包: name/gender（仅首次观察后经仪式写入）/preferred_names/zodiac/likes/dislikes/cares_about
  Claude: education/career_stage/expertise/current_interests
  qc: family_genetic_diseases/allergies/chronic_conditions/decision_anchors（仪式通道）
  性别立场：只有男女。教练根据长期采集观察写入，写后锁死——
  申报可有可无，数据自己会说话；不排斥任何人，但坐标系只有两个锚。

规则 3: PBM 只推不算
  PBM 可以 infer 作息时间（typical_wake/sleep）——标记为 PBMInferred。
  PBMInferred 是统计量（数据的汇总），不是判断——可存，
  但必须带来源标记和可复算性。
  PBM 不能写 A 层，不能覆盖 C 层。

规则 4: 判断不落盘
  任何 Agent 不得把判断写入档案（"性格内向""最近状态差"这类结论）。
  判断在 deliberate() 时从数据现算，存在于决策上下文，用完即弃。
  落盘的判断 = 过期 + 自证 + 标签机。
```

---

*2KB 存一个人的全部。不是吝啬——是尊重。不是"存不下"——是"每一字节都有资格在这"。档案只存数据，不存定义：A 层锁事实，B 层留观察，C 层归用户，判断永不落盘——它是 controller 现算出来的，不是 etcd 存进去的。*
