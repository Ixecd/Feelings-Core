// Feelings-Core — 神经感受引擎
//
// Pass 6-8（Personalize / DeviceMap / CodeGen）
// + PBM 管理 + Session 管理 + NeuroEnergyTracker
//
// Anim 编译器输出 FSIR → Core 读 FSIR + PBM 基线 → PSIR/DSIR/ESIR
// Core 是运行时——操作个人数据——永不离设备
//
// License: MIT

pub mod pbm;
pub mod personalize;
pub mod tracker;
pub mod session;
pub mod dsir;
