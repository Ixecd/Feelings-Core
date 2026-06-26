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
    // 元数据——Core 管理，Agent 不直接读写
    // ═══════════════════════════════════════════════════════════
    pub created_at: u64,
    pub updated_at: u64,
    pub source_map: HashMap<String, ProfileSource>, // field_name → 来源
}

pub enum ProfileSource {
    UserProvided,     // 用户自己填的——最高置信度
    CoachObserved,    // Agent 在交互中推理的——中置信度
    PBMInferred,      // 从生理数据推算的——低置信度（如作息从活跃时间推断）
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
gender: "男"/"male"/"M"         Gender: Male | Female | NonBinary | PreferNotToSay
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
  UserProvided 字段   → 永久有效    → 用户不主动更新就不变
                      "我叫什么"、"我是谁"——这些你自己说了算

实现——每个字段的 source_map entry 从 ProfileSource 升级为:
  (ProfileSource, u64)  // (来源, 首次写入的时间戳)

判断是否过期:
  PBMInferred 字段: now - timestamp > 30×86400×1e9 → 过期
  CoachObserved 字段: now - timestamp > 90×86400×1e9 → 过期
  UserProvided 字段: 永不过期

不额外储存 TTL 列——用来源类型映射到固定常量——省存储。
```

## 三、读写接口——KV 最少操作集

```rust
pub trait ProfileStore {
    /// 读取整个档案。首轮 deliberate 时加载。
    fn load(&self, user_id: &str) -> Result<UserProfile, ProfileError>;

    /// 写入单个字段——增量更新，不覆写全量。
    /// source 标记来源——UserProvided > CoachObserved > PBMInferred
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

    /// 删除字段——用户主动要求
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

  用户写入 → 先清空列表再写（用户是全量覆盖的最高权限）
    例: traits = ["坚韧", "内向", "有创造力"]
        用户重新填 traits = ["内心强大"]
        → 结果: ["内心强大"]——不是追加，是重置
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

## 六、字段写入的安全规则

```
规则 1: 来源优先级不可绕过
  某字段被 UserProvided 覆盖 → CoachObserved/PBMInferred 写入被拒绝。
  用户可以随时覆盖任何字段——用户是最高权限。
  用户删除某字段 → 所有更低来源的写入也同时被清。

规则 2: Agent 只能写自己能观察到的
  豆包: name/gender/preferred_names/zodiac/traits/likes/dislikes/cares_about
  Claude: education/career_stage/expertise/current_interests/afraid_of
  qc: family_genetic_diseases/allergies/chronic_conditions

规则 3: PBM 只推不算
  PBM 可以 infer 作息时间（typical_wake/sleep）——标记为 PBMInferred。
  PBM 不能直接修改 UserProvided 覆盖的字段。
```

---

*2KB 存一个人的全部。不是吝啬——是尊重。不是"存不下"——是"每一字节都有资格在这。"*
