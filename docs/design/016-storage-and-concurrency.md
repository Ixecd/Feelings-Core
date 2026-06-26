# ADR 016: 存储全景 + 并发模型 + 自研替代

> 状态：定稿（Q1/Q2 已拍，四个盲区已修）
> 日期：2026-06-25
> [Core] PbmStorage trait 映射到具体存储。并发模型：事件驱动 + Delta Buffer + M:N 协程。
> [feelings-server] 数据库六件套。自研 KVCache 替代 Redis。channel 背压策略。
> 对应：Feelings/docs/architecture/tech-architecture.md §九（存储架构）

---

## 〇、先定前提

```
Core 不引入数据库——只定义 PbmStorage trait。
feelingsd（边缘）→ SQLite
feelings-server（服务器）→ PostgreSQL + TimescaleDB + NATS + MinIO + etcd
Redis → 自研 KVCache 替代（ADR 015 的 ThreadLocalCache + NodeSharedCache 覆盖全部需求）

并发原则：
  锁不是瓶颈 → RwLock<HashMap> 够用
  channel 是背压 → 不逐条 push，走 Delta Buffer 批量 merge
```

> 性质：Core trait → 具体存储的映射 + feelings-server 并发模型
> 核心：自研替代 Redis + Delta Buffer 取代逐条 channel + M:N 协程调度

---

## 一、数据库全景——六件套

### 1.1 每层对应

```
feelingsd (边缘)
  └─ SQLite 单文件              PbmStorage trait 的本地实现
     存：PBM 快照 / session 历史 / 离线积压队列
     零运维，与 feelingsd 进程同生命周期

feelings-server (服务器)
  ├─ PostgreSQL                主库——结构化业务数据
  │   用户/感受包/分成/安全事件/session 元数据
  │
  ├─ TimescaleDB               pg 扩展——时序数据（v1.0+）
  │   个人基准时间序列 / temporal_snapshots
  │
  ├─ MinIO                     对象存储（S3 兼容）
  │   AI 生成音乐 / 固件二进制 / 用户导出包
  │
  ├─ etcd                      分布式状态
  │   设备注册 / 服务发现 / 分布式锁
  │   KubePivot 已集成，不动
  │
  └─ KVCache（自研）          替代 Redis
      缓存 + 实时状态（详见 §三）

v1.0 追加:
  ├─ NATS                      跨服务流式处理（v1.0+）
  │   多 feelings-server 节点间的感受曲线流 / AI 教练事件流
  └─ TimescaleDB               时序数据（在 pg 已有基础上加扩展）
```

### 1.2 为什么 NATS 推迟到 v1.0

```
KubePivot 验证了: 零外部消息队列足够支撑 K8s controller 级别的
事件吞吐——三层 in-process 队列（chan + sync.Mutex + atomic）
比引入 NATS 少一个进程、少一套运维、多一层已验证的工程经验。

v0.x 的帧分发走同一个模式:
  帧到达 → DeltaBuffer 攒 → WorkerPool 非阻塞 drop + 令牌桶限流
  → Worker 协程幂等处理 → 结果回写 PbmStorage

v1.0 多 feelings-server 节点时才需要 NATS——
跨节点的感受曲线流 / AI 教练事件流 / JetStream KV 配置分发。
单节点时代，in-process queue 完全够。
```

### 1.3 为什么不要 TiDB

```
TiDB 的价值区间       分布式 OLTP，跨地域多活，HTAP 混合负载
Feelings 的查询模式   按 user_id 查 session / 按时间范围查快照
                      全是单 key 或简单范围——pg + 好索引足够

TiDB 的代价            PD + TiKV + TiDB 三个进程
                      运维负担 ← 撞 power-wall
                      和"刚好够"互斥
```

### 1.4 SQL → PostgreSQL 切换路径

```
v0.x     SQLite 单文件（边缘 + 服务器原型）
         零运维，与 feelingsd 同生命周期

v1.0     服务器切 PostgreSQL
         迁移工具：PbmStorage trait 的 list_sessions() + save_snapshot()
         导出 SQLite → JSON → INSERT into pg
         不丢数据

v1.0+    TimescaleDB（在 pg 已有基础上加扩展）
         CREATE EXTENSION timescaledb;
         temporal_snapshots 表变 hypertable——零代码改动
```

---

## 二、并发模型——事件驱动 + Delta Buffer + M:N 协程

### 2.1 两件已知的事

```
1. 锁不是瓶颈
   RwLock<HashMap<...>> 的临界区 < 1μs
   L1 ThreadLocalCache 根本不进锁（&mut self）
   不需要 lock-free 结构——写 lock-free 代码的脑力
   不如花在"怎么让冲突本身不发生"上

2. channel 是背压
   tokio::mpsc 在高速 burst 下堆积 → 内存涨 → latency spike
   逐条 push 到 channel → 消费者来不及消费 → 队列爆
   不是 channel 实现不好——是"逐条 push"这个模式本身
   在 burst 下就是排队
```

### 2.2 Delta Buffer 批量 merge

不逐条 push——攒一批，一次写：

```rust
/// 写缓冲——取代逐条 channel push
pub struct DeltaBuffer<T> {
    pending: Vec<T>,
    capacity: usize,
    flush_threshold: usize,
}

impl<T> DeltaBuffer<T> {
    /// 追加一条变更——不 push，只是攒着
    pub fn push(&mut self, item: T) {
        self.pending.push(item);
    }

    /// 是否需要 flush？
    pub fn should_flush(&self) -> bool {
        self.pending.len() >= self.flush_threshold
    }

    /// 批量取出所有积压的变更
    pub fn flush(&mut self) -> Vec<T> {
        std::mem::take(&mut self.pending)
    }
}
```

**应用场景**：

```
PBM 写缓冲:
  SessionManager 每帧更新 damping_state
    → DeltaBuffer::push(delta)    // 攒着
    → 每 50 帧 → flush → PbmStore::save_snapshot()  // 批量写

Cache 驱逐级联:
  L1 驱逐 → DeltaBuffer 攒着
    → should_flush? → 批量 push 到 L2
    → 不是踢一个 push 一个——踢 10 个 push 一次

网络帧发送:
  PSIR 帧积压
    → DeltaBuffer 攒到 1KB 或 10ms
    → 一次 WSS writev → 少一次 syscall
```

### 2.3 M:N 协程模型——一队列一线程

```
不是：
  tokio 默认的 work-stealing（全局队列 + 随机窃取）
  → 跨线程窃取 → cache line 失效 → 隐藏延迟

而是：
  每个线程一个专用队列
  一个线程 → N 个协程（该线程上的 session）
  session 不跨线程迁移
  ThreadLocalCache 天然绑定（&mut self 保证）

为什么并发度不会降:

  一帧 SessionManager::tick() 全链路 <1μs:
    intake_and_verify    ~200ns
    cross_dim_check      ~300ns
    tick_frame_window    ~50ns
    DeltaBuffer::push    ~20ns
    ─────────────────────────────
    合计                  <1μs

  1000fps → 1ms CPU/s。一个核能跑几百个 session。
  线程迁移本身的开销 ~5μs > 一帧 tick 的 5 倍。
  把 session 锁在同一核上不是牺牲并发——是保护缓存局部性。

调度规则:
  feelingsd 主循环:
    serial_reader → parse_frame → SessionManager::tick()
    这些跑在一个线程上——无锁，无 channel，纯函数调用

  feelings-server:
    每个 CPU core 一个 worker 线程
    每个 worker 线程上跑 N 个协程（N 个活跃 session）
    SessionManager 在自己的线程上独占
    跨 session 共享的数据走 parking_lot::RwLock<HashMap>——极少冲突

每线程独立缓存:
  一队列一线程 → 每个线程拥有自己的 L1 ThreadLocalCache
  Thread A 的 session 查 PBM → 命中线程 A 的 L1 → 零跨核通信
  Thread B 的 session 查同一条 PBM 行 → 各自 L1 各自命中
  → 热点自然复制在多核上，没有 false sharing，没有一致性协议

  只有当 L1 miss 时才下沉到 L2（NodeSharedCache）。
  L2 的 parking_lot::RwLock 只在 L1 miss 时被碰——
  而 L1 命中率在 session 内 >95%（session 内反复查同一用户的 PBM 行）。
  → L2 的锁竞争 <0.01%。

  不是"共享缓存 + 锁保护"。
  是"每核独立缓存 + 共享缓存只做冷数据回填"。


任务粒度分流:

  帧关键路径（<1μs，必须在主线程同步完成）:
    intake → leak → cross_dim → push DeltaBuffer
    → 不能让帧间隙 >1ms——硬实时边界

  非关键路径（>100μs，溢出到计算线程池）:
    AOT 仿真 / ESIR 预交织 / 分支预测更新 / 磁盘 I/O
    → spawn_blocking 扔到专用计算线程池
    → 结果通过 oneshot 回写 L1 ThreadLocalCache

  主线程不干活——只做路由。轻量帧在主线上跑，
  重计算溢出到池里。session 仍然绑定核心——
  但它的重任务不绑。
```

### 2.4 为什么锁不是瓶颈——一个数值论证

```
场景: L2 NodeSharedCache
  容量: 10000 条目
  每次 get: RwLock::read().get(key) → HashMap 查找 ~20ns
  持有读锁时间: ~50ns（含函数调用开销）

  100 个并发 session × 每 session 10 次 get/s = 1000 次读/s
  1000 × 50ns = 50μs/s → 0.005% 的时间持有读锁

  写锁: 只在 cache miss + 回填时发生
  每秒 <10 次写 → 每次 ~1μs → 10μs/s → 0.001%

结论: RwLock 的 contention < 0.01% CPU——不是瓶颈。
      真正的瓶颈: burst 时 1000 帧/s 塞进一个 mpsc channel
      → 消费者跟不上 → 队列爆 → latency 成倍增长
```

---

## 三、自研 KVCache 替代 Redis

### 3.1 Redis 的哪些功能 Feelings 需要？

```
Sorted Set（ZADD/ZRANGE）      → 榜单 Top N      → BTreeMap<score, Vec<user_id>>，~50 行
Hash（HSET/HGET/HGETALL）     → 会话状态       → ThreadLocalCache（ADR 015），已设计
String（INCR/EXPIRE）          → 限流计数器     → AtomicU64 ring buffer，~30 行
Pub/Sub                        → 实时推送       → tokio::broadcast，一行
TTL                            → 会话自动过期   → CacheEntry.ttl_ms，已设计
Lua Scripting                  → 原子操作       → 不需要——单线程 L1 天然原子
Persistence（RDB/AOF）          → 数据持久化     → PbmStorage trait，已设计
Sentinel/Cluster               → 高可用         → v0.x 不需要，v1.0 靠 NATS KV + etcd
```

### 3.2 自研实现——少即是多

```rust
/// 榜单——BTreeMap 替代 Redis Sorted Set
pub struct Leaderboard {
    entries: BTreeMap<u64, Vec<Uuid>>,  // score → user_ids，天然有序
    user_index: HashMap<Uuid, u64>,       // user_id → score，O(1) 查排名
}

impl Leaderboard {
    pub fn insert(&mut self, user: Uuid, score: u64) {
        if let Some(&old_score) = self.user_index.get(&user) {
            if let Some(users) = self.entries.get_mut(&old_score) {
                users.retain(|u| u != &user);
            }
        }
        self.entries.entry(score).or_default().push(user);
        self.user_index.insert(user, score);
    }
    pub fn top_n(&self, n: usize) -> Vec<(Uuid, u64)> { /* iter */ }
}
// 50 行。不需要哨兵进程，不需要 fork，不需要 evalsha。
```

```rust
/// 限流——AtomicU64 时间窗 ring buffer
/// 每个 slot 编码：高 32 位 = tick_id（周期标识），低 32 位 = count。
/// 防止环绕时旧周期计数污染当前窗口。
pub struct RateLimiter {
    ring: Box<[AtomicU64]>,
    resolution_ms: u64,
    mask: usize,
}

impl RateLimiter {
    pub fn check(&self, now_ms: u64, limit: u32) -> bool {
        let tick = (now_ms / self.resolution_ms) as u32;
        let idx = tick as usize & self.mask;
        let encoded = self.ring[idx].load(Ordering::Relaxed);
        let slot_tick = (encoded >> 32) as u32;
        let count = (encoded & 0xFFFF_FFFF) as u32;

        if slot_tick != tick {
            return true; // 新周期——还没计数，肯定不超限
        }
        count < limit
    }

    pub fn incr(&self, now_ms: u64) {
        let tick = (now_ms / self.resolution_ms) as u32;
        let idx = tick as usize & self.mask;
        let new_encoded = ((tick as u64) << 32) | 1;

        loop {
            let current = self.ring[idx].load(Ordering::Relaxed);
            let slot_tick = (current >> 32) as u32;
            if slot_tick != tick {
                // 跨周期——原子重置
                match self.ring[idx].compare_exchange_weak(
                    current, new_encoded,
                    Ordering::Release, Ordering::Relaxed,
                ) {
                    Ok(_) => return,
                    Err(_) => continue, // CAS 失败——重试
                }
            } else {
                // 同周期——递增
                let new_val = current + 1;
                match self.ring[idx].compare_exchange_weak(
                    current, new_val,
                    Ordering::Release, Ordering::Relaxed,
                ) {
                    Ok(_) => return,
                    Err(_) => continue,
                }
            }
        }
    }
}
// ~50 行。不需要 Redis 网络往返。不需要哨兵 fork。不需要 evalsha。
```

### 3.3 为什么不用 Redis

```
1. 运维负担        Redis 进程 + 监控 + 内存碎片 + RDB fork latency spike
                  自研 200 行 Rust < 运维一个 Redis 实例

2. 网络往返        Redis::get() = TCP 往返 ~100μs
                  ThreadLocalCache::get() = HashMap 查找 ~20ns
                  快 5000 倍

3. 功能冗余        Redis 127 种数据结构 × Feelings 只用 4 种
                  90% 的功能是死代码——但运维负担不是死的

4. 内存效率        Redis 的 RESP 协议 + 内部数据结构开销 ~100B/entry
                  ThreadLocalCache::CacheEntry = 精确控制每条目字节数
```

---

## 四、Core PbmStorage trait 到具体存储的映射

```
PbmStorage trait        → LocalJsonStore     feelingsd（已有，v0.4 实现）
                        → SqliteStore        feelingsd/feelings-server（v0.5+）

服务器侧（v1.0）:
  PBM 快照              → PostgreSQL（user_baselines 表）
  Session 历史           → PostgreSQL（feeling_sessions 表）
  时序基准               → TimescaleDB（temporal_snapshots hypertable）
  安全事件               → PostgreSQL（safety_events 表）
```

这些是 feelings-server 的职责——Core 不需要知道底层是什么数据库。`PbmStorage` trait 的三个方法（save_snapshot / load_latest / list_sessions）对每种实现都一样。

---

## 五、channel 背压——Delta Buffer 具体策略

### 5.1 何处用 channel，何处用 Delta Buffer

```
用 channel 的地方:
  - tokio::broadcast → 事件广播（session_started / safety_breach）
    受众多，不需要排队——broadcast 是 fire-and-forget
  - oneshot → RPC 风格一问一答（"这个 user 的 PBM 最新版本是多少"）

用 Delta Buffer 的地方:
  - PBM 写 → 50 帧攒一次 flush
  - Cache 驱逐级联 → 10 条攒一次 push
  - PSIR 帧上行 → 1KB 或 10ms 攒一次 WSS writev
  - 指标/日志 → 1s 攒一次批量写

不用 channel 的地方:
  - SessionManager 和 CacheLayer 之间——它们是同一个线程上的函数调用
    不需要 channel。不需要队列。不需要序列化。
```

### 5.2 背压信号——不是等了才报，是主动告知

```rust
/// 背压信号——生产者收到后应减速
#[derive(Debug, Clone, Copy)]
pub enum Backpressure {
    Green,    // 正常，继续
    Yellow,   // 缓冲过半，建议降速——跳过非关键帧（heartbeat/报告）
    Red,      // 缓冲将满，强制降速——只接受 SignalSafety + DataEsir
}
```

DeltaBuffer 在 `should_flush()` 时检查自身深度，返回背压信号。生产方（SessionManager::tick）根据信号决定当前帧是否跳过非关键数据的写入。

**Red 级为什么保留对话帧而 KubePivot 可以丢 reconcile task**：

```
KubePivot: 丢的是一轮 reconcile——下轮 watch 自动补——状态是幂等的。
          AP 最终一致——丢了就丢了，watch 会拉回来。

Feelings:  对话帧是人在等。不能幂等重试。"等一下，卡了"是信任破裂。
          Red 级只接受 SignalSafety + DataEsir + 对话帧——
          不是这些帧比 reconcile task 更重——
          是这些帧对面有一个人在等。

不是"丢掉不重要"。
是"丢掉可以等的。保住有人在等的。"
```

---

## 六、微观硬核——四个必须回避的坑

### 6.1 阻塞逃逸阀——主线程上禁绝 >100μs 的非帧关键路径

```
问题:
  feelingsd 边缘用 SQLite。即使 WAL 模式，COMMIT 时磁盘 I/O 高 →
  OS 线程被挂起 → 绑定在该线程上的所有 N 个活跃 session 全部失响应
  → 物理帧积压 → 背压连锁反应

  不仅是磁盘 I/O——任何 >100μs 的非帧关键路径计算
  （AOT 仿真器膨胀到 300μs、ESIR 预交织变重、分支预测全量更新）
  都会堵死同线程上的帧循环。主线程不干活——只做路由。

治理:
  两条铁律——

  1. 帧关键路径（<1μs，主线程同步完成）:
     intake → leak → cross_dim → push DeltaBuffer
     → 必须在当前帧的 1ms 窗口内完成——不让帧间隙被撑破

  2. 非关键路径（>100μs，spawn 到计算线程池）:
     磁盘 I/O（SQLite COMMIT / JSON fsync）
     AOT 仿真 / ESIR 预交织 / 分支预测更新
     唤醒规则:
       spawn_blocking 或专用计算线程池执行
       → 结果通过 oneshot 通知主循环
       → 回写 ThreadLocalCache（无锁，主线程独占写入）

  主线上永远不碰同步磁盘 I/O，也永远不碰 >100μs 的 CPU 重计算。
  不是"怕慢"——是"一帧的慢会传染给所有同核的 session"。
```

```rust
/// 计算线程池——专门消化 >100μs 的非帧关键任务
/// SessionManager 不直接持有它——由 feelingsd/feelings-server 注入
pub fn spawn_compute<F, T>(f: F) -> oneshot::Receiver<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    tokio::task::spawn_blocking(move || {
        let result = f();
        let _ = tx.send(result);
    });
    rx
}
```

### 6.2 DeltaBuffer 的异步共享——`Rc<RefCell<>>` 而不是 `&mut self`

```
问题:
  同一个 Worker 线程上的多个协程需要写入同一个公共 Buffer
  （如 L2 Cache 级联队列、WSS 网络缓冲）。
  &mut self 在多个 .await 之间无法安全传递——Rust 借用检查不让。
  但这些协程在同一个 OS 线程上——不需要 Arc，不需要 Mutex。

治理:
  公共 DeltaBuffer 内部用 Rc<RefCell<DeltaBufferInner<T>>>。
  单线程无锁（一队列一线程，RefCell 的运行时检查是零成本的），
  多点写入不做跨线程同步——因为根本不会跨线程。
  Arc 留给真的需要跨线程的场景（如 WSS sender 在多 worker 上）。

  struct SharedDeltaBuffer<T> {
      inner: Rc<RefCell<DeltaBuffer<T>>>,
  }
```

### 6.3 TimescaleDB Hypertable——复合主键必须包含时间列

```
问题:
  create_hypertable 的硬限制——表上任何唯一约束必须包含时间分区列。
  如果原 pg 表主键是 PRIMARY KEY (session_id, seq) →
  转换时报错：cannot create a unique index without the partition column

治理:
  v0.x SQLite Schema 就要提前把 created_at 纳入复合主键:
    PRIMARY KEY (session_id, created_at, seq)
  或:
    不设整行唯一约束，只用普通索引 + 应用层去重
    但推荐前者——TimescaleDB 的 chunk 分区依赖时间列在唯一约束里
```

### 6.4 为什么锁不是瓶颈而 channel 是——完整论证

```
场景: 1000 frames/s burst

锁的路径:
  1000 次 parking_lot::RwLock::read() × 50ns = 50μs/s → 0.005% CPU
  锁内不做 .await → 无 Future 分配 → 无套娃

channel 的路径:
  1000 次 mpsc::Sender::send() × 每条分配 Future 对象 ~200ns = 200μs/s
  + 接收方唤醒开销 + 队列膨胀 + 内存碎片
  burst 到 10000/s → 2ms/s → 2% CPU → latency spike

不是 channel 的实现不好。是"逐条 push"的模式在 burst 下本质就是排队——
和你在哪个队列里排队无关——排队这件事本身就是背压。

DeltaBuffer 彻底消灭逐条 push:
  1000 次 push → 攒在 Vec 里（1000 × memcpy ~500ns 总开销）
  1 次 flush → 1 次批量处理
  不是 1000 条 channel message → 是 1 次函数调用
```

---

## 七、实施顺序

```
v0.4 (现状)    ✅ PbmStore 具体实现（JSON 文件）
               ✅ SessionManager 基础就绪
               ✅ ADR 015 KVCache 设计定稿

v0.5           PbmStorage trait 抽离（从 PbmStore 提取接口）
               JsonFileStore impl PbmStorage
               ThreadLocalCache 实现（ADR 015 落代码）
               DeltaBuffer + RateLimiter + Leaderboard
               引入 bytes + rustc-hash 依赖

v0.6           feelingsd SQLite PbmStorage 实现
                feelings-server 骨架（axum + WSS）
                NodeSharedCache（L2，parking_lot::RwLock<HashMap>）

v1.0           PostgreSQL + TimescaleDB + MinIO + NATS
               帧分发走 WorkerPool + DeltaBuffer（KubePivot 已验证的
               非阻塞 drop + 令牌桶限流 + 幂等去重模式）
```

---

## 八、设计问题（已拍板）

### Q1: DeltaBuffer 攒多少才 flush？

```
决定：多维复合阈值——数量 + 字节数 + 定时器，三者任一触发即 flush。

PBM 写缓冲    50 帧 或 Backpressure::Yellow → 立即 flush + PbmStore::save_snapshot
WSS 网络帧    1KB 或 10ms 定时器兜底 → 防止关键帧因后续无新帧被无限期滞留
Cache 驱逐    10 条 或 1KB → 批量级联到 L2
```

定时器兜底是关键——单条关键帧（如 SafetyBreach）不能因为"后面没新帧了"被永远困在缓冲区里。

```rust
pub struct DeltaBuffer<T> {
    pending: Vec<T>,
    capacity: usize,
    flush_count: usize,
    flush_bytes: usize,
    flush_interval: Duration,
    last_flush: Instant,
}

impl<T: AsRef<[u8]>> DeltaBuffer<T> {
    pub fn should_flush(&self) -> bool {
        self.pending.len() >= self.flush_count
            || self.pending.iter().map(|t| t.as_ref().len()).sum::<usize>() >= self.flush_bytes
            || self.last_flush.elapsed() >= self.flush_interval
    }
}
```

### Q2: L2 NodeSharedCache 用 RwLock 还是分片锁？

```
决定：parking_lot::RwLock，严禁 tokio::sync::RwLock。

理由：
  parking_lot 无冲突锁开销趋近 0，CPU 级自旋高效
  tokio::sync::RwLock 为跨 .await 持有设计——加锁解锁分配 Future 对象
  临界区 ~50ns，tokio 版本的开销 > 临界区本身——性能差一个数量级

铁律：
  持有 parking_lot::RwLock 的代码块内永远不能出现 .await
  一旦在同步锁守卫里让出 CPU → 同线程死锁
  一队列一线程模型天然满足——session 在自己的线程上跑，不进锁里等 IO

分片锁不搞：
  几乎没有跨核争抢（thread-per-core）
  分片增加了哈希计算 + 内存对齐开销——纯负收益
```

---

*锁不是瓶颈。channel 是。Delta Buffer 不是"优化"——是"让 burst 到来时不排队"。Redis 可以替代。200 行 Rust 可以覆盖 Redis 对 Feelings 有用的全部数据结构。不自研是因为懒——不是因为没有能力。*
