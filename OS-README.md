# Feelings-OS

> **Unix 哲学 x 硬实时双调度** — animi v2.0 从 01 裸奔的底座。

---

## 这是什么

Feelings-OS 是 Feelings 设备的极薄运行时。不是 Linux。不是 RTOS。是刚好够不跟硬件打架、刚好少到不拖慢任何实时路径的一层。

```
六个独立进程
    mempoold    内存池——零动态分配，零碎片，零 GC
    schedulerd  双调度域——P0 安域硬实时 > P1 感受域软实时 > P2 I/O > P3 后台
    busd        总线驱动——主设备时钟广播 + 从设备轮询
    cached      四层缓存——L0 BRAM → L1 线程 → L2 节点 → L3 分布式
    timerd      PLL 全局主时钟——所有设备共享同一个时钟源
    logd        审计日志——追加写入，哈希链锚定

一切皆文件
    /dev/ear        迷走神经耳支刺激 + 心率采集
    /dev/wrist      皮肤电导采集 + 温度控制
    /dev/neck       本体感受采集 + 低频振动
    /dev/temple     EEG 采集 + 认知状态信号
    /dev/companion  飞行陪伴体
    /dev/safety     安域状态
    /dev/mempool    内存池状态
    /dev/cache      缓存命中率/驱逐率
```

---

## 和 Unix 的关系

Unix 哲学——一个东西只做一件事、一切皆文件、小工具管道组合——被 Feelings-OS 完整继承。唯一偏离的地方：Unix 说一切平等，Feelings-OS 说安域永远是最高优先级。这是 Feelings 自己的哲学。

---

## 和 Linux 的关系

Feelings-OS 不是 Linux。不管虚拟内存。不管文件系统。不管 shell。不管登录。只有六个守护进程 + `/dev` 文件接口 + 严格优先级抢占调度器。从加电到可服务 < 2s。

---

## 在 Feelings 全栈里的位置

```
层 0-4   电子 → 离散 → 机器码      FPGA 物理。Feelings-OS 不碰。
层 5     固件与实时系统              busd 总线驱动 + FPGA 寄存器直写
层 6     设备硬件                   /dev/ear /dev/wrist /dev/neck /dev/temple
层 7     信号处理                   PLL 锁相 + 信号滤波
层 8     交织管线                   animi 八 Pass——在 Feelings-OS 上作为独立进程
层 9     类型系统                   animi Pass 1——受 mempoold 内存保护
层 10    数据架构                   cached + mempoold——L1/L2 缓存物理管理
```

---

## License

MIT License.

> Made by qc (Ixecd), for Feelings.
