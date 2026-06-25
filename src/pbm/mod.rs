// Feelings-Core — PBM 模块
// 基线矩阵：ColdStartGuard / DampingState / SessionLabel / DataConfidence / DefenceLevel

pub mod convergence;
pub mod persist;
pub mod state;

pub use state::*;
