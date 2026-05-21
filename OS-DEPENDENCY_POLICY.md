# Feelings-OS 依赖管理政策

> 创建日期：2026-05-21
> 状态：已生效

---

## 哲学根基

**Feelings-OS 的依赖原则：零外部依赖不是洁癖——是裸机运行的硬约束。**

v1.0 裸机运行意味着没有 Linux 内核、没有 POSIX API、没有 libc。任何外部 crate 或 C 库如果在裸机环境下不可用——就是死代码。

---

## 三级分级标准

### Level 0 — 禁止：依赖标准库或 POSIX

Linux 系统调用 / pthread / malloc-free 动态分配——裸机不需要这些。

```
当前状态：无 Level 0 依赖（代码零行）
未来候选：std::fs / std::net / std::thread 的全部路径——裸机不提供这些
替代：    Feelings-OS 自己实现 /dev 文件接口 + 帧缓冲区 + mempoold
```

### Level 1 — 受限：零 OS 依赖的 Rust no_std crate

必须满足：不依赖 libc，不依赖 alloc，可以编译为 `#![no_std]` 目标。

```
允许引入    核心：libcore（Rust 内建——无 OS 依赖）
            tracing（可观测性——通过 logd 输出而非 stdout）
审查        任何需要 `#[cfg(not(bare_metal))]` 条件编译的 crate
```

### Level 2 — 允许：外部二进制工具

通过 CLI wrapper 调用。Feelings-OS 不链接任何外部 SDK。

```
工具    用途
cargo   编译 Rust 上层
cc      编译 C 驱动
objcopy 生成裸机二进制
```

---

## v1.0 裸机的边界

Rust `#![no_std]` 目标。C 驱动无外部库。所有 I/O 通过 volatile 指针映射 FPGA 寄存器。内存管理通过 mempoold——不经过 libc malloc。
