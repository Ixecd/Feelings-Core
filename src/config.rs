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
    /// 前 N 次 Session 判定为冷启动。默认 10。
    pub sessions_threshold: u32,
    /// 冷启动结束后——阻尼从 0% 过渡到 100% 的窗口长度（Session 数）。默认 5。
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
    /// 两次采样间的最大间隔（秒）。超过则视为时钟挂起——不泄漏。默认 0.1。
    pub max_dt_seconds: f64,
    /// 非线性泄漏系数 γ。能量越高漏得越快——γ 越大加速越明显。默认 1.0。
    pub nonlinear_gamma: f64,
    /// 不应期持续帧数。能量触碰阈值后在此窗口内暂停摄入。默认 50。
    pub refractory_frames: u32,
    /// 跨维度耦合强度 σ (0-1)。一维充盈会同比压缩其他三维的临界阈值。默认 0.3。
    pub sigma: f64,
    /// 全局桶权重 α。四维能量总和乘以 α 后与全局阈值对比。默认 0.3。
    pub global_alpha: f64,
    /// 全局桶比例 β。全局阈值 = 四维临界阈值之和 × β。默认 0.9。
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
    /// 曲线陡峭度。越大越陡——中强度压缩更剧烈。默认 6.0。
    pub k: f64,
    /// 曲线中点（归一化强度）。强度超过此值后压缩加速。默认 0.5（50%）。
    pub x0: f64,
    /// 压缩幅度 α (0-1)。越大饱和越深——高强度输出压得越低。默认 0.5。
    pub compression_alpha: f64,
}

/// 四维差异化冷启动系数。
///
/// 系数 < 1.0 = 该维度初始敏感度偏低——需要更强信号才能触发同等感受。
/// 来源：Feelings-ROADMAP §1.2。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartCoeffsConfig {
    /// 内脏维度系数。默认 0.75。
    pub visceral: f64,
    /// 情绪维度系数。默认 0.40——初始最不敏感。
    pub emotional: f64,
    /// 触觉维度系数。默认 0.80。
    pub tactile: f64,
    /// 听觉维度系数。默认 0.85。
    pub auditory: f64,
}

/// 用户安全档案——四维漏桶基线。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyProfileConfig {
    /// 每个维度的能量泄漏速率（强度分/秒）。[Visceral, Emotional, Tactile, Auditory]。默认 [2.0; 4]。
    pub standard_leak_rates: [f64; 4],
    /// 每个维度的临界能量阈值——超过即 SafetyBreach。[Visceral, Emotional, Tactile, Auditory]。默认 [600.0; 4]。
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
