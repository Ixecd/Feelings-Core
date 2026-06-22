// src/tracker/mod.rs — ADR 011: NeuroEnergyTracker 四维漏桶
//
// 时域能量积分与生物漏桶安全。Session 级状态——每帧 intake_and_verify()。
//
// 当前逻辑在 Anim src/safety.rs 里——迁移后移入本模块。

pub mod bucket;
pub mod coupling;
