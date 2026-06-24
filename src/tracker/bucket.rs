// Feelings-Core — NeuroEnergyTracker<const D: usize, S> 泛型漏桶
//
// D = 维度数 (编译期常量), S = 物种 (决定维度类型)
// 调用方用法: NeuroEnergyTracker<{Human::DIM_COUNT}, Human>

use crate::config::CoreConfig;
use crate::pbm::DefenceLevel;
use crate::species::FeelingTarget;
use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    Standard,
    LowAnchor,
    DefenceD1,
    DefenceD2,
    DefenceD3,
}

pub struct UserSafetyProfile<const D: usize, S: FeelingTarget> {
    pub kind: ProfileKind,
    pub leak_rates: [f64; D],
    pub critical_thresholds: [f64; D],
    pub _phantom: PhantomData<S>,
}

impl<const D: usize, S: FeelingTarget> UserSafetyProfile<D, S> {
    pub fn standard(leak_rates: [f64; D], critical_thresholds: [f64; D]) -> Self {
        debug_assert_eq!(
            D,
            S::DIM_COUNT,
            "Dimension mismatch for species {}",
            S::species_name()
        );
        UserSafetyProfile {
            kind: ProfileKind::Standard,
            leak_rates,
            critical_thresholds,
            _phantom: PhantomData,
        }
    }
    pub fn leak_rate(&self, dim: S::Dimension) -> f64 {
        self.leak_rates[S::dim_index(dim)]
    }
    pub fn critical_threshold(&self, dim: S::Dimension) -> f64 {
        self.critical_thresholds[S::dim_index(dim)]
    }
}

#[derive(Debug, Clone)]
pub struct NeuroEnergyTracker<const D: usize, S: FeelingTarget> {
    cumulative_energy: [f64; D],
    last_tick_ns: Option<u64>,
    max_dt: f64,
    nonlinear_gamma: f64,
    refractory_active: [bool; D],
    refractory_counter: [u32; D],
    refractory_frames: u32,
    pub(crate) sigma: f64,
    pub(crate) global_alpha: f64,
    pub(crate) global_beta: f64,
    pub _phantom: PhantomData<S>,
}

impl<const D: usize, S: FeelingTarget> NeuroEnergyTracker<D, S> {
    pub fn from_config(config: &CoreConfig) -> Self {
        debug_assert_eq!(
            D,
            S::DIM_COUNT,
            "Tracker dimension mismatch for species {}",
            S::species_name()
        );
        NeuroEnergyTracker {
            cumulative_energy: [0.0; D],
            last_tick_ns: None,
            max_dt: config.leaky_bucket.max_dt_seconds,
            nonlinear_gamma: config.leaky_bucket.nonlinear_gamma,
            refractory_active: [false; D],
            refractory_counter: [0; D],
            refractory_frames: config.leaky_bucket.refractory_frames,
            sigma: config.leaky_bucket.sigma,
            global_alpha: config.leaky_bucket.global_alpha,
            global_beta: config.leaky_bucket.global_beta,
            _phantom: PhantomData,
        }
    }

    pub fn intake_and_verify(
        &mut self,
        intensity: u32,
        dim: S::Dimension,
        profile: &UserSafetyProfile<D, S>,
        now_ns: u64,
    ) -> Result<(), &'static str> {
        let idx = S::dim_index(dim);
        let threshold = profile.critical_threshold(dim);

        // 时钟单调性守卫——回拨/重放数据 → 跳过本次摄入，不污染漏桶
        if let Some(last) = self.last_tick_ns {
            if now_ns <= last {
                return Ok(());
            }
        }

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

    pub fn energy(&self, dim: S::Dimension) -> f64 {
        self.cumulative_energy[S::dim_index(dim)]
    }
    /// 返回四维能量数组——零堆分配。硬实时路径直接拷⻉。
    pub fn all_energies(&self) -> [f64; D] {
        self.cumulative_energy
    }
    pub fn energies(&self) -> &[f64; D] {
        &self.cumulative_energy
    }
    pub fn reset(&mut self) {
        self.cumulative_energy = [0.0; D];
        self.last_tick_ns = None;
        self.refractory_active = [false; D];
        self.refractory_counter = [0; D];
    }
}

pub fn effective_leak_rate(cumulative: f64, threshold: f64, baseline: f64, gamma: f64) -> f64 {
    if threshold <= 0.0 {
        return baseline;
    }
    baseline * (1.0 + gamma * (cumulative / threshold).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species::{Human, HumanDimension};

    fn human_profile() -> UserSafetyProfile<{ Human::DIM_COUNT }, Human> {
        UserSafetyProfile::standard([2.0; Human::DIM_COUNT], [600.0; Human::DIM_COUNT])
    }

    type HumanTracker = NeuroEnergyTracker<{ Human::DIM_COUNT }, Human>;

    #[test]
    fn zero_intensity_does_nothing() {
        let mut t = HumanTracker::from_config(&CoreConfig::default());
        let p = human_profile();
        t.intake_and_verify(0, HumanDimension::Emotional, &p, 1_000_000)
            .unwrap();
        assert!((t.energy(HumanDimension::Emotional) - 0.0).abs() < 1e-10);
    }
    #[test]
    fn first_frame_accumulates() {
        let mut t = HumanTracker::from_config(&CoreConfig::default());
        let p = human_profile();
        t.intake_and_verify(50, HumanDimension::Emotional, &p, 1_000_000)
            .unwrap();
        assert!((t.energy(HumanDimension::Emotional) - 50.0).abs() < 1e-10);
    }
    #[test]
    fn leak_reduces_energy() {
        let mut t = HumanTracker::from_config(&CoreConfig::default());
        let p = human_profile();
        t.intake_and_verify(50, HumanDimension::Emotional, &p, 1_000_000)
            .unwrap();
        t.intake_and_verify(50, HumanDimension::Emotional, &p, 2_000_000)
            .unwrap();
        assert!(t.energy(HumanDimension::Emotional) < 100.0);
    }
    #[test]
    fn high_continuous_breaches() {
        let mut t = HumanTracker::from_config(&CoreConfig::default());
        let p = UserSafetyProfile::standard([0.0; Human::DIM_COUNT], [10.0; Human::DIM_COUNT]);
        let mut r = Ok(());
        for i in 0..100 {
            r = t.intake_and_verify(1, HumanDimension::Emotional, &p, (i + 1) as u64 * 1_000_000);
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
