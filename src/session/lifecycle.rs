// Feelings-Core — Session 生命周期
//
// start() / end() / abort() + 帧窗口计数器 + user_cap 管理 + DefenceLevel 实时切换
//
// v0.1: 类型定义骨架。完整实现待 Core v0.2+

/// Session 标识符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId(pub u64);

/// Session 配置。
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// 当前 Session 的 user_cap。
    pub user_cap: u32,
    /// 帧窗口硬上限（帧数）。0 = 无限制。
    pub max_frame_window: u32,
}
