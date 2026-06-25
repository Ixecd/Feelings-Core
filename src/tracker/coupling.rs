// Feelings-Core — 跨维度耦合 + 全局桶（泛型）

use crate::species::FeelingTarget;
use crate::tracker::{NeuroEnergyTracker, UserSafetyProfile};

#[derive(Debug, Clone)]
pub enum SafetyBreach<S: FeelingTarget> {
    CrossDimCoupling {
        source: S::Dimension,
        current_energy: f64,
        suppressed_threshold: f64,
    },
    GlobalBucketOverload {
        total_sum: f64,
        limit: f64,
    },
}

impl<const D: usize, S: FeelingTarget> NeuroEnergyTracker<D, S> {
    pub fn verify_cross_dimension(
        &self,
        profile: &UserSafetyProfile<D, S>,
    ) -> Result<(), SafetyBreach<S>> {
        let energies = self.all_energies();
        for (i, &e_i) in energies.iter().enumerate() {
            let e_norm = e_i / profile.critical_thresholds[i].max(1.0);
            let coupling = self.sigma * e_norm;
            for (j, &e_j) in energies.iter().enumerate() {
                if i == j {
                    continue;
                }
                let effective = profile.critical_thresholds[j] * (1.0 - coupling);
                if e_j > effective {
                    return Err(SafetyBreach::CrossDimCoupling {
                        source: S::index_to_dim(i).expect("valid index"),
                        current_energy: e_j,
                        suppressed_threshold: effective,
                    });
                }
            }
        }
        let energy_global: f64 = energies.iter().sum::<f64>() * self.global_alpha;
        let global_threshold: f64 =
            profile.critical_thresholds.iter().sum::<f64>() * self.global_beta;
        if energy_global > global_threshold {
            return Err(SafetyBreach::GlobalBucketOverload {
                total_sum: energy_global,
                limit: global_threshold,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species::{Human, HumanDimension};
    use crate::tracker::ProfileKind;

    #[test]
    fn global_bucket_catches_alternating_attack() {
        let mut t = NeuroEnergyTracker::<{ Human::DIM_COUNT }, Human>::from_config(
            &crate::config::CoreConfig::default(),
        );
        let p = UserSafetyProfile::<{ Human::DIM_COUNT }, Human> {
            kind: ProfileKind::Standard,
            leak_rates: [0.0; Human::DIM_COUNT],
            critical_thresholds: [100.0; Human::DIM_COUNT],
            _phantom: std::marker::PhantomData,
        };
        let dims = [
            HumanDimension::Visceral,
            HumanDimension::Emotional,
            HumanDimension::Tactile,
            HumanDimension::Auditory,
        ];
        for (i, d) in dims.iter().enumerate() {
            t.intake_and_verify(50, *d, &p, (i + 1) as u64 * 1_000_000)
                .unwrap();
        }
        for round in 0..10 {
            for (j, d) in dims.iter().enumerate() {
                let ns = ((round + 1) * 4 + j + 1) as u64 * 1_000_000;
                if t.intake_and_verify(50, *d, &p, ns).is_err() {
                    return;
                }
            }
            if t.verify_cross_dimension(&p).is_err() {
                return;
            }
        }
        assert!(t.verify_cross_dimension(&p).is_err());
    }
}
