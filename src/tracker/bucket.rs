// Feelings-Core — NeuroEnergyTracker 四维漏桶
//
// ADR 011: 时域能量积分与生物漏桶安全。Session 级状态——每帧 intake_and_verify()。
// 非线性泄漏（防 PWM）、不应期保护窗、跨维度耦合 σ、全局总耦合能耗漏桶。
//
// 当前默认值基于人类生理。猫/狗/鹦鹉换 CoreConfig 即可。

use crate::config::CoreConfig;
use crate::pbm::dim_index;
use crate::pbm::PbmDimension;

/// 安全 Profile 类型——决定漏桶参数矩阵的梯度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    Standard,
    LowAnchor,
    DefenceD1,
    DefenceD2,
    DefenceD3,
}

/// 用户安全档案——持有四维漏桶参数。
#[derive(Debug, Clone, Copy)]
pub struct UserSafetyProfile {
    pub kind: ProfileKind,
    /// 每维度能量泄漏速率（强度分/秒）。越高漏得越快→不容易超限。
    pub leak_rates: [f64; 4],
    /// 每维度临界能量阈值——累积超过此值即 SafetyBreach。
    pub critical_thresholds: [f64; 4],
}

impl Default for UserSafetyProfile {
    fn default() -> Self {
        UserSafetyProfile {
            kind: ProfileKind::Standard,
            leak_rates: [2.0; 4],
            critical_thresholds: [600.0; 4],
        }
    }
}

impl UserSafetyProfile {
    pub fn standard() -> Self {
        Self::default()
    }
    pub fn leak_rate(&self, dim: PbmDimension) -> f64 {
        self.leak_rates[dim_index(dim)]
    }
    pub fn critical_threshold(&self, dim: PbmDimension) -> f64 {
        self.critical_thresholds[dim_index(dim)]
    }
}

/// 四维独立生物漏桶——模拟神经递质重摄取/代谢清除。
///
/// 每维度一个独立漏桶。持续摄入→累积，泄漏→衰减。
/// 触碰 90% 阈值→不应期，超过临界值→SafetyBreach。
#[derive(Debug, Clone)]
pub struct NeuroEnergyTracker {
    /// 四维当前累积能量。[Visceral, Emotional, Tactile, Auditory]
    cumulative_energy: [f64; 4],
    /// 上次摄入时间戳(纳秒)。用于算两次帧间的泄漏时长。
    last_tick_ns: Option<u64>,
    /// 最大采样间隔(秒)。超过=时钟挂起——不泄漏，保护窗。
    max_dt: f64,
    /// 非线性泄漏系数γ。能量越高漏越快——γ越大加速越明显。
    nonlinear_gamma: f64,
    /// 不应期激活标志——触碰 90% 阈值后启动。
    refractory_active: [bool; 4],
    /// 不应期剩余帧数——倒计时至零后恢复。
    refractory_counter: [u32; 4],
    /// 不应期持续帧数。在此窗口内暂停能量摄入。
    refractory_frames: u32,
    /// 跨维度耦合强度σ。一维充盈→同比压缩其他维临界阈值。
    pub(crate) sigma: f64,
    /// 全局桶权重α。四维能量和×α vs 全局阈值。
    pub(crate) global_alpha: f64,
    /// 全局桶比例β。全局阈值=四维临界阈值和×β。超限→熔断。
    pub(crate) global_beta: f64,
}

impl Default for NeuroEnergyTracker {
    fn default() -> Self {
        Self::from_config(&CoreConfig::default())
    }
}

impl NeuroEnergyTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 CoreConfig 构造——所有参数从配置读取。
    pub fn from_config(config: &CoreConfig) -> Self {
        NeuroEnergyTracker {
            cumulative_energy: [0.0; 4],
            last_tick_ns: None,
            max_dt: config.leaky_bucket.max_dt_seconds,
            nonlinear_gamma: config.leaky_bucket.nonlinear_gamma,
            refractory_active: [false; 4],
            refractory_counter: [0; 4],
            refractory_frames: config.leaky_bucket.refractory_frames,
            sigma: config.leaky_bucket.sigma,
            global_alpha: config.leaky_bucket.global_alpha,
            global_beta: config.leaky_bucket.global_beta,
        }
    }

    /// 每帧摄入强度 + 校验是否超限。
    pub fn intake_and_verify(
        &mut self,
        intensity: u32,
        dim: PbmDimension,
        profile: &UserSafetyProfile,
        now_ns: u64,
    ) -> Result<(), &'static str> {
        let idx = dim_index(dim);
        let threshold = profile.critical_threshold(dim);
        if self.refractory_active[idx] {
            self.refractory_counter[idx] = self.refractory_counter[idx].saturating_sub(1);
            if self.cumulative_energy[idx] < 0.5 * threshold {
                self.refractory_active[idx] = false;
                self.refractory_counter[idx] = 0;
            } else if self.refractory_counter[idx] == 0 {
                self.refractory_counter[idx] = self.refractory_frames;
            } else {
                if let Some(last) = self.last_tick_ns {
                    let dt = (now_ns.saturating_sub(last)) as f64 / 1_000_000_000.0;
                    if dt <= self.max_dt {
                        let leak = effective_leak_rate(
                            self.cumulative_energy[idx],
                            threshold,
                            profile.leak_rate(dim),
                            self.nonlinear_gamma,
                        );
                        self.cumulative_energy[idx] =
                            (self.cumulative_energy[idx] - leak * dt).max(0.0);
                    }
                }
                self.last_tick_ns = Some(now_ns);
                return Ok(());
            }
        }
        if let Some(last) = self.last_tick_ns {
            let dt = (now_ns.saturating_sub(last)) as f64 / 1_000_000_000.0;
            if dt <= self.max_dt {
                let leak = effective_leak_rate(
                    self.cumulative_energy[idx],
                    threshold,
                    profile.leak_rate(dim),
                    self.nonlinear_gamma,
                );
                self.cumulative_energy[idx] =
                    (self.cumulative_energy[idx] + intensity as f64 - leak * dt).max(0.0);
            } else {
                self.cumulative_energy[idx] += intensity as f64;
            }
        } else {
            self.cumulative_energy[idx] += intensity as f64;
        }
        self.last_tick_ns = Some(now_ns);
        if self.cumulative_energy[idx] > 0.9 * threshold && !self.refractory_active[idx] {
            self.refractory_active[idx] = true;
            self.refractory_counter[idx] = self.refractory_frames;
        }
        if self.cumulative_energy[idx] > threshold {
            return Err("SafetyBreach");
        }
        Ok(())
    }

    pub fn energy(&self, dim: PbmDimension) -> f64 {
        self.cumulative_energy[dim_index(dim)]
    }
    pub fn reset(&mut self) {
        self.cumulative_energy = [0.0; 4];
        self.last_tick_ns = None;
        self.refractory_active = [false; 4];
        self.refractory_counter = [0; 4];
    }
}

/// 非线性泄漏速率。
/// LeakRate(E) = baseline × (1 + γ × E/threshold)——能量越高漏得越快。
pub fn effective_leak_rate(cumulative: f64, threshold: f64, baseline: f64, gamma: f64) -> f64 {
    if threshold <= 0.0 {
        return baseline;
    }
    baseline * (1.0 + gamma * (cumulative / threshold).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_intensity_does_nothing() {
        let mut t = NeuroEnergyTracker::new();
        let p = UserSafetyProfile::standard();
        t.intake_and_verify(0, PbmDimension::Emotional, &p, 1_000_000)
            .unwrap();
        assert!((t.energy(PbmDimension::Emotional) - 0.0).abs() < 1e-10);
    }
    #[test]
    fn first_frame_accumulates() {
        let mut t = NeuroEnergyTracker::new();
        let p = UserSafetyProfile::standard();
        t.intake_and_verify(50, PbmDimension::Emotional, &p, 1_000_000)
            .unwrap();
        assert!((t.energy(PbmDimension::Emotional) - 50.0).abs() < 1e-10);
    }
    #[test]
    fn leak_reduces_energy() {
        let mut t = NeuroEnergyTracker::new();
        let p = UserSafetyProfile::standard();
        t.intake_and_verify(50, PbmDimension::Emotional, &p, 1_000_000)
            .unwrap();
        t.intake_and_verify(50, PbmDimension::Emotional, &p, 2_000_000)
            .unwrap();
        assert!(t.energy(PbmDimension::Emotional) < 100.0);
    }
    #[test]
    fn high_continuous_breaches() {
        let mut t = NeuroEnergyTracker::new();
        let mut p = UserSafetyProfile::standard();
        p.critical_thresholds = [10.0; 4];
        let mut r = Ok(());
        for i in 0..100 {
            r = t.intake_and_verify(1, PbmDimension::Emotional, &p, (i + 1) as u64 * 1_000_000);
            if r.is_err() {
                break;
            }
        }
        assert!(r.is_err());
    }
    #[test]
    fn nonlinear_leak_faster_at_high() {
        assert!(
            effective_leak_rate(90.0, 100.0, 2.0, 1.0) > effective_leak_rate(10.0, 100.0, 2.0, 1.0)
        );
    }
}
