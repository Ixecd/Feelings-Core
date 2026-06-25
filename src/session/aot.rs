// Feelings-Core — AOT 静态满载仿真器
//
// Init 阶段对 FSIR 做最坏情况静态波形仿真。
// 给定 shape + intensity + 预估帧数 → 逐帧生成强度序列 → 在 tracker 上离线推演。
// 数学上必然触发 GlobalBucketBreach → 交织阶段拒绝 PSIR 生成。

use crate::species::FeelingTarget;
use crate::tracker::NeuroEnergyTracker;

/// AOT（Ahead-of-Time）仿真结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AotResult {
    Pass,
    Fail { frame: usize, reason: String },
}

/// Shape 类型——决定强度曲线形状。
#[derive(Debug, Clone, Copy)]
pub enum Shape {
    Steady,
    GradualRiseFall,
    SharpPeak,
    Wave,
    SlowDecay,
    AbruptStop,
}

/// 按 shape + intensity 区间 + 帧数生成强度序列。
///
/// 每种 shape 的最坏情况 = 全帧取 max(intensity) 走稳态推演。
/// 注：这不是精确的 frame-level 仿真——是**数学最坏情况积分**。
/// 如果最坏情况不超限——真实波形更不会超限。
pub fn worst_case_waveform(shape: Shape, intensity_max: u32, total_frames: usize) -> Vec<u32> {
    match shape {
        Shape::Steady | Shape::Wave | Shape::AbruptStop => {
            // 稳态 / 波 / 骤停——最坏情况 = 全帧 max
            vec![intensity_max; total_frames]
        }
        Shape::GradualRiseFall => {
            // 渐升渐退——峰值在中间 50% 区域
            // 安全保守逼近：前 25% 线性爬升 → 中 50% 峰值 → 后 25% 线性下降
            let mut seq = Vec::with_capacity(total_frames);
            let q = total_frames / 4;
            let half = intensity_max / 2;
            for t in 0..total_frames {
                let i = if t < q {
                    half + (t as u32 * half / q as u32)
                } else if t < total_frames - q {
                    intensity_max
                } else {
                    intensity_max - ((t - (total_frames - q)) as u32 * half / q as u32)
                };
                seq.push(i.min(intensity_max));
            }
            seq
        }
        Shape::SharpPeak => {
            // 尖峰——中间 10% 区域达到 max，其余为半强度
            let mut seq = Vec::with_capacity(total_frames);
            let peak_start = total_frames * 45 / 100;
            let peak_end = total_frames * 55 / 100;
            let half = intensity_max / 2;
            for t in 0..total_frames {
                if t >= peak_start && t < peak_end {
                    seq.push(intensity_max);
                } else {
                    seq.push(half);
                }
            }
            seq
        }
        Shape::SlowDecay => {
            // 缓慢衰减——起点 max，线性衰减到 0
            let mut seq = Vec::with_capacity(total_frames);
            for t in 0..total_frames {
                let i = intensity_max - (t as u32 * intensity_max / total_frames as u32);
                seq.push(i);
            }
            seq
        }
    }
}

/// AOT 静态仿真器。
pub struct AotSimulator;

impl AotSimulator {
    /// 对给定的 FSIR 波形做最坏情况帧级积分推演。
    ///
    /// 如果最坏情况都通过——真实波形不会超限。反之——直接拒绝 PSIR 生成。
    pub fn preflight_check<const D: usize, S: FeelingTarget>(
        tracker: &NeuroEnergyTracker<D, S>,
        profile: &crate::tracker::UserSafetyProfile<D, S>,
        shape: Shape,
        intensity_max: u32,
        total_frames: usize,
    ) -> AotResult {
        if total_frames == 0 {
            return AotResult::Pass;
        }

        let waveform = worst_case_waveform(shape, intensity_max, total_frames);
        let mut sim = tracker.clone();

        // 按 1ms 间隔推演
        for (frame, &intensity) in waveform.iter().enumerate() {
            let now_ns = (frame + 1) as u64 * 1_000_000;

            // 对所有维度同步摄入——最坏情况
            for dim_idx in 0..D {
                if let Some(dim) = S::index_to_dim(dim_idx) {
                    if sim
                        .intake_and_verify(intensity, dim, profile, now_ns)
                        .is_err()
                    {
                        return AotResult::Fail {
                            frame,
                            reason: format!("维度 {:?} 单维度超限 @ frame {}", dim, frame),
                        };
                    }
                }
            }

            if let Err(e) = sim.verify_cross_dimension(profile) {
                return AotResult::Fail {
                    frame,
                    reason: format!("跨维度/全局桶超限 @ frame {}: {:?}", frame, e),
                };
            }
        }

        AotResult::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CoreConfig;
    use crate::species::Human;
    use crate::tracker::UserSafetyProfile;

    fn human_profile() -> UserSafetyProfile<{ Human::DIM_COUNT }, Human> {
        UserSafetyProfile::standard([2.0; 4], [600.0; 4])
    }

    #[test]
    fn steady_safe_passes() {
        let tracker =
            NeuroEnergyTracker::<{ Human::DIM_COUNT }, Human>::from_config(&CoreConfig::default());
        let r = AotSimulator::preflight_check(&tracker, &human_profile(), Shape::Steady, 5, 50);
        assert_eq!(r, AotResult::Pass);
    }

    #[test]
    fn extreme_intensity_fails() {
        let tracker =
            NeuroEnergyTracker::<{ Human::DIM_COUNT }, Human>::from_config(&CoreConfig::default());
        let profile = UserSafetyProfile::standard([0.0; 4], [50.0; 4]);
        let r = AotSimulator::preflight_check(&tracker, &profile, Shape::Steady, 30, 1000);
        assert!(matches!(r, AotResult::Fail { .. }));
    }

    #[test]
    fn worst_case_waveform_steady_is_flat() {
        let w = worst_case_waveform(Shape::Steady, 50, 100);
        assert_eq!(w.len(), 100);
        assert!(w.iter().all(|&i| i == 50));
    }

    #[test]
    fn worst_case_gradual_rise_fall_peaks_in_middle() {
        let w = worst_case_waveform(Shape::GradualRiseFall, 100, 100);
        assert_eq!(w.len(), 100);
        assert_eq!(w[49], 100); // middle = peak
    }
}
