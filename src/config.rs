// Feelings-Core — 运行时配置
//
// 所有可调参数的单一定义点。每个用户有自己的基线——系数不能写死。
// Session 启动时从 YAML/JSON 加载，未提供则用默认值。

use serde::{Deserialize, Serialize};

/// Core 运行时完整配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    /// 冷启动参数。
    pub cold_start: ColdStartConfig,
    /// 阻尼参数。
    pub damping: DampingConfig,
    /// 漏桶参数。
    pub leaky_bucket: LeakyBucketConfig,
    /// Sigmoidal 缩放参数。
    pub sigmoidal: SigmoidalConfig,
    /// 四维差异化冷启动基线偏移系数。
    pub cold_start_coeffs: ColdStartCoeffsConfig,
    /// 用户安全档案——四维漏桶基线。
    pub safety_profile: SafetyProfileConfig,
    /// PBM 维度数量。当前 = 4 (Visceral/Emotional/Tactile/Auditory)。
    /// 新设备入列后递增——17 条 NeuralPathway 已在 ADR 016 枚举。
    pub dimension_count: usize,
}

/// 冷启动守护。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartConfig {
    pub sessions_threshold: u32,
    pub damping_window: u32,
}

/// 阻尼矩阵梯度阈值——每维度独立。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DampingConfig {
    pub emotional_threshold: f64,
    pub visceral_threshold: f64,
    pub tactile_threshold: f64,
    pub auditory_threshold: f64,
    pub ema_alpha: f64,
    pub freeze_factor: f64,
}

/// 漏桶参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakyBucketConfig {
    pub max_dt_seconds: f64,
    pub nonlinear_gamma: f64,
    pub refractory_frames: u32,
    pub sigma: f64,
    pub global_alpha: f64,
    pub global_beta: f64,
}

/// Sigmoidal 缩放参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigmoidalConfig {
    pub k: f64,
    pub x0: f64,
    pub compression_alpha: f64,
}

/// 四维差异化冷启动系数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartCoeffsConfig {
    pub visceral: f64,
    pub emotional: f64,
    pub tactile: f64,
    pub auditory: f64,
}

/// 用户安全档案——四维漏桶基线。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyProfileConfig {
    pub standard_leak_rates: [f64; 4],
    pub standard_critical_thresholds: [f64; 4],
}

impl Default for CoreConfig {
    fn default() -> Self {
        CoreConfig {
            cold_start: ColdStartConfig {
                sessions_threshold: 10,
                damping_window: 5,
            },
            damping: DampingConfig {
                emotional_threshold: 2.0,
                visceral_threshold: 1.5,
                tactile_threshold: 3.0,
                auditory_threshold: 2.0,
                ema_alpha: 0.5,
                freeze_factor: 0.5,
            },
            leaky_bucket: LeakyBucketConfig {
                max_dt_seconds: 0.100,
                nonlinear_gamma: 1.0,
                refractory_frames: 50,
                sigma: 0.3,
                global_alpha: 0.3,
                global_beta: 0.9,
            },
            sigmoidal: SigmoidalConfig {
                k: 6.0,
                x0: 0.5,
                compression_alpha: 0.5,
            },
            cold_start_coeffs: ColdStartCoeffsConfig {
                visceral: 0.75,
                emotional: 0.40,
                tactile: 0.80,
                auditory: 0.85,
            },
            safety_profile: SafetyProfileConfig {
                standard_leak_rates: [2.0; 4],
                standard_critical_thresholds: [600.0; 4],
            },
            dimension_count: 4,
        }
    }
}
