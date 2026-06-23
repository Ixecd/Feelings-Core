// Feelings-Core — NeuroEnergyTracker 四维漏桶
//
// ADR 011: 时域能量积分与生物漏桶安全。Session 级状态——每帧 intake_and_verify()。
// 非线性泄漏（防 PWM）、不应期保护窗、跨维度耦合 σ、全局总耦合能耗漏桶。

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
    pub leak_rates: [f64; 4],
    pub critical_thresholds: [f64; 4],
}

impl Default for UserSafetyProfile {
    fn default() -> Self {
        UserSafetyProfile {
            kind: ProfileKind::Standard,
            leak_rates: [2.0, 2.0, 2.0, 2.0],
            critical_thresholds: [600.0, 600.0, 600.0, 600.0],
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

/// 四维独立生物漏桶。
#[derive(Debug, Clone)]
pub struct NeuroEnergyTracker {
    cumulative_energy: [f64; 4],
    last_tick_ns: Option<u64>,
    max_dt: f64,
    nonlinear_gamma: f64,
    refractory_active: [bool; 4],
    refractory_counter: [u32; 4],
    refractory_frames: u32,
    pub(crate) sigma: f64,
    pub(crate) global_alpha: f64,
    pub(crate) global_beta: f64,
}

impl Default for NeuroEnergyTracker {
    fn default() -> Self {
        NeuroEnergyTracker {
            cumulative_energy: [0.0; 4],
            last_tick_ns: None,
            max_dt: 0.100,
            nonlinear_gamma: 1.0,
            refractory_active: [false; 4],
            refractory_counter: [0; 4],
            refractory_frames: 50,
            sigma: 0.3,
            global_alpha: 0.3,
            global_beta: 0.9,
        }
    }
}

impl NeuroEnergyTracker {
    pub fn new() -> Self {
        Self::default()
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

        // 不应期处理
        if self.refractory_active[idx] {
            self.refractory_counter[idx] = self.refractory_counter[idx].saturating_sub(1);
            if self.cumulative_energy[idx] < 0.5 * threshold {
                self.refractory_active[idx] = false;
                self.refractory_counter[idx] = 0;
            } else if self.refractory_counter[idx] == 0 {
                self.refractory_counter[idx] = self.refractory_frames;
            } else {
                // 不应期内只泄漏
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

        // 正常摄入 + 泄漏
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

        // 不应期触发
        if self.cumulative_energy[idx] > 0.9 * threshold && !self.refractory_active[idx] {
            self.refractory_active[idx] = true;
            self.refractory_counter[idx] = self.refractory_frames;
        }

        // 超限熔断
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

/// 非线性泄漏速率——ADR 011 §非线性泄漏。
pub fn effective_leak_rate(cumulative: f64, threshold: f64, baseline: f64, gamma: f64) -> f64 {
    if threshold <= 0.0 {
        return baseline;
    }
    let e_norm = (cumulative / threshold).clamp(0.0, 1.0);
    baseline * (1.0 + gamma * e_norm)
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
        let e = t.energy(PbmDimension::Emotional);
        assert!(e < 100.0, "leak should reduce energy, got {}", e);
    }

    #[test]
    fn high_continuous_breaches() {
        let mut t = NeuroEnergyTracker::new();
        let mut p = UserSafetyProfile::standard();
        p.critical_thresholds = [10.0; 4];
        let mut result = Ok(());
        for i in 0..100 {
            result =
                t.intake_and_verify(1, PbmDimension::Emotional, &p, (i + 1) as u64 * 1_000_000);
            if result.is_err() {
                break;
            }
        }
        assert!(result.is_err());
    }

    #[test]
    fn nonlinear_leak_faster_at_high() {
        let low = effective_leak_rate(10.0, 100.0, 2.0, 1.0);
        let high = effective_leak_rate(90.0, 100.0, 2.0, 1.0);
        assert!(high > low);
    }
}
