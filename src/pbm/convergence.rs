// Feelings-Core — PBM 基线收敛算法
//
// Sigmoidal 缩放 + 四维差异化冷启动系数 + DampingMatrix +
// 跨 Session 收敛 + DataConfidence 步长乘数 + 冷启动阻尼淡入窗

use crate::config::CoreConfig;
use crate::pbm::DefenceLevel;

/// PBM 四维差异化冷启动基线偏移系数。
///
/// 来自 Feelings-ROADMAP §1.2：
///   内脏 0.75 / 情绪 0.40 / 触觉 0.80 / 听觉 0.85
#[derive(Debug, Clone, Copy)]
pub struct PbmColdStartCoefficients {
    pub visceral: f64,
    pub emotional: f64,
    pub tactile: f64,
    pub auditory: f64,
}

impl Default for PbmColdStartCoefficients {
    fn default() -> Self {
        PbmColdStartCoefficients {
            visceral: 0.75,
            emotional: 0.40,
            tactile: 0.80,
            auditory: 0.85,
        }
    }
}

// ── Sigmoidal 非线性缩放 ─────────────────────────────────────

/// 强度 sigmoidal 非线性缩放。
///
/// 公式:
///   x = original / cap
///   σ(x) = 1/(1+e^{-k(x-x0)}), k=6, x0=0.5
///   compression(x) = 1 − α×σ(x), α=0.5
///   applied = original × baseline_coeff × compression(x)
pub fn sigmoidal_scale(original: u32, baseline_coeff: f64, cap: u32) -> u32 {
    sigmoidal_scale_with_config(original, baseline_coeff, cap, &CoreConfig::default())
}

/// 从 CoreConfig 读取 sigmoidal 参数。
pub fn sigmoidal_scale_with_config(
    original: u32,
    baseline_coeff: f64,
    cap: u32,
    config: &CoreConfig,
) -> u32 {
    if cap == 0 {
        return 0;
    }
    let x = original as f64 / cap as f64;
    let k = config.sigmoidal.k;
    let x0 = config.sigmoidal.x0;
    let alpha = config.sigmoidal.compression_alpha;
    let sigmoid = 1.0 / (1.0 + (-k * (x - x0)).exp());
    let compression = 1.0 - alpha * sigmoid;
    let scaled = original as f64 * baseline_coeff * compression;
    (scaled.round() as u32).min(cap)
}

/// 防御敏感系数——从 CoreConfig 读取。
pub fn d_sensitivity(defence_level: Option<DefenceLevel>) -> f64 {
    d_sensitivity_with_config(defence_level, &CoreConfig::default())
}

/// 从 CoreConfig 读取防御敏感系数。
pub fn d_sensitivity_with_config(defence_level: Option<DefenceLevel>, config: &CoreConfig) -> f64 {
    match defence_level {
        None => config.defence_sensitivity.none,
        Some(DefenceLevel::D1) => config.defence_sensitivity.d1,
        Some(DefenceLevel::D2) => config.defence_sensitivity.d2,
        Some(DefenceLevel::D3) => config.defence_sensitivity.d3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigmoidal_zero_intensity() {
        assert_eq!(sigmoidal_scale(0, 1.0, 100), 0);
    }

    #[test]
    fn sigmoidal_low_near_linear() {
        let applied = sigmoidal_scale(10, 1.0, 100);
        assert_eq!(applied, 10);
    }

    #[test]
    fn sigmoidal_mid_compression() {
        let applied = sigmoidal_scale(50, 1.0, 100);
        assert_eq!(applied, 38);
    }

    #[test]
    fn sigmoidal_high_saturation() {
        let applied = sigmoidal_scale(100, 1.0, 100);
        assert_eq!(applied, 52);
    }

    #[test]
    fn sigmoidal_emotional_coeff() {
        let applied = sigmoidal_scale(90, 0.40, 100);
        assert_eq!(applied, 19); // 情绪维度压缩极重
    }

    #[test]
    fn d_sensitivity_none() {
        assert!((d_sensitivity(None) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn d_sensitivity_d3() {
        assert!((d_sensitivity(Some(DefenceLevel::D3)) - 2.5).abs() < 1e-10);
    }
}
