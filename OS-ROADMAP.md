# Feelings-OS ROADMAP

> 创建日期：2026-05-21
> 当前版本：v0.1.0（架构设计阶段）
> 原则：架构设计先于代码。模糊想法 → [FUTURE.md](OS-FUTURE.md)

---

## v0.1.0 — 架构设计定稿（当前）

### 已完成

- `Feelings-OS.md` — OS 完整架构（上级项目根目录）
- 六个守护进程：mempoold / schedulerd / busd / cached / timerd / logd
- /dev 文件体系：ear / wrist / neck / temple / safety / mempool / cache
- 双调度域：P0 安域 > P1 感受域 > P2 I/O > P3 后台
- 内存池：Page / Chunk / Slab / Zone 四级预分配
- 四层缓存集成：L0 BRAM → L1 线程 → L2 节点 → L3 分布式
- Unix 哲学对齐：小组件 + 文件接口 + 管道组合
- 调度器偏离 Unix CFS 的明确声明——安域永远抢占
- 项目管理文档：OS-MEMORY / OS-README / OS-PHILOSOPHY / OS-HANDOFF / OS-ROADMAP / OS-SNAPSHOT / OS-FORGET / OS-FUTURE / OS-MISTAKES / OS-DEPENDENCY_POLICY / OS-CONVENTIONS / OS-DEEPSEEK

---

## v0.2 — Milestone 1: 最小启动骨架

### 目标

```
六个守护进程骨架全部跑通 → /dev 文件接口可读写 → 内存池可分配
```

### 核心交付

- `src/main.rs` — 初始化入口
- `src/mempoold.rs` — Page/Chunk 层级的预分配和空闲链表
- `src/dev.rs` — /dev 文件接口注册 + open/read/write/close
- `src/timerd.rs` — PLL 时钟信号源守护进程
- `src/logd.rs` — 审计日志追加写入
- `drivers/bus.c` — 总线协议骨架（主时钟脉冲生成）
- 启动序列：mempoold → cached → busd → timerd → schedulerd → Idle

### 验收

```
✓ cargo build 通过 + cc drivers/*.c 编译通过
✓ /dev/mempool read 返回内存池状态
✓ 启动后打印 "Feelings-OS v0.2.0 ready"
✓ 每个守护进程是独立进程——ps 可见
```

---

## v0.3 — Milestone 2: 调度器 + 中断

### 目标

```
P0 安域中断抢占 → P1 正常调度 → P2/P3 级联退避
```

### 核心交付

- `src/schedulerd.rs` — 四级优先级抢占调度器
- C 驱动层中断向量表配置——P0 安域最高优先级
- 抢占验证：P1 运行时收到 P0 中断 → 当前周期结束立即切换

### 验收

```
✓ P0 中断到达 < 100μs 内安域接管
✓ P1 帧级交织不被 P2/P3 打断
✓ P3 只在 Idle 时运行
```

---

## v0.4 — Milestone 3: 设备驱动 + 文件接口

### 目标

```
/dev/ear /dev/wrist /dev/neck /dev/temple 全部可读写
```

### 核心交付

- `drivers/ear.c` — 耳后设备驱动
- `drivers/wrist.c` — 腕部设备驱动
- `drivers/neck.c` — 后颈设备驱动
- `drivers/temple.c` — 颞部设备驱动
- 帧缓冲区 mmap——零拷贝读写

### 验收

```
✓ cat /dev/ear → 返回当前心率 + 温度 + 皮电值
✓ echo "freq:25" > /dev/ear → 耳后设备发射 25Hz 刺激
✓ 多设备同时读写——总线轮询周期 < 1ms
```

---

## v0.5 — Milestone 4: 缓存层 + animi 集成

### 目标

```
cached 管理 L0-L2 物理映射 → animi 进程在 Feelings-OS 上跑通一个 Session
```

### 核心交付

- `src/cached.rs` — L0 BRAM 分区 + L1/L2 映射
- animi 在 Feelings-OS 上作为独立进程运行
- 第一个端到端链路：/dev/ear → animi Pipeline → /dev/ear + /dev/wrist

### 验收

```
✓ animi Pass 6 (Personalize) 查询 L1 KVCache → 命中 < 100ns
✓ 一个完整 session 从 /dev 文件输入到 ESIR 帧输出
```

---

## v1.0 — 裸机运行

### 目标

```
Feelings-OS 不再依赖 Linux。直接运行在 FPGA 之上。
animi 完成自举——不再依赖 Rust 工具链。
从加电到可服务 < 2s。
```

---

## 版本号规则

```
v0.x      架构验证阶段，一切可变
v1.0      裸机运行
```
