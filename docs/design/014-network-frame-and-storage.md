# ADR 014: 网络帧协议 + PBM 存储接口——Core 边界定义

> 状态：定稿（Q1/Q2/Q3 已拍）
> 日期：2026-06-25
> [Core] 二进制帧格式、帧编解码 trait、PBM 存储 trait、epoll 风格事件源 归属 Core。网络传输实现（WSS/QUIC）归属 feelingsd/feelings-server 独立 crate。
> 对应：Feelings/docs/design/network-protocol-implementation.md（上层架构）、P0#1 PbmStore 已落地

---

## 〇、先定前提

```
Core 的边界 = 纯逻辑 + 数据模型 + trait 定义。
不做 I/O，不做网络，不引入 tokio。
Core 定义"数据长什么样"——feelingsd 决定"数据怎么传"。
```

> 性质：Core 层的接口定义——给了 feelingsd 和 feelings-server 一个"合约"
> 核心：Frame 编解码 + PBM 存储 trait + Session 同步语义

---

## 一、二进制帧——Core 侧数据模型

### 1.1 Rust 类型

```rust
/// 帧类型——Core 定义，网络层透传
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    DataFsir      = 0x0,
    DataPsir      = 0x1,
    DataDsir      = 0x2,
    DataEsir      = 0x3,
    SignalSafety  = 0x4,  // 最高优先级
    SignalHeartbeat = 0x5,
    MetaSession   = 0x6,
    MetaConfig    = 0x7,
    ReportIntrospection = 0x8,
    Ack           = 0x9,
}

/// 帧头——固定 16 字节
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct FrameHeader {
    pub version_type: u8,   // high 4b = version, low 4b = type
    pub stream_id: u16,     // 逻辑流标识
    pub payload_len: [u8; 3], // 大端，最大 16MB
    pub reserved: u8,
    pub timestamp_ns: u64,  // 设备采集时刻（纳秒）
    pub checksum: u16,      // header + payload 的 CRC-16/XMODEM
}
// size_of::<FrameHeader>() == 16

/// 完整帧——拥有 payload
#[derive(Debug, Clone)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}
```

### 1.2 Frame 工具方法（Core 实现）

```rust
impl FrameHeader {
    pub fn version(&self) -> u8 { self.version_type >> 4 }
    pub fn frame_type(&self) -> FrameType {
        FrameType::from_u8(self.version_type & 0x0F)
    }
    pub fn payload_len(&self) -> usize {
        u32::from_be_bytes([0, self.payload_len[0], self.payload_len[1], self.payload_len[2]]) as usize
    }
    pub fn set_payload_len(&mut self, len: usize) {
        let bytes = (len as u32).to_be_bytes();
        self.payload_len[0] = bytes[1];
        self.payload_len[1] = bytes[2];
        self.payload_len[2] = bytes[3];
    }
    pub fn compute_checksum(&mut self, payload: &[u8]) {
        self.checksum = 0;
        let header_bytes: &[u8; 16] = unsafe { std::mem::transmute(self) };
        self.checksum = crc16_xmodem(header_bytes, payload);
    }
}

impl Frame {
    pub fn new(frame_type: FrameType, stream_id: u16, timestamp_ns: u64, payload: Vec<u8>) -> Self
    pub fn verify(&self) -> bool // checksum 验证
}
```

### 1.3 设计决策——为什么不用 protobuf/prost

```
Q: protobuf 不是更标准吗？

A: 不是。理由如下——

1. 感受帧每 1ms 一帧（1000 fps）。
   protobuf varint 编码开销 → 每帧多 ~10μs CPU
   1000fps × 10μs = 10ms/s → 吃掉 1% 的 CPU 给序列化
   固定二进制头 → 每条指令就是一次 memcpy（~50ns）

2. Core 不引入 protobuf 依赖。
   Core 的依赖目前只有 serde。加 prost 会带进来
   protobuf codegen + bytes + 编译时步骤。
   固定头 16 字节 + memcpy——不需要 codegen。

3. v1.0 QUIC 切换时，可以在 feelingsd 加一层
   Frame → protobuf → gRPC 转换——Core 不用变。
```

### 1.4 帧优先级——高优先级跳过排队

```
SignalSafety  0x4    高优先级，直接发送，不排队       熔断信号必须零延迟
Ack           0x9    高优先级                         确认帧不排队
DataEsir      0x3    中优先级（下行感受注入）         感受不能等
DataPsir      0x1    中优先级                         上行信号
DataFsir      0x0    正常优先级                       采集帧
其他          0xX    低优先级，可以批量、延迟          心跳/报告
```

Core 提供优先级判断方法，网络层决定发送顺序：

```rust
impl Frame {
    pub fn priority(&self) -> u8 {
        match self.header.frame_type() {
            FrameType::SignalSafety | FrameType::Ack => 0,  // 最高
            FrameType::DataEsir => 1,
            FrameType::DataPsir | FrameType::DataFsir => 2,
            _ => 3,  // 最低
        }
    }
}
```

---

## 二、FrameCodec trait——Core 定义，网络层实现

### 2.1 trait 定义

```rust
/// 帧编解码——Core 定义接口，feelingsd/feelings-server 各自实现
pub trait FrameCodec {
    /// 编码 Frame 为 wire bytes（含 header + payload + checksum）
    fn encode(frame: &Frame, buf: &mut Vec<u8>);

    /// 从字节流解码——返回 None 表示数据不完整（等更多字节）
    fn decode(buf: &[u8]) -> Option<(Frame, usize)>; // (frame, consumed)
}
```

### 2.2 默认实现——标准编码器

Core 提供一个默认实现 `WireCodec`，网络层可以直接用。底层是 `memcpy` + CRC-16：

```
encode:
  [header 16B | payload 0..N B]
  header.checksum = crc16(header + payload)
  memcpy(header) + memcpy(payload) → buf

decode:
  至少需要 16 字节（header）
  读 payload_len → 检查 buf 是否有 header + payload
  验证 checksum → 返回 Frame 或 None（等更多数据）
```

### 2.3 为什么 Core 提供 trait + 默认实现

```
- Core 不引入 bytes crate、不引入 tokio
- Core 的默认实现用 Vec<u8>，零依赖
- feelingsd 如果需要 zero-copy（bytes crate），
  它可以实现自己的 FrameCodec——trait 约束保证
  帧格式一致性
```

---

## 三、PBM 存储 trait ——从具体类型抽接口

### 3.1 当前状态

P0#1 实现了 `PbmStore`——具体类型，直接写 JSON 文件到 `~/.feelings/pbm/`。

问题：服务器侧需要存数据库（SQLite/Postgres），不能用一个硬编码的本地文件 store。

### 3.2 trait 提取

```rust
/// PBM 持久化——Core 定义接口，本地/服务器各自实现
pub trait PbmStorage {
    /// 保存一次 session 快照
    fn save_snapshot(&self, snapshot: &PbmSnapshot) -> Result<(), PbmStoreError>;

    /// 加载最近一次快照
    fn load_latest(&self) -> Result<Option<PbmSnapshot>, PbmStoreError>;

    /// 列出某个物种的所有 session
    fn list_sessions(&self, species: &str) -> Result<Vec<PbmSnapshot>, PbmStoreError>;
}

#[derive(Debug)]
pub enum PbmStoreError {
    NotFound,
    InvalidData(String),
    Io(std::io::Error),
}
```

### 3.3 两个实现

```
JsonFileStore    实现 PbmStorage    ~/.feelings/pbm/ JSON    边缘 feelingsd 用
SqliteStore      实现 PbmStorage    SQLite 单文件.db         服务器 feelings-server 用
PostgresStore    实现 PbmStorage    PostgreSQL (v1.0+)       多用户/多租户
```

### 3.4 SessionManager 改签名

当前 `SessionManager` 持有 `PbmStore`（具体类型）。改后：

```rust
pub struct SessionManager<const D: usize, S: FeelingTarget, Store: PbmStorage> {
    pub session: Session<D, S>,
    store: Store,        // ← 泛型了
    guard: ColdStartGuard,
    damping: DampingState,
    total_sessions: u32,
}
```

**影响**：`SessionManager::new()` 的调用方需要传一个 `impl PbmStorage`。对于边缘 feelingsd，传 `JsonFileStore::new(species)`。对于服务器，传 `SqliteStore::new(conn, species)`。

---

## 四、Session 同步语义——epoll 风格事件触发

### 4.1 设计：LT/ET 双模事件源

Core 不定义"怎么传"，但定义"什么时候该传"。用事件源模型——类似 epoll 的 `EPOLLIN`/`EPOLLOUT`：

- **LT（level-triggered）**：事件发生了就一直报，直到被 ack。适合"必须处理"的场景（安全熔断）
- **ET（edge-triggered）**：状态变化时触发一次，取走后不重复。适合"通知一下就够了"（session 结束）

SessionManager 在 `tick()` / `end_session()` / `handle_safety_breach()` 内部 `signal()` 相应事件位。网络层（feelingsd）在自己的 event loop 里 `pending_events()` 或 `take_events()`——完全解耦。

### 4.2 事件位定义

```
SESSION_STARTED   SessionManager 新建，第一帧即将开始
FRAME_BATCH_READY 积压 N 帧，该推送给服务器了（默认每 50 帧 = 50ms）
SESSION_ENDED     Session 正常结束，需要上传最终 PBM 快照
SAFETY_BREACH     发生了熔断——最高优先级，零延迟推送
RECONNECT_SYNC    断线重连后，本地有未同步的 session 积压
PBM_DIRTY         DampingState 有变更，应该持久化（本地或 push 到服务器）
```

### 4.3 离线积压与 RECONNECT_SYNC

```
SessionManager 断网期间继续跑——本地 PbmStorage 一直写
重连后：
  1. PbmStorage::list_sessions() → 找未同步的
  2. SessionManager 调用 source.signal(RECONNECT_SYNC)
  3. 网络层在 LT 循环里看到 RECONNECT_SYNC → 批量推 PbmSnapshot
  4. 推送完成 → source.ack(RECONNECT_SYNC)
```

---

## 五、数据库——下一个 ADR 的话题

本文档不选数据库，只定义接口。但以下是已明确的方向：

```
边缘 (feelingsd)     SQLite 单文件     零配置，嵌入式，与 feelingsd 进程同生命周期
服务器 (v0.x)        SQLite 单文件     原型期零运维，Rust crate rusqlite 或 sqlx
服务器 (v1.0+)       PostgreSQL        多用户、连接池、备份、主从
```

下一个 ADR（015）将专门覆盖：数据库 schema 设计、迁移策略、PbmStorage 的 SQL 实现。

---

## 六、与 Anim 的边界

```
Core 负责：
  - Frame 数据模型 + 编解码 trait
  - PbmStorage trait
  - SessionManager 同步 trigger

Anim 负责：
  - ESIR 帧的 payload 格式（Core 的 payload 是 opaque bytes）
  - ESIR→执行设备的序列化（I2S/PWM/压电驱动）

feelingsd（新 crate）负责：
  - tokio-serial 串口读取
  - tokio-tungstenite WSS client
  - FrameCodec 网络传输实现
  - SyncTrigger → 实际推送策略

feelings-server（新 crate）负责：
  - axum WSS acceptor
  - Frame 路由 + 转发
  - SqliteStore 实现
  - JWT 认证 + TLS
```

---

## 七、实施顺序

```
v0.4 (现状)    P0#1 PbmStore 具体实现完成
               P0#2 SessionManager 具体实现完成

v0.5 (本 ADR)  src/frame.rs     —— Frame + FrameHeader + FrameCodec trait + WireCodec
               src/pbm/store.rs —— PbmStorage trait + JsonFileStore impl
               SessionManager<D, S, Store> 泛型化
               Makefile 加入 ensure_no_tokio 检查（Core 不依赖 tokio）

v0.6+          feelingsd crate（独立）—— 串口读取 + WSS client
               feelings-server crate（独立）—— WSS acceptor + SqliteStore
               见 Feelings/docs/design/network-protocol-implementation.md §七
```

---

## 八、设计问题（待拍板）

### Q1: `PbmStorage` trait 的 save_snapshot 是同步还是异步？

```
A: 同步（当前方案）    Core 不引入 tokio，SessionManager 同步调用 store.save()
                       feelingsd/sqlite 层的同步写足够快（<1ms）
B: 异步                Core 需要引入 async_trait，SessionManager 变 async
```

建议：v0.x 走 A——同步写。JSON 文件写盘 ~100μs，SQLite INSERT ~500μs——都在 1ms 以内，不会阻塞帧循环（1ms/帧）。v1.0 引入 delta buffer 批量异步写入——累积 N 帧后一次 flush，减少 I/O 次数。

### Q2: Frame 的 payload 要不要压缩？

```
A: 不压缩   帧很小（PSIR payload ~40B），压缩开销 > 收益
B: 压缩     服务器→客户端下行 ESIR 可能较大（多设备同步），压缩有意义
```

决定：v0.x 不压缩。为了最终体验 v1.0 必须上压缩——但不是 Core 层的事，在 QUIC 流级别由 quinn 处理或 feelingsd 加一层 Snappy/Zstd。

### Q3: SyncTrigger——epoll 风格事件触发（LT/ET 双模）

原方案用 enum 返回——太"拉"。改成事件源模型，类似 epoll 的 `EPOLLIN`/`EPOLLOUT`：

```rust
use std::sync::atomic::{AtomicU64, Ordering};

/// 同步事件位掩码——每个 bit 代表一种事件类型
#[derive(Debug, Clone, Copy)]
pub struct SyncEventMask(u64);

impl SyncEventMask {
    pub const SESSION_STARTED  : u64 = 1 << 0;
    pub const FRAME_BATCH_READY: u64 = 1 << 1;   // 积压 N 帧，该推了
    pub const SESSION_ENDED    : u64 = 1 << 2;
    pub const SAFETY_BREACH    : u64 = 1 << 3;
    pub const RECONNECT_SYNC   : u64 = 1 << 4;   // 重连后积压 session
    pub const PBM_DIRTY        : u64 = 1 << 5;   // PBM 有变更待持久化

    pub fn contains(&self, other: SyncEventMask) -> bool {
        self.0 & other.0 != 0
    }
}

/// 事件源——类似 epoll fd。
/// SessionManager 内部持有，网络层订阅。
pub struct SyncEventSource {
    /// 当前待处理事件（LT 语义——不清除直到被 ack）
    pending: AtomicU64,
}

impl SyncEventSource {
    /// LT 模式：查询当前所有未处理事件。
    /// 不会清除——调用方看到事件后必须 ack() 才能关掉。
    /// 类似 epoll 的 level-triggered：只要事件还在，每次查询都返回。
    pub fn pending_events(&self) -> SyncEventMask {
        SyncEventMask(self.pending.load(Ordering::Acquire))
    }

    /// ET 模式：取出新事件并原子清除。
    /// 类似 epoll 的 edge-triggered：只通知一次，
    /// 不管调用方有没有处理完——状态变更只触发一次信号。
    pub fn take_events(&self) -> SyncEventMask {
        SyncEventMask(self.pending.swap(0, Ordering::AcqRel))
    }

    /// 确认某些事件已处理（LT 模式用）。
    /// 清除指定位——如果所有位都清 0，网络层知道可以 idle 了。
    pub fn ack(&self, mask: SyncEventMask) {
        self.pending.fetch_and(!mask.0, Ordering::Release);
    }

    /// 触发事件——SessionManager 内部调用。
    pub(crate) fn signal(&self, mask: SyncEventMask) {
        self.pending.fetch_or(mask.0, Ordering::Release);
    }
}
```

**LT vs ET 使用场景**：

```
LT (level-triggered):
  feelingsd 主循环每 tick 调 pending_events()
  如果 SAFETY_BREACH 还在 → 每 tick 都看到 → 推 WSS → ack 后消失
  适合"必须处理，不处理就一直烦你"

ET (edge-triggered):
  feelingsd 用 take_events() 在专用线程/epoll 循环里等
  session 结束 → 触发一次 SESSION_ENDED → take 走 → 不再重复
  适合"触发一次就够了，不用反复提醒"
```

**与 push/poll 的本质区别**：

```
旧方案 (enum 返回):
  tick() -> Option<SyncTrigger>  // 每次 tick 可能返回一个 enum
  调用方必须每次 tick 后检查返回值——耦合在调用节奏里

新方案 (事件源):
  tick() 内部 signal(event)
  调用方在任意时机 pending_events()/take_events()
  解耦——SessionManager 不需要知道"谁在看、什么时候看"
```

**为什么不用 channel**：`tokio::mpsc` 不在 Core 的依赖里。`AtomicU64` 是 `core::sync` 自带的——零依赖，零分配。feelingsd 可以在自己的 tokio 循环里 `interval.tick()` 后调 `pending_events()`，或者用 `tokio::spawn` 单独跑一个 event loop 调 `take_events()`。

---

*Core 定义了数据的形状和事件的信号。网络层决定怎么流动。边界清晰了，两边的迭代互不阻塞。*
