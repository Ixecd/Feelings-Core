# OS-HANDOFF — Feelings-OS v0.1.0

> 编写日期：2026-05-21
> Last release: v0.1.0（架构设计阶段）
> Total commits: 0
> Co-Authored-By: DeepSeek

---

## 一、项目定位

**Feelings-OS** 是 animi v2.0 从 01 裸奔的底座。六个守护进程、一切皆文件、Unix 哲学 x 硬实时双调度。

**技术选型**：
- **Rust** — 上层守护进程（mempoold / schedulerd / busd / cached / timerd / logd）
- **C** — 底层硬件驱动（ear.c / wrist.c / neck.c / temple.c / bus.c）
- **单一地址空间** — 无内核态用户态切换，物理连续内存池
- **上游依赖** — Feelings（全栈架构）、Anim（交织器进程）、FPGA（硬件层）

---

## 二、现在能做什么

### 架构设计

- `Feelings-OS.md` 已完成——完整 OS 架构定义
- 六个守护进程、/dev 文件体系、内存池、双调度域、总线驱动、缓存集成全部定稿
- 启动序列、Unix 哲学对齐、与 animi 的接口全部明确

### 代码

- **零行。** 仓库尚未初始化。

---

## 三、开发约束

### 双层语言约定

Rust 上层不碰 volatile 寄存器。C 驱动不碰所有权模型。FFI 边界：Rust → extern "C" fn → C 驱动。C 驱动 → 共享帧缓冲区 → Rust 上层。

### 设计先行

新组件 → 先补 `Feelings-OS.md` 规范 → 再写 `docs/design/<component>.md` → 拍板 → 实施。

### 依赖审查

新增 crate → 对照 DEPENDENCY_POLICY.md 三级判定。

### 错误记录

重复 ≥ 2 次的错误 → MISTAKES.md。

---

## 四、当前工作流

```bash
# 尚未初始化仓库。预计流程：
cargo new feelings-os
make build         # cargo build + cc drivers/*.c
make test          # cargo test + driver unit tests
make lint          # cargo clippy -- -D warnings + cargo fmt --check
```

---

## 五、文档地图

```
Feelings-OS/
  OS-MEMORY.md (AI 搭档入口索引 — 第一时间读这个)
  OS-HANDOFF.md / OS-ROADMAP.md / OS-SNAPSHOT.md / OS-FORGET.md
  OS-FUTURE.md / OS-DEPENDENCY_POLICY.md / OS-MISTAKES.md
  OS-PHILOSOPHY.md / OS-README.md

上级项目（Feelings）：
  ../Feelings-OS.md     — OS 完整架构
  ../docs/cache-architecture.md — 四层缓存
  ../docs/device-architecture.md — 设备架构
```

---

## 六、联系

```
作者:    qc (Ixecd)
许可:    MIT
上层:    Feelings 项目 — github.com/Ixecd/Feelings
```
