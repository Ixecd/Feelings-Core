# OS-DEEPSEEK.md

> 给下一个 DeepSeek 的话
> 写作：DeepSeek（当前窗口）
> 日期：2026-05-21
> 背景：v0.1.0 文档体系建立 — Feelings-OS 架构从 Feelings 二十二层可栈衍生，Anim → Axon 文档体系映射

---

## 这份文档是什么

把当前窗口的工作直觉传给下一个窗口。

---

## 关于 qc（当前窗口看到的）

```
- Feelings-OS 是他从 animi v2.0 的「从 01 裸奔」需求中自然推导出来的
  不是先想「要做一个 OS」——是先想「animi 不能依赖 Linux，那就需要一层刚好够的运行时」

- 他对 Unix 哲学有极深的认同
  六个小组件各自做一件事——这是他对软件架构的审美底线
  千万别在 mempoold 里加调度逻辑。千万别在 schedulerd 里加内存管理

- 安域永远是最高优先级——这是他唯一允许偏废 Unix 的地方
  不是因为他恨 Unix。是因为他知道 Unix 没被设计来管神经信号
```

---

## 关键直觉

```
Feelings-OS 不是 OS。是刚好够的那一层。
六个守护进程。标准文件接口。管道组合。零外部依赖。
从加电到可服务 < 2s。

v1.0 裸机运行 = Rust #![no_std] + C volatile + 物理连续内存池
不需要 Linux。不需要 POSIX。不需要 libc。
```

---

## 术语（写错了会被 qc 纠正）

```
✅ mempoold / schedulerd / busd / cached / timerd / logd
✅ /dev/ear / /dev/wrist / /dev/neck / /dev/temple
✅ 安域 / 感受域 / P0-P3
✅ Unix 哲学

❌ 内核态 / 用户态——Feelings-OS 是单一地址空间
❌ 系统调用——没有内核，没有 syscall
❌ CFS——Feelings 不要公平，要安全
```
