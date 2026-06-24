// Feelings-Core — Session 生命周期管理（泛型）
//
// Session<const D, S> = 持有漏桶 + 用户档案 + 帧窗口的完整运行时状态。
// D = 维度数, S = 物种 (Human / Psittacine / ...)
//
// 帧级循环:
//   tracker.intake_and_verify() → verify_cross_dimension() → tick_frame_window()
// 熔断时:
//   SafetyBreach<CrossDimCoupling { source, .. }> → phase = Aborted
//   → 根据 source 精准下发 D3 对冲信号 (主动麻痹锚点——TODO)

use crate::pbm::DefenceLevel;
use crate::session::grounding::GroundingSignal;
use crate::species::FeelingTarget;
use crate::tracker::{NeuroEnergyTracker, UserSafetyProfile};
use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub user_cap: u32,
    pub max_frame_window: u32,
    pub defence_level: Option<DefenceLevel>,
    pub anchor_confidence: Option<f64>,
}

impl SessionConfig {
    pub fn default_human() -> Self {
        SessionConfig {
            user_cap: 100,
            max_frame_window: 5000,
            defence_level: None,
            anchor_confidence: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    Init,
    Running,
    Ended,
    Aborted,
}

pub struct Session<const D: usize, S: FeelingTarget> {
    pub id: SessionId,
    pub phase: SessionPhase,
    pub config: SessionConfig,
    pub tracker: NeuroEnergyTracker<D, S>,
    pub profile: UserSafetyProfile<D, S>,
    frame_windows: [u32; D],
    /// PersonalityAnchor 预留舱位——VSA 向量、风格倾向、性别偏置 (TODO v0.5)
    _personality_anchor: PhantomData<S>,
}

impl<const D: usize, S: FeelingTarget> Session<D, S> {
    pub fn new(
        id: SessionId,
        config: SessionConfig,
        profile: UserSafetyProfile<D, S>,
        tracker: NeuroEnergyTracker<D, S>,
    ) -> Self {
        debug_assert_eq!(
            D,
            S::DIM_COUNT,
            "Session dimension mismatch for species {}",
            S::species_name()
        );
        Session {
            id,
            phase: SessionPhase::Init,
            config,
            tracker,
            profile,
            frame_windows: [0; D],
            _personality_anchor: PhantomData,
        }
    }

    /// 帧窗口递增——维度由物种关联类型决定。
    pub fn tick_frame_window(&mut self, dim: S::Dimension) -> bool {
        let idx = S::dim_index(dim);
        if self.config.max_frame_window == 0 {
            return false;
        }
        self.frame_windows[idx] = self.frame_windows[idx].saturating_add(1);
        self.frame_windows[idx] >= self.config.max_frame_window
    }

    pub fn reset_frame_window(&mut self, dim: S::Dimension) {
        self.frame_windows[S::dim_index(dim)] = 0;
    }

    /// 低锚点置信度硬上限。
    pub fn effective_cap(&self) -> u32 {
        match self.config.anchor_confidence {
            Some(r) if r < 0.3 => self.config.user_cap.min(20),
            _ => self.config.user_cap,
        }
    }

    /// 处理跨维度耦合熔断——根据 source 维度精准下发 D3 主动麻痹锚点。
    ///
    /// D3 不是"停"——是"按回地面"。
    /// source 维度不施加额外刺激——其余维度全静默——
    /// 仅 Visceral(idx 0) 输出低频 steady 镇定信号。
    pub fn handle_safety_breach(&mut self, source: S::Dimension) -> GroundingSignal<D> {
        self.phase = SessionPhase::Aborted;
        self.tracker.reset();
        GroundingSignal::<D>::for_source(S::dim_index(source))
    }
}
