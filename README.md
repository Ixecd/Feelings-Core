## 部署

Feelings-Core 不打包为 Docker 镜像。不部署在云端。不运行在 Kubernetes 上。

Core 编译为单个 Rust 静态二进制——作为 Feelings-OS 的独立进程运行在设备本地：

```
Feelings-OS 六守护进程:
  mempoold  → 内存池管理
  cached    → 四层缓存
  busd      → 总线驱动
  timerd    → PLL 全局主时钟
  logd      → 审计日志
  → Core    → 读 FSIR + PBM → PSIR/DSIR/ESIR (本进程)

Core 和 Anim 的关系:
  Anim (编译器)  → .anim → FSIR → 磁盘文件
  Core (运行时)  → 读 FSIR → PBM 校准 → ESIR 帧 → busd → FPGA
```

不联网。不在云端。数据不离设备。
