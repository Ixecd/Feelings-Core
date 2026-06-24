// Feelings-Core — 运行时配置
//
// 加载时 Vec 接收任意维度——validate_dimensions::<D>() 在构造 Session 前校验长度。
// serde 不支持泛型 [f64; D] derive——Vec 是务实选择。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartConfig {
    pub sessions_threshold: u32,
    pub damping_window: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DampingConfig {
    pub emotional_threshold: f64,
    pub visceral_threshold: f64,
    pub tactile_threshold: f64,
    pub auditory_threshold: f64,
    pub ema_alpha: f64,
    pub freeze_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakyBucketConfig {
    pub max_dt_seconds: f64,
    pub nonlinear_gamma: f64,
    pub refractory_frames: u32,
    pub sigma: f64,
    pub global_alpha: f64,
    pub global_beta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigmoidalConfig {
    pub k: f64,
    pub x0: f64,
    pub compression_alpha: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefenceSensitivityConfig {
    pub none: f64,
    pub d1: f64,
    pub d2: f64,
    pub d3: f64,
}

/// 运行时配置——维度无关。加载后用 validate_dimensions::<D>() 校验。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    pub cold_start: ColdStartConfig,
    pub damping: DampingConfig,
    pub leaky_bucket: LeakyBucketConfig,
    pub sigmoidal: SigmoidalConfig,
    /// 四维差异化冷启动系数——长度必须 = D。
    pub cold_start_coeffs: Vec<f64>,
    pub standard_leak_rates: Vec<f64>,
    pub standard_critical_thresholds: Vec<f64>,
    pub dimension_hint: usize,
    pub defence_sensitivity: DefenceSensitivityConfig,
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
            cold_start_coeffs: vec![0.75, 0.40, 0.80, 0.85],
            standard_leak_rates: vec![2.0; 4],
            standard_critical_thresholds: vec![600.0; 4],
            dimension_hint: 4,
            defence_sensitivity: DefenceSensitivityConfig {
                none: 1.0,
                d1: 1.3,
                d2: 1.8,
                d3: 2.5,
            },
        }
    }
}

impl CoreConfig {
    /// 校验维度长度是否与编译期 D 匹配。不匹配→拒绝启动。
    pub fn validate_dimensions<const D: usize>(&self) -> Result<(), &'static str> {
        if self.dimension_hint != D
            || self.cold_start_coeffs.len() != D
            || self.standard_leak_rates.len() != D
            || self.standard_critical_thresholds.len() != D
        {
            return Err("配置维度数与编译期 D 不匹配");
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.defence_sensitivity.d1 >= self.defence_sensitivity.d2
            || self.defence_sensitivity.d2 >= self.defence_sensitivity.d3
        {
            return Err("Defence sensitivity must be strictly increasing");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_validates() {
        assert!(CoreConfig::default().validate_dimensions::<4>().is_ok());
    }

    #[test]
    fn dimension_mismatch_rejected() {
        assert!(CoreConfig::default().validate_dimensions::<3>().is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let json = serde_json::to_string(&CoreConfig::default()).unwrap();
        let cfg: CoreConfig = serde_json::from_str(&json).unwrap();
        assert!(cfg.validate_dimensions::<4>().is_ok());
    }
}
