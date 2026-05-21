# OS-MEMORY.md — Feelings-OS 项目入口索引

> 这是 AI 搭档打开项目后**第一时间读的文件**。
> 读完此文件 → 按文档地图索引到具体文档 → 建立完整项目心智模型。
> 最后更新：2026-05-21 | v0.1.0

---

## 一、项目身份（30 秒速览）

```
Feelings-OS = animi v2.0 从 01 裸奔的底座
核心理念：   六个小组件、一切皆文件、管道组合
            Unix 哲学 x 硬实时双调度
            P0 安域永远抢占——安全比 Unix 说得更重要
技术栈：     Rust（v0.x 寄居于 Feelings 设备 FPGA 之上）
             C 用于驱动层（总线驱动 + PLL 时钟）
当前版本：   v0.1.0 — 架构设计完成，代码零行
上级项目：   Feelings — 神经感受民主化平台
```

---

## 二、文档地图（按阅读顺序）

### 第一圈：核心理解（必读，按此顺序）

| 序号 | 文件 | 回答的问题 | 读完时长 |
|------|------|-----------|----------|
| 1 | `OS-MEMORY.md` | 我在哪？这个项目是什么？文档怎么找？ | 你正在读 |
| 2 | [README.md](./OS-README.md) | 项目对外介绍，定位和价值主张 | 3 min |
| 3 | [PHILOSOPHY.md](./OS-PHILOSOPHY.md) | 怎么写代码？Unix 哲学怎么落地？ | 10 min |
| 4 | [HANDOFF.md](./OS-HANDOFF.md) | 现在能做什么？开发约束？工作流？ | 5 min |
| 5 | [SNAPSHOT.md](./OS-SNAPSHOT.md) | 精确到 commit 的代码结构和测试状态 | 5 min |

### 第二圈：工程管理（知道项目怎么管）

| 文件 | 回答的问题 |
|------|-----------|
| [ROADMAP.md](./OS-ROADMAP.md) | 各个 milestone 分别交付什么？ |
| [FORGET.md](./OS-FORGET.md) | 什么东西还没做？（P0 命门 / P1 受限） |
| [FUTURE.md](./OS-FUTURE.md) | 什么种子现在不做、以后可能做？ |
| [DEPENDENCY_POLICY.md](./OS-DEPENDENCY_POLICY.md) | 新 crate 能引入吗？三级判定怎么走？ |
| [MISTAKES.md](./OS-MISTAKES.md) | 哪些坑踩过 ≥2 次？认知盲区在哪里？ |

### 第三圈：设计决策（理解为什么这么做）

| 文件 | 回答的问题 |
|------|-----------|
| 上级 `Feelings-OS.md` | Feelings-OS 完整架构——组件/dev/调度器/内存池/总线/缓存 |
| 上级 `docs/cache-architecture.md` | 四层缓存——L0 BRAM 到 L3 分布式 |
| 上级 `docs/device-architecture.md` | 设备架构——主设备星型拓扑 |

### 第四圈：AI 搭档专属

| 文件 | 回答的问题 |
|------|-----------|
| [DEEPSEEK.md](./OS-DEEPSEEK.md) | 上一个 DeepSeek 窗口留下了什么直觉？qc 是什么风格？ |

---

## 三、架构速览（一张图理解项目）

```
/dev/ear ──read──→ animi ──write──→ /dev/ear （迷走神经刺激）
/dev/wrist                                          /dev/neck
/dev/neck                                           /dev/wrist
/dev/temple                                         /dev/temple

                   ↑ schedulerd 调度这一切 ↑
                   P0 安域 > P1 感受域 > P2 I/O > P3 后台

mempoold ──→ 全量物理连续内存池 ──→ cached ──→ L0 BRAM / L1 线程 / L2 共享
timerd   ──→ PLL 全局主时钟 → busd ──→ 主设备广播 → 从设备锁定
logd     ──→ 审计日志追加写入 → 哈希链锚定
```

**六个独立进程。标准文件接口。管道组合。Unix 哲学 x 硬实时双调度。**

---

## 四、代码结构速查（v0.1.0 预计）

```
Feelings-OS/
├── Cargo.toml              Rust 项目配置（上层进程）
├── Makefile                 构建脚本
│
├── src/
│   ├── main.rs              入口——初始化六个守护进程
│   ├── mempoold.rs          内存池守护进程
│   ├── schedulerd.rs        双调度域守护进程
│   ├── busd.rs              总线驱动守护进程
│   ├── cached.rs            缓存层管理守护进程
│   ├── timerd.rs            PLL 时钟守护进程
│   ├── logd.rs              审计日志守护进程
│   ├── dev.rs               /dev 文件接口层
│   └── fbuf.rs              帧缓冲区（零拷贝 mmap 管道）
│
├── drivers/
│   ├── ear.c                耳后设备驱动（C——寄存器直写）
│   ├── wrist.c              腕部设备驱动
│   ├── neck.c               后颈设备驱动
│   ├── temple.c             颞部设备驱动
│   └── bus.c                总线协议——主时钟 + 从设备轮询
│
├── docs/
│   └── design/              设计文档（ADR）
│
├── OS-MEMORY.md             项目入口索引
├── OS-PHILOSOPHY.md         开发哲学
├── OS-CONVENTIONS.md        代码规范
├── OS-MISTAKES.md           错误日志
├── OS-FORGET.md             P0+P1 待修复
├── OS-DEPENDENCY_POLICY.md  依赖管理
├── OS-FUTURE.md             架构种子库
├── OS-ROADMAP.md            milestone 计划
├── OS-HANDOFF.md             接手指南
├── OS-DEEPSEEK.md           DeepSeek 窗口直觉传递
├── OS-SNAPSHOT.md           代码快照
└── OS-README.md
```

---

## 五、关键约定

### commit 规范

```
git commit -F commits/<file>.txt
格式: type(scope): description
subject 严格 ASCII
```

### 双层语言——Rust 上层 + C 驱动

```
Rust    守护进程层——mempoold / schedulerd / busd / cached / timerd / logd
        Cgo 零容忍——纯 Rust，零 unsafe（除非 FFI 到 C 驱动层）

C       硬件驱动层——ear.c / wrist.c / neck.c / temple.c / bus.c
        寄存器直写——volatile 指针映射 FPGA 寄存器
```

### 依赖引入

```
新增 crate → 查 DEPENDENCY_POLICY.md 三级判定
L0 禁止 / L1 受限需论证 / L2 CLI wrapper 直接通过
```

### 错误记录

```
犯两次 → MISTAKES.md 入册（根因+解法+计数）
```

---

## 六、当前状态（2026-05-21）

```
版本:    v0.1.0
提交:    0 commits（仓库尚未初始化）
Sprint:  架构设计完成 → 创建仓库 → 实现六个守护进程

完成度:
  架构设计    ████████████████████ 100%（上级 Feelings-OS.md）
  OS 代码     ░░░░░░░░░░░░░░░░░░░░   0%
  测试        ░░░░░░░░░░░░░░░░░░░░   0%
```

---

## 七、外部依赖

| 系统 | 关系 | 文档 |
|------|------|------|
| Feelings | 上级项目，定义全栈二十二层架构 | `../` |
| Feelings-OS.md | OS 完整架构规范 | `../Feelings-OS.md` |
| Anim | 交织器——在 Feelings-OS 上作为独立进程运行 | `../Anim/` |
| FPGA | L0 硬件层——BRAM 分区 + 总线寄存器 | `docs/cache-architecture.md` |

---

## 八、读完后你应该能回答

- [ ] Feelings-OS 是什么？为什么不是 Linux？
- [ ] 六个守护进程分别做什么？/dev/* 文件体系是什么？
- [ ] 双调度域的优先级顺序是什么？安域为什么永远抢占？
- [ ] Unix 哲学如何落地到实时系统？哪里偏离了？
- [ ] 下一个文件该读哪个？（按文档地图顺序）
