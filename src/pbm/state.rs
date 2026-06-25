// Feelings-Core — PBM 基线矩阵地基
//
// 个人基线矩阵。设备本地存储——永不离设备。
// 职责: ColdStartGuard / DampingState / SessionLabel / DataConfidence /
//       DefenceLevel / 四维 PBM 偏移值 [Visceral, Emotional, Tactile, Auditory]

use crate::config::CoreConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── 四维感受维度 ──────────────────────────────────────────

/// 四维感受维度——每个原子的主感受方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PbmDimension {
    Visceral,
    Emotional,
    Tactile,
    Auditory,
}

/// PbmDimension → [f64; 4] 数组索引。
pub fn dim_index(dim: PbmDimension) -> usize {
    match dim {
        PbmDimension::Visceral => 0,
        PbmDimension::Emotional => 1,
        PbmDimension::Tactile => 2,
        PbmDimension::Auditory => 3,
    }
}

pub const DIM_ORDER: [PbmDimension; 4] = [
    PbmDimension::Visceral,
    PbmDimension::Emotional,
    PbmDimension::Tactile,
    PbmDimension::Auditory,
];

// ── Session 标签与数据置信度 ──────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionLabel {
    Normal,
    Abnormal,
    ColdStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataConfidence {
    High,
    Low,
    Contaminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbmUpdateStrategy {
    Full,
    SafetyOnly,
    Frozen,
}

impl SessionLabel {
    pub fn update_strategy(&self) -> PbmUpdateStrategy {
        match self {
            SessionLabel::Normal => PbmUpdateStrategy::Full,
            SessionLabel::Abnormal => PbmUpdateStrategy::SafetyOnly,
            SessionLabel::ColdStart => PbmUpdateStrategy::Full,
        }
    }
}

// ── 冷启动守护 ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartGuard {
    pub threshold: u32,
    pub session_count: u32,
}

impl Default for ColdStartGuard {
    fn default() -> Self {
        ColdStartGuard {
            threshold: 10,
            session_count: 0,
        }
    }
}

impl ColdStartGuard {
    pub fn from_config(config: &CoreConfig) -> Self {
        ColdStartGuard {
            threshold: config.cold_start.sessions_threshold,
            session_count: 0,
        }
    }

    pub fn is_cold_start(&self) -> bool {
        self.session_count < self.threshold
    }
    pub fn predictor_enabled(&self) -> bool {
        !self.is_cold_start()
    }
    pub fn complete_session(&mut self) {
        self.session_count = self.session_count.saturating_add(1);
    }
}

// ── 防御激活层级 ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefenceLevel {
    D1,
    D2,
    D3,
}

// ── 阻尼矩阵 ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepState {
    Active,
    Frozen { reason: PbmDimension },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DampingState {
    step_multipliers: [f64; 4],
    previous_snapshot: Option<[f64; 4]>,
    ema_alpha: f64,
    freeze_factor: f64,
    cold_start: bool,
}

impl Default for DampingState {
    fn default() -> Self {
        DampingState {
            step_multipliers: [1.0; 4],
            previous_snapshot: None,
            ema_alpha: 0.3,
            freeze_factor: 0.85,
            cold_start: true,
        }
    }
}

impl DampingState {
    pub fn new(
        initial_steps: [f64; 4],
        ema_alpha: f64,
        freeze_factor: f64,
        cold_start: bool,
    ) -> Self {
        DampingState {
            step_multipliers: initial_steps,
            previous_snapshot: None,
            ema_alpha,
            freeze_factor,
            cold_start,
        }
    }

    /// 从 CoreConfig 构造——所有参数从配置读取。
    pub fn from_config(config: &CoreConfig, cold_start: bool) -> Self {
        Self::new(
            [1.0; 4],
            config.damping.ema_alpha,
            config.damping.freeze_factor,
            cold_start,
        )
    }

    pub fn current_steps(&self) -> [(PbmDimension, f64); 4] {
        [
            (PbmDimension::Visceral, self.step_multipliers[0]),
            (PbmDimension::Emotional, self.step_multipliers[1]),
            (PbmDimension::Tactile, self.step_multipliers[2]),
            (PbmDimension::Auditory, self.step_multipliers[3]),
        ]
    }

    pub fn update(&mut self, values: &[f64; 4]) {
        if let Some(ref prev) = self.previous_snapshot {
            for i in 0..4 {
                let delta = (values[i] - prev[i]).abs();
                self.step_multipliers[i] =
                    self.ema_alpha * delta + (1.0 - self.ema_alpha) * self.step_multipliers[i];
                if !self.cold_start {
                    self.step_multipliers[i] *= self.freeze_factor;
                }
                self.step_multipliers[i] = self.step_multipliers[i].clamp(0.1, 2.0);
            }
        }
        self.previous_snapshot = Some(*values);
    }

    pub fn compute_gradients(&self, values: &[f64; 4]) -> Option<[(PbmDimension, f64); 4]> {
        self.previous_snapshot.map(|prev| {
            [
                (PbmDimension::Visceral, (values[0] - prev[0]).abs()),
                (PbmDimension::Emotional, (values[1] - prev[1]).abs()),
                (PbmDimension::Tactile, (values[2] - prev[2]).abs()),
                (PbmDimension::Auditory, (values[3] - prev[3]).abs()),
            ]
        })
    }
}

/// 跨维度冻结矩阵——静态映射表。
pub struct DampingMatrix;

impl DampingMatrix {
    /// 跨维度冻结规则：触发维度 → 被冻结的维度。
    fn triggers(dim: PbmDimension) -> &'static [PbmDimension] {
        match dim {
            PbmDimension::Emotional => &[PbmDimension::Visceral, PbmDimension::Tactile],
            PbmDimension::Visceral => &[PbmDimension::Emotional],
            _ => &[],
        }
    }

    /// 从 CoreConfig 读取梯度阈值——每维度独立配置。
    /// 不再提供硬编码后备——每个人阈值不同，必须从配置或采集数据中确定。
    pub fn gradient_threshold(dim: PbmDimension, config: &CoreConfig) -> f64 {
        match dim {
            PbmDimension::Emotional => config.damping.emotional_threshold,
            PbmDimension::Visceral => config.damping.visceral_threshold,
            PbmDimension::Tactile => config.damping.tactile_threshold,
            PbmDimension::Auditory => config.damping.auditory_threshold,
        }
    }

    pub fn apply(
        gradients: &[(PbmDimension, f64); 4],
        current_steps: &[(PbmDimension, f64); 4],
    ) -> HashMap<PbmDimension, StepState> {
        Self::apply_with_config(gradients, current_steps, &CoreConfig::default())
    }

    /// 从 CoreConfig 读取梯度阈值——每个人阈值不同。
    pub fn apply_with_config(
        gradients: &[(PbmDimension, f64); 4],
        current_steps: &[(PbmDimension, f64); 4],
        config: &CoreConfig,
    ) -> HashMap<PbmDimension, StepState> {
        let step_map = |dim: PbmDimension| -> f64 {
            current_steps
                .iter()
                .find(|(d, _)| *d == dim)
                .map(|(_, s)| *s)
                .unwrap_or(1.0)
        };
        let mut states: HashMap<_, _> = gradients
            .iter()
            .map(|(d, _)| (*d, StepState::Active))
            .collect();
        for (dim, grad) in gradients.iter() {
            let threshold = Self::gradient_threshold(*dim, config) * step_map(*dim);
            if *grad > threshold {
                for target in Self::triggers(*dim) {
                    if *target != *dim {
                        states.insert(*target, StepState::Frozen { reason: *dim });
                    }
                }
            }
        }
        states
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_start_session_0_is_cold() {
        let guard = ColdStartGuard::default();
        assert!(guard.is_cold_start());
    }

    #[test]
    fn cold_start_session_10_is_warm() {
        let mut guard = ColdStartGuard::default();
        for _ in 0..10 {
            guard.complete_session();
        }
        assert!(!guard.is_cold_start());
    }

    #[test]
    fn damping_emotional_freezes_visceral_and_tactile() {
        let gradients = [
            (PbmDimension::Emotional, 3.0),
            (PbmDimension::Visceral, 0.0),
            (PbmDimension::Tactile, 0.0),
            (PbmDimension::Auditory, 0.0),
        ];
        let steps = DIM_ORDER.map(|d| (d, 1.0));
        let frozen = DampingMatrix::apply(&gradients, &steps);
        assert!(matches!(
            frozen.get(&PbmDimension::Visceral),
            Some(StepState::Frozen { .. })
        ));
        assert!(matches!(
            frozen.get(&PbmDimension::Tactile),
            Some(StepState::Frozen { .. })
        ));
        assert!(matches!(
            frozen.get(&PbmDimension::Auditory),
            Some(StepState::Active)
        ));
    }

    #[test]
    fn damping_state_computes_gradients() {
        let mut ds = DampingState::new([1.0; 4], 0.5, 1.0, false);
        ds.update(&[1.0, 2.0, 1.0, 1.0]);
        let g = ds.compute_gradients(&[1.5, 2.5, 1.2, 0.8]).unwrap();
        assert!((g[0].1 - 0.5).abs() < 0.001);
    }
}
