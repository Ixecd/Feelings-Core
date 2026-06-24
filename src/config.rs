// Feelings-Core — 运行时配置
//
// 加载时 Vec 接收任意维度——validate_dimensions::<D>() 在构造 Session 前校验长度。
// serde 不支持泛型 [f64; D] derive——Vec 是务实选择。

use serde::{Deserialize, Serialize};

/// 冷启动守护。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartConfig {
    /// 前 N 次 Session 判定为冷启动。默认 10。
    pub sessions_threshold: u32,
    /// 冷启动结束后阻尼从 0% 过渡到 100% 的窗口长度（Session 数）。默认 5。
    pub damping_window: u32,
}

/// 阻尼矩阵梯度阈值——每维度独立。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DampingConfig {
    /// 情绪维度梯度阈值（×当前步长）。默认 2.0。
    pub emotional_threshold: f64,
    /// 内脏维度梯度阈值（×当前步长）。默认 1.5。
    pub visceral_threshold: f64,
    /// 触觉维度梯度阈值（×当前步长）。默认 3.0。
    pub tactile_threshold: f64,
    /// 听觉维度梯度阈值（×当前步长）。默认 2.0。
    pub auditory_threshold: f64,
    /// EMA 平滑系数 (0-1)。越大越敏感——历史数据权重越低。默认 0.5。
    pub ema_alpha: f64,
    /// 阻尼冻结因子 (0-1)。冻结后步长乘以该系数。1.0 = 不冻结。默认 0.5。
    pub freeze_factor: f64,
}

/// 漏桶参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakyBucketConfig {
    /// 两次采样间最大间隔（秒）。超过=时钟挂起→不泄漏（保护窗）。默认 0.1。
    pub max_dt_seconds: f64,
    /// 非线性泄漏系数 γ。能量越高漏越快——γ 越大加速越明显。默认 1.0。
    pub nonlinear_gamma: f64,
    /// 不应期持续帧数。触碰 90% 阈值后在此窗口内暂停摄入。默认 50。
    pub refractory_frames: u32,
    /// 跨维度耦合强度 σ (0-1)。一维充盈同比压缩其他维的临界阈值。默认 0.3。
    pub sigma: f64,
    /// 全局桶权重 α。四维能量总和 × α 后与全局阈值对比。默认 0.3。
    pub global_alpha: f64,
    /// 全局桶比例 β。全局阈值 = 四维临界阈值之和 × β。超限→全局熔断。默认 0.9。
    pub global_beta: f64,
}

/// Sigmoidal 缩放参数。
///
/// 公式（ADR 009 §一）:
///   x = original / cap
///   σ(x) = 1/(1+e^{-k(x-x0)})
///   compression(x) = 1 − α×σ(x)
///   applied = original × baseline_coeff × compression(x)
///
/// 效果：低强度 ≈ 线性，中强度减缓，高强度趋近饱和。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigmoidalConfig {
    /// 曲线陡峭度 k。越大越陡——中强度压缩更剧烈。默认 6.0。
    pub k: f64,
    /// 曲线中点 x0（归一化强度）。超过后压缩加速。默认 0.5（50%）。
    pub x0: f64,
    /// 压缩幅度 α (0-1)。越大饱和越深——高强度输出压得越低。默认 0.5。
    pub compression_alpha: f64,
}

/// 防御敏感系数——每个 DefenceLevel 对应不同放大倍数。
/// None = 标准感知 / D1 = 轻度敏感 / D2 = 高度敏感 / D3 = 极限敏感。
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
    /// 每维度能量泄漏速率（强度分/秒）。长度必须 = D。
    pub standard_leak_rates: Vec<f64>,
    /// 每维度临界能量阈值。长度必须 = D。
    pub standard_critical_thresholds: Vec<f64>,
    /// 维度数量提示。必须与编译期 D 一致。
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
    /// 校验配置维度是否与编译期 D 匹配。不匹配→拒绝启动。
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

    /// 校验参数合法性。
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
