# ADR 015: 缓存分层——KVCache + Priority-LRU + 内存池接口

> 状态：定稿（Q1/Q2/Q3 已拍，算法已修正）
> 日期：2026-06-25
> [Core] KVCache 条目定义、CacheLayer trait、Priority-LRU 驱逐、ThreadLocalCache（L1）、内存池接口 归属 Core。L2 共享缓存和 L3 分布式池归属 feelings-server。
> 对应：Feelings/docs/architecture/cache-architecture.md（全栈缓存架构）

---

## 〇、先定前提

Core 负责缓存的逻辑定义——条目长什么样、驱逐怎么决策、trait 怎么抽象。Core 不做的事：物理内存分配（那是 feelingsd 的启动逻辑）、跨线程锁（L2 是 feelings-server 的事）、分布式共识（L3）。

> 性质：Core 层的缓存逻辑 + trait——不是全栈缓存实现
> 核心：定义四类 KVCache 条目的驱逐策略，LRU 只用于低优先级，高优先级永不踢

---

## 一、Core 的缓存层级——只做 L1

对照 `cache-architecture.md` 的四层：

```
L0  FPGA 寄存器      → 硬件，不归 Core
L1  线程级 KVCache    → Core 实现 ← 这篇文档的范围
L2  节点共享缓存      → feelings-server 实现，Core 定义 trait
L3  分布式存储池      → feelings-server 实现
```

L1 的特性决定 Core 的实现约束：
- 单线程独占——不需要 `Mutex`/`RwLock`
- MB 级别容量——不需要 swap 到磁盘
- 条目数小（百到千级）——O(n) 驱逐扫描够用，不需要复杂的 LRU 链表

---

## 二、KVCache 条目——数据模型

### 2.1 CacheKey

```rust
/// 缓存键——复合键，类型前缀 + ID + 版本号
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CacheKey {
    pub namespace: CacheNamespace,  // feeling_atom / safety_rule / pbm_row / esir_frame
    pub id: u64,                     // atom_id / session_id / frame_index
    pub version: u32,                // 来源版本号
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum CacheNamespace {
    FeelingAtom,   // Pattern Registry 热条目
    SafetyRule,    // 安全规则索引
    PbmRow,        // PBM 活跃行
    EsirFrame,     // 预交织 ESIR 帧
    FsirCompiled,  // 预编译 FSIR
    SessionMeta,   // 会话元数据
}
```

### 2.2 CacheEntry

```rust
/// 缓存条目——最小调度单元
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub key: CacheKey,
    pub value: bytes::Bytes,     // 零拷贝 Arc 胖指针——L1→L2 下沉只需复制 24B
    pub priority: CachePriority,
    pub ttl_ms: Option<u32>,
    pub created_at: u64,
    pub last_access: u64,
    pub access_count: u64,
}

/// 驱逐优先级——1=最低（最先被踢），5=最高（永不驱逐）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CachePriority(u8);

impl CachePriority {
    pub const LOWEST:    u8 = 1;  // 历史 session 数据，可交换到磁盘
    pub const LOW:       u8 = 2;  // Registry 热条目，LRU
    pub const MEDIUM:    u8 = 3;  // PBM 活跃行，TTL + LRU
    pub const HIGH:      u8 = 4;  // 当前 PSIR，永不驱逐
    pub const CRITICAL:  u8 = 5;  // 安全规则索引，永不驱逐

    pub const fn new(p: u8) -> Self { CachePriority(p.min(5).max(1)) }
    pub fn is_pinned(&self) -> bool { self.0 >= 4 }
}
```

---

## 三、驱逐策略——Priority-first, LRU-second

通用 OS 的 LRU 不管内容。Feelings 的驱逐按两级决策：

```
驱逐决策树:
  cache 满了？
    → 找最小 priority 的条目
    → 同一 priority 里找最久未访问的（LRU）
    → 如果该 priority 的条目有 TTL 且已过期 → 优先驱逐
    → 踢出去
    → 如果所有条目都是 pinned（priority >= 4）→ 拒绝插入，返回错误
```

**为什么不是纯 LRU**：安全规则 priority 5 可能八百年不访问——但它在 cache 里的意义不是"最近用没用"，是"下一秒会不会有熔断需求"。LRU 看不到这个维度。

**为什么不是纯 priority**：同一 priority 里，最近被访问过的条目比旧条目的热度高——这是 LRU 的唯一用武之地——只在同 priority 内做 tie-break。

### 3.1 驱逐算法——全局过期优先

```
驱逐决策树（已修正——全局过期扫描优先于分级 LRU）:

  cache 满了？
    → Step 1: 全局扫描 priority 1→3，有 TTL 过期的→踢出去
              （防止 prio 1 新条目被踢而 prio 2 死数据霸占缓存）
    → Step 2: 优先踢最低 priority，同 priority 内 LRU
    → 如果所有条目都是 pinned（priority >= 4）→ 拒绝插入

为什么 Step 1 必须在前:
  反例：prio 1 有一个 5ms 前插入的新条目；
       prio 2 有一个 TTL 已超时 30s 的死条目。
       如果先按 prio 做 LRU → prio 1 的新条目会被踢，死数据留在 prio 2。
       全局过期优先→死数据先死，活着的数据才参与 LRU 竞争。
```

```rust
fn evict_one(&mut self) -> Option<CacheEntry> {
    let now = monotonic_ns();

    // Step 1: 全局过期扫描——从低到高，过期的一律先死
    for prio in 1u8..=3u8 {
        if let Some(idx) = self.find_expired_at(prio, now) {
            return Some(self.entries.swap_remove(idx));
        }
    }

    // Step 2: 硬驱逐——只在没有过期数据时才走 Priority-LRU
    for prio in 1u8..=3u8 {
        if let Some(idx) = self.find_lru_at(prio) {
            return Some(self.entries.swap_remove(idx));
        }
    }

    None // 全 pinned，拒绝驱逐
}
```

---

## 四、CacheLayer trait——Core 定义，各层实现

```rust
/// 缓存操作结果
#[derive(Debug, Clone)]
pub struct CacheResult {
    pub entry: CacheEntry,
    pub hit: bool,           // true = 命中，false = miss 后回填
    pub source_layer: u8,   // 0-3，数据实际来源——用于延迟统计
}

/// 缓存层抽象——Core 定义接口
pub trait CacheLayer {
    /// 查询缓存。返回命中条目或 None。
    /// 命中时更新 last_access。
    fn get(&mut self, key: &CacheKey) -> Option<&CacheEntry>;

    /// 插入或更新。如果 cache 满 → 驱逐一个 → 插入。
    /// 返回被驱逐的条目（如果有）——调用方直接 cascade 到下一层：
    /// ```ignore
    /// if let Some(evicted) = l1.put(entry) { l2.put(evicted); }
    /// ```
    fn put(&mut self, entry: CacheEntry) -> Option<CacheEntry>;

    /// 显式失效——安全相关用，不靠 TTL 自动过期。
    fn invalidate(&mut self, key: &CacheKey) -> Option<CacheEntry>;

    /// 当前条目数
    fn len(&self) -> usize;

    /// 容量上限
    fn capacity(&self) -> usize;
}
```

**设计决策——为什么 trait 方法用 `&mut self`**：

```
Q: L1 是单线程的，为什么还要 &mut self 不用内部可变性？

A: 三个理由。
1. ThreadLocalCache 不需要锁——&mut self 是自然的，Rust 编译器保证无竞争。
2. 内部可变性（RefCell）只为了绕过编译器检查——单线程场景不需要。
3. 如果未来 L2 需要 Arc<RwLock<dyn CacheLayer>>，实现方自己在 RwLock 里拿 &mut——trait 不需要管。
```

---

## 五、ThreadLocalCache——L1 实现（Core 提供）

### 5.1 存储后端——Vec ↔ FxHashMap 自动切换

Cache 底层不为 Vec 或 HashMap 二选一——它自适应容量。

```
capacity <= 128    Vec<CacheEntry>     线性扫描 ~100ns，CPU 缓存局部性极佳
capacity > 128     FxHashMap           哈希查找 O(1)，千级条目时防塌陷
```

理由：百级条目下 Vec 连续内存的 cache line 局部性完胜 HashMap 的指针跳转。超过 128 后分支预测失败概率暴增——FxHashMap 的非加密哈希（fxhash）在保证 O(1) 的同时不引入 DoS 攻击面（L1 是单线程的，key 来自可信代码）。

```rust
enum CacheStorage {
    Vec { entries: Vec<CacheEntry> },
    Map { entries: FxHashMap<CacheKey, CacheEntry> },
}
```

`get/put/invalidate` 内部 match `self.storage`，两条路径各自实现。用宏消除重复——或者用泛型 trait 的 static dispatch。

### 5.2 ThreadLocalCache 完整实现

```rust
pub struct ThreadLocalCache {
    storage: CacheStorage,
    capacity: usize,
}

impl ThreadLocalCache {
    pub fn new(capacity: usize) -> Self {
        let storage = if capacity <= 128 {
            CacheStorage::Vec { entries: Vec::with_capacity(capacity) }
        } else {
            CacheStorage::Map { entries: FxHashMap::with_capacity_and_hasher(
                capacity, BuildHasherDefault::default()) }
        };
        ThreadLocalCache { storage, capacity }
    }
}

impl CacheLayer for ThreadLocalCache {
    fn get(&mut self, key: &CacheKey) -> Option<&CacheEntry> {
        match &mut self.storage {
            CacheStorage::Vec { entries } => {
                entries.iter_mut().find(|e| &e.key == key).map(|e| {
                    e.last_access = monotonic_ns();
                    &*e
                })
            }
            CacheStorage::Map { entries } => {
                entries.get_mut(key).map(|e| {
                    e.last_access = monotonic_ns();
                    &*e
                })
            }
        }
    }

    fn put(&mut self, entry: CacheEntry) -> Option<CacheEntry> {
        match &mut self.storage {
            CacheStorage::Vec { entries } => {
                if let Some(existing) = entries.iter_mut().find(|e| e.key == entry.key) {
                    return Some(std::mem::replace(existing, entry));
                }
                if entries.len() >= self.capacity {
                    self.evict_one();
                }
                entries.push(entry);
                None
            }
            CacheStorage::Map { entries } => {
                if entries.len() >= self.capacity && !entries.contains_key(&entry.key) {
                    self.evict_one();
                }
                entries.insert(entry.key.clone(), entry)
            }
        }
    }
    // ...
}
```

---

## 六、内存池接口——下一个 ADR

`cache-architecture.md` 把内存池作为地基。当前 Core 不需要完整的内存池——Vec 的分配由 allocator 处理。但如果要支持 L2 共享缓存或固定内存区域，需要一个池抽象：

```rust
/// 内存池——预分配固定大小 block，零运行期分配
pub trait MemoryPool {
    /// 分配一个 block
    fn alloc(&self, size: BlockSize) -> Option<PoolPtr>;
    /// 归还 block（缓存驱逐时调用）
    fn free(&self, ptr: PoolPtr);
}

pub enum BlockSize { B4K, B64K, B1M, B16M }
pub struct PoolPtr(u64);  // opaque handle
```

内存池留给 ADR 016。当前 L1 的实现不依赖它——`Vec<CacheEntry>` 的堆分配对 v0.x 足够了。

---

## 七、与 Anim 的边界

```
Core 负责:
  - CacheKey / CacheEntry / CachePriority
  - CacheLayer trait
  - ThreadLocalCache（Vec-based L1 实现）
  - 驱逐策略（priority-first, LRU-second）

Anim 负责:
  - FSIR 缓存的 key 设计（FSIR 文件的 key 由 Anim 的 Registry 管理）
  - 安全规则索引的 key 设计

feelings-server 负责:
  - L2 NodeSharedCache（HashMap + RwLock 版本）
  - L3 DistributedPoolCache
```

---

## 八、设计问题（已拍板）

### Q1: `CacheEntry.value` 是 `Vec<u8>` 还是泛型？

```
A: bytes::Bytes（最终方案） 拒绝泛型——避免 CacheLayer 单态化膨胀
                            Bytes 是 Arc 胖指针（24B），L1→L2 下沉零深拷贝
                            调用方 serde 反序列化由自己决定——高频帧可缓
                            存解包后的结构体引用，低频率条目走按需 deserialize
B: 泛型 CacheEntry<V>        类型膨胀 + 关联类型地狱——已拒绝
```

决定：A。引入 `bytes` crate（~0 额外依赖，tokio 生态标准）。

### Q2: 单线程 L1 真的不需要考虑并发吗？

```
A: 不需要    坚决。&mut self = Rust 编译期独占借用 = 最完美的"无锁锁"。
             L2 的实现方自己用 RwLock<HashMap<...>>，在 write() guard 里
             拿到 &mut self，底层 L1 接口保持纯粹——给上层留最大并发自由度。
```

### Q3: 驱逐时要不要回调通知调用方？

```
A: 不回调     put() 返回 Option<CacheEntry>——调用方用线性管道处理级联下沉：
             if let Some(evicted) = l1.put(entry) { l2.put(evicted); }
             不引入闭包——避免 Send+Sync+'static 生命周期绑架。
```

---

## 九、新依赖

```
bytes       1.x     Bytes 零拷贝胖指针——Arc<[u8]> 语义，L1→L2 下沉零深拷贝
rustc-hash  0.x     FxHashMap——Firefox 的非加密哈希，容量>128 时防性能塌陷
```

两者都是 tokio/servo 生态标准，编译时间忽略不计。

---

*缓存不是越大越好。是每一层都知道自己在守着什么——安全规则永不踢，热数据自然沉在顶层，冷数据自然沉到底层。驱逐不是惩罚——是层级呼吸。*
