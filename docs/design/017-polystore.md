# ADR 017: PolyStore——编译期异构存储引擎

> 状态：定稿
> 日期：2026-06-25
> [Core] 异质容器——enum-based slot dispatch——每种数据类型用自己最优的物理结构
> [Core] 零虚函数，编译器 jump table 分发；零 trait object 间接跳
> 对应：ADR 015（缓存分层）、ADR 016（存储全景）、power-wall（刚好够）

---

## 〇、先定前提

一个 HashMap 统治全部数据类型 = O(1) 给了不需要的东西，O(n) 拿不到需要的有序遍历。不同数据的访问模式天然不同——强用同一种结构，要么浪费要么不够。

```
不是 一个数据结构解决所有问题
而是 每种数据用自己的最优物理结构——通过 enum dispatch 零成本选择
```

> 性质：编译期异构存储——不是运行时动态类型
> 核心：7 种 slot 类型 × 7 种数据访问模式 = PolyStore

---

## 一、七种数据 × 七种结构

Feelings 的数据天然分成七类，每类的访问模式完全不同：

```
数据类型              访问模式              最优结构           理由
──────────────────────────────────────────────────────────────────────
PBM 矩阵行            按 user_id 有序范围查    BTreeMap          范围扫描 O(log n + k)，顺序走键
Session 元数据        按 session_id 点查       FxHashMap         纯点查，O(1)，fxhash 够快
安全规则索引          按 priority + TTL 遍历   SkipList          驱逐时按优先级遍历，O(log n) 插入
设备拓扑              多对多关系 + 最短路径    DiGraph           邻接表，Dijkstra/BFS
时序基准              顺序追加 + 时间窗查询    RingBuffer        固定窗口，自动覆盖，零分配
ESIR 帧缓存           按 frame_index 顺序读    Vec               CPU cache 友好，DMA 连续
Pattern Registry      按 namespace 点查       FxHashMap         纯点查，编译后不变
```

**为什么不是全用 BTreeMap**：Session 点查不需要有序遍历——log n 比 fxhash 的常数慢。安全规则驱逐时需要按优先级有序走——HashMap 做不到。

**为什么不是全用 HashMap**：PBM 需要查"最近 N 次 session 的阻尼历史"——范围扫描。HashMap 没有顺序。

---

## 二、PolyStore——编译期异质容器

### 2.1 数据结构

```rust
use std::collections::BTreeMap;
use std::collections::HashMap;

/// 槽位类型——编译期确定，运行期零开销分发
pub enum DataSlot {
    /// BTreeMap: 有序范围查询——PBM 矩阵行 (user_key → PbmRow)
    Matrix(BTreeMap<UserKey, Vec<u8>>),
    /// FxHashMap: 纯点查——Session 元数据
    Session(FxHashMap<SessionId, SessionMeta>),
    /// SkipList: 有序遍历 + 快速插入——安全规则索引
    Safety(SkipList<SafetyRuleEntry>),
    /// 有向图: 多对多关系 + 最短路径——设备拓扑
    Topology(DiGraph<DeviceId, LinkWeight>),
    /// RingBuffer: 固定窗口顺序追加——时序基准快照
    Timeline(RingBuffer<BaselineSnapshot>),
    /// Vec: 连续内存顺序读——ESIR 帧缓存
    Frames(Vec<EsirFrame>),
    /// FxHashMap: 纯点查——Pattern Registry
    Registry(FxHashMap<AtomId, PatternDef>),
}

/// 槽位统计——每种结构的命中率和驱逐量
#[derive(Debug, Default)]
pub struct SlotStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub memory_bytes: usize,
}

/// 异构存储引擎——每个 slot 一种最优结构
pub struct PolyStore {
    slots: [DataSlot; 7],
    stats: [SlotStats; 7],
}
```

### 2.2 为什么 enum 而不是 trait object

```
trait object (Box<dyn StorageSlot>):
  ┌──────┐     ┌──────────────┐     ┌─────────────────┐
  │vtable│ --> │fn get(key)   │ --> │BTreeMap::get     │
  │ptr   │     │fn insert(k,v)│     │或 HashMap::get   │
  │data  │     │fn evict()    │     │或 Vec::binary_...│
  └──────┘     └──────────────┘     └─────────────────┘
  两次指针跳转。每次调用。

enum dispatch (match + 编译器 jump table):
  match &self.slots[idx] {
      DataSlot::Matrix(m) => m.get(key),
      DataSlot::Session(m) => m.get(key),
      ...
  }
  第一次访问: 查 jump table → 跳转 (~2 CPU cycles)
  后面的访问: 分支预测器记熟 → 0 cycles

  零虚函数表。零 trait object 间接跳。零堆分配。
  编译器把 match 编译成和虚函数分发一样速度的代码——
  但没有 vtable 的指针跳。
```

### 2.3 查询分发——零成本抽象

```rust
impl PolyStore {
    pub fn query_matrix(&self, key: &UserKey) -> Option<&Vec<u8>> {
        match &self.slots[0] {
            DataSlot::Matrix(m) => m.get(key),
            _ => unreachable!("slot 0 invariant"),
        }
    }

    pub fn query_session(&self, id: &SessionId) -> Option<&SessionMeta> {
        match &self.slots[1] {
            DataSlot::Session(m) => m.get(id),
            _ => unreachable!(),
        }
    }

    pub fn query_safety_rules(&self, min_priority: u8) -> impl Iterator<Item = &SafetyRuleEntry> {
        match &self.slots[2] {
            DataSlot::Safety(list) => list.range(min_priority..),
            _ => unreachable!(),
        }
    }

    pub fn shortest_path(
        &self, from: DeviceId, to: DeviceId,
    ) -> Option<Vec<DeviceId>> {
        match &self.slots[3] {
            DataSlot::Topology(g) => dijkstra(g, from, to),
            _ => unreachable!(),
        }
    }

    pub fn timeline_window(&self, now_ms: u64, window_ms: u64)
        -> impl Iterator<Item = &BaselineSnapshot>
    {
        match &self.slots[4] {
            DataSlot::Timeline(buf) => buf.range(now_ms - window_ms..now_ms),
            _ => unreachable!(),
        }
    }

    pub fn frame_at(&self, idx: usize) -> Option<&EsirFrame> {
        match &self.slots[5] {
            DataSlot::Frames(f) => f.get(idx),
            _ => unreachable!(),
        }
    }

    pub fn registry_atom(&self, id: &AtomId) -> Option<&PatternDef> {
        match &self.slots[6] {
            DataSlot::Registry(m) => m.get(id),
            _ => unreachable!(),
        }
    }
}
```

每个方法只匹配它有意义的 slot 索引。编译器把 match 编译成单次数组索引 + 跳转——不遍历 7 个分支。

---

## 三、每种 slot 的物理性质

### 3.1 Matrix slot——BTreeMap

```
为什么 BTree: PBM 矩阵按 user_id 有序。查询"最近 N 个 session"需要范围扫描。
               BTreeMap 的键是有序的——scan_next 就是一个迭代器 ++。

为什么不是 Hash: user_id 的有序范围在 HashMap 里需要全量遍历 + 排序。

物理性质:
  查找 O(log n)，范围扫描 O(log n + k)
  内存: 每条目 ~40B 额外开销（键 + 值指针 + 颜色标记）
  页大小: 6 × sizeof(entry) ≈ 6 × 64B = 384B B-tree 节点
```

### 3.2 Session / Registry slot——FxHashMap

```
为什么 FxHash: session_id 和 atom_id 的访问模式是纯点查——从不扫描。
               fxhash 是 Firefox 的非加密哈希——50% 比 SipHash 快。

为什么不是 crypto hash: 无 DoS 威胁（key 来自内部，不可信的外部不直接送 key）。
                       不需要哈希防碰撞攻击。

物理性质:
  查找 O(1)，无范围查询
  内存: 每条目 ~48B 额外开销
  负载因子: 0.75 触发扩容（2× 翻倍）
```

### 3.3 Safety slot——SkipList

```
为什么 SkipList: 安全规则按 priority 分层。驱逐时走"priority 最低的"——需要有序遍历。
                 SkipList 的插入和删除都是 O(log n)，与 BTreeMap 相同，
                 但实现更简单——不需要旋转和重着色。

为什么不是 BTree: 安全规则条目数少（<50 条）。
                  SkipList 的常数开销小——不需要维护平衡因子。
                  v0.x 甚至可以降级为有序 Vec + binary search。

v0.x 降级方案:
  Vec<SafetyRuleEntry> + 按 priority 排序
  insert 是 O(n)——但安全规则只在启动时插入，不是热路径
  查询是 binary_search O(log n)
  驱逐是 pop_前几个元素
  → 不需要 SkipList 的数据结构复杂度

  type SafetySlot = Vec<SafetyRuleEntry>;  // v0.x
  type SafetySlot = SkipList<SafetyRuleEntry>; // v1.0+（如果需要热插入）
```

### 3.4 Topology slot——DiGraph

```
为什么图: 设备连接是多对多的——耳后→后颈→腕部→颞部。最短延迟路径需要图算法。
          邻接表最省内存——设备和边数量都小（<100 设备）。

为什么不是关系型: "从耳后设备到颞部设备的最短延迟路径" 在 SQL 里要递归 CTE——
                  比邻接表 Dijkstra 慢两个数量级。

物理性质:
  空间 O(V + E)，V < 100
  最短路径 O((V + E) log V)
  图不频繁变——只在设备插拔时更新
```

### 3.5 Timeline slot——RingBuffer

```
为什么 RingBuffer: 时序基准只追加，不删除。固定窗口（如 30 天）。
                   窗口满了自动覆盖——零分配，零 GC。

为什么不是 Vec: 追加会导致扩容 + 拷贝。RingBuffer 预分配，永远不会扩容。

物理性质:
  容量: 30 天 × 24h × 60 帧/h = 43200 条
  内存: 43200 × sizeof(BaselineSnapshot) ≈ 43200 × 256B = 11MB
  追加 O(1)，查询 O(log n)（内部二分查找时间戳）
```

### 3.6 Frames slot——Vec

```
为什么 Vec: ESIR 帧按帧号顺序访问——连续内存让 CPU 预取器自动抓下一帧。
           不需要查找——帧号就是数组索引。

为什么不是 RingBuffer: 帧不需要覆盖——每帧都被推到设备。旧帧可以驱逐到磁盘。

物理性质:
  容量: 1 个 session 约 5000 帧
  内存: 5000 × sizeof(EsirFrame) ≈ 5000 × 128B = 640KB
  frame_at(idx) = 一次指针加法 + 偏移 → ~1ns
```

---

## 四、统一缓存 + 统一事件

虽然每个 slot 的底层结构不同，但它们共享同一套基础设施：

```
缓存层（ADR 015）:
  每个 DataSlot 都包裹在一个 ThreadLocalCache 里（只缓存热条目）。
  CacheLayer trait 对 BTreeMap 和 HashMap 的实现是一样的——get/put/invalidate。
  Priority-LRU 驱逐对上层的 slot dispatch 完全透明。

事件源（ADR 014）:
  slot 内部的变更（insert / evict / expire）→ signal SyncEventMask::PBM_DIRTY
  → feelingsd LT 循环捕获 → DeltaBuffer 批量 push → WSS 上行
  不需要每个 slot 自己写事件逻辑——PolyStore 统一 signal。
```

---

## 五、不需要的 crate

```
petgraph            v0.x 不需要。设备数 <10，手写邻接表。
                    type TopoGraph = HashMap<DeviceId, Vec<(DeviceId, u32)>>;  // from → [(to, weight)]
                    Dijkstra: 20 行实现。

crossbeam-skiplist  v0.x 不需要。安全规则有序 Vec 够用。
                    v1.0+ 再评估是否需要真正的并发跳表。
```

Polystore 的依赖仅为 `std::collections::BTreeMap` + `FxHashMap`——零新 crate。

---

## 六、与"一个 HashMap 统治一切"的对比

```
                全 HashMap          PolyStore
───────────────────────────────────────────────────
PBM 范围查询    O(n) 全量遍历      O(log n + k) BTree
Session 点查    O(1) ✅            O(1) ✅
安全规则驱逐    O(n) 排序          O(1) 有序 Vec 头
设备最短路径    无法表达            O(log V) Dijkstra
时序窗口        O(n) 遍历全量      O(log n + k) Ring
帧缓存          O(1) 但碎片化      O(1) 连续 cache 友好
Registry       O(1) ✅            O(1) ✅

不是 HashMap 不好——是"只用 HashMap"不好。
每种数据用自己的最优结构——刚好够，不多一层开销，不少一个 O(log n) 的有序查找。
```

---

## 七、与 power-wall 的同一件事

```
power-wall:       刚好够的热量 = 刚好不烫皮肤 = 刚好不慢过神经
PolyStore:        刚好够的结构 = 刚好不多一层间接跳 = 刚好不少一次 O(1) 查找
ADR 015 缓存:     刚好够的驱逐 = 刚好不踢安全规则 = 刚好不让冷数据占坑

同一个哲学，三层具象。
不是"优化到极致"——是"刚好不需要更多"。
```

---

*数据库在内部就是这么干的——B-tree 给范围、Hash 给点查、SkipList 给有序。只是它们把这个选择藏在 SQL 优化器后面。PolyStore 把它提到应用层——用 Rust enum 零成本分发。*
