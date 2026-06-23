// Feelings-Core — 跨 Session 冷却
//
// ADR 009 §十.9 约束二: 同维度高强度感受包 ≥ 6h 间隔。
// 跨 Session 违规计数 → DefenceLevel 升级桥接。
//
// v0.1: 骨架。完整实现待 Core v0.3+。

/// 同维度冷却跟踪器。
#[derive(Debug, Clone, Default)]
pub struct CooldownTracker {
    /// 每维度最后一次高强度 Session 的时间戳（ns）。
    _last_high_intensity_ns: [u64; 4],
}
