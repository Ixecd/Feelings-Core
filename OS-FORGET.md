# OS-FORGET.md — 待修复项（P0 + P1）

> 扫描日期：2026-05-21
> 范围：架构规范（`Feelings-OS.md`）+ 代码（零行）
> 原则：只列 P0（生产命门）和 P1（功能受限）

---

## P0 — 生产命门（OS 不存在就无法工作）— 0/6

1. **mempoold 零代码** — 内存池四级预分配 + 空闲链表未实现。FIXME: v0.2。

2. **schedulerd 零代码** — P0 安域硬实时抢占调度器未实现。FIXME: v0.3。

3. **busd 零代码** — 总线驱动 + 主时钟分发未实现。FIXME: v0.2（总线骨架）+ v0.4（设备驱动）。

4. **timerd 零代码** — PLL 锁相时钟源未实现。FIXME: v0.2。

5. **/dev 文件接口零代码** — 设备注册 + open/read/write/close 未实现。FIXME: v0.2。

6. **C 驱动层零代码** — ear.c / wrist.c / neck.c / temple.c / bus.c 全部未实现。FIXME: v0.4。

---

## P1 — 功能受限（规模化前必做）— 0/4

7. **cached 零代码** — L0 BRAM 分区 + L1/L2 映射未实现。FIXME: v0.5。

8. **帧缓冲区零拷贝零设计** — mmap 页切换机制未落地。FIXME: v0.4。

9. **中断向量表零配置** — P0 安域最高优先级未在硬件层配置。FIXME: v0.3。

10. **animi 集成零验证** — animi 作为独立进程在 Feelings-OS 上运行未测试。FIXME: v0.5。

---

## 编辑记录

```
2026-05-21  v0.1 初始扫描
            - 架构规范 100%
            - 代码 0%
            - P0 6 项：mempoold / schedulerd / busd / timerd / dev接口 / C驱动
            - P1 4 项：cached / fbuf / 中断向量表 / animi集成
```
