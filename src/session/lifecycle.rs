// Feelings-Core — Session 生命周期管理
//
// Session: start → 帧循环 → end/abort
// 每帧: tracker.intake_and_verify() → cross_dim_coupling() → cooldown check
// Session 结束: PBM 偏移 + tracker 状态写回 CoreConfig

use crate::pbm::PbmDimension;

/// Session 标识符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId(pub u64);

/// Session 配置——启动时加载。
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// 当前 user_cap——来自 PBM 或 CoreConfig。
    pub user_cap: u32,
    /// 帧窗口硬上限（帧数）。单维度连续高频注入不得超此值。0 = 无限制。
    pub max_frame_window: u32,
    /// DefenceLevel——来自 Core PBM。None = 普通用户。
    pub defence_level: Option<crate::pbm::DefenceLevel>,
    /// 锚点置信度 R (0-1)。R<0.3→cap 硬上限 20。
    pub anchor_confidence: Option<f64>,
}

/// Session 运行阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    /// 初始化——加载 FSIR + CoreConfig。
    Init,
    /// 帧循环中——每 1ms 一帧。
    Running,
    /// 正常结束——用户主动退出。PBM 全量更新。
    Ended,
    /// 异常终止——熔断触发。PBM 仅更新安全阈值。
    Aborted,
}

/// Session 管理器——持有本次 Session 的全部运行时状态。
pub struct Session {
    pub id: SessionId,
    pub phase: SessionPhase,
    pub config: SessionConfig,
    /// 每维度当前帧窗口计数——超出 max_frame_window 则强制降级。
    frame_windows: [u32; 4],
}

impl Session {
    pub fn new(config: SessionConfig) -> Self {
        Session {
            id: SessionId(0),
            phase: SessionPhase::Init,
            config,
            frame_windows: [0; 4],
        }
    }

    /// 帧窗口递增——达到上限返回 true（触发降级）。
    pub fn tick_frame_window(&mut self, dim: PbmDimension) -> bool {
        let idx = crate::pbm::dim_index(dim);
        if self.config.max_frame_window == 0 {
            return false;
        }
        self.frame_windows[idx] = self.frame_windows[idx].saturating_add(1);
        self.frame_windows[idx] >= self.config.max_frame_window
    }

    /// 重置指定维度的帧窗口（恢复帧到来后）。
    pub fn reset_frame_window(&mut self, dim: PbmDimension) {
        self.frame_windows[crate::pbm::dim_index(dim)] = 0;
    }

    /// 低锚点置信度硬上限——R<0.3 → cap = 20。
    pub fn effective_cap(&self) -> u32 {
        match self.config.anchor_confidence {
            Some(r) if r < 0.3 => self.config.user_cap.min(20),
            _ => self.config.user_cap,
        }
    }
}
