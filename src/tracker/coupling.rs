// Feelings-Core — 跨维度耦合 + 全局桶 + 遥测安全熔断
//
// ADR 011: verify_cross_dimension() — 四维归一化 → σ 耦合 → 动态下调其他维度阈值。
// global_bucket: 四维能量 sum × global_alpha vs 全局阈值 global_beta。
// 超限返回 SafetyBreach 枚举——携带遥测数据，不给裸串。

use crate::pbm::{dim_index, PbmDimension, DIM_ORDER};
use crate::tracker::{NeuroEnergyTracker, UserSafetyProfile};

/// 安全熔断原因——携带遥测数据。
#[derive(Debug, Clone)]
pub enum SafetyBreach {
    /// 跨维度耦合过载。source=触发维度, current_energy=当前能量, suppressed_threshold=被压缩后的阈值。
    CrossDimCoupling {
        source: PbmDimension,
        current_energy: f64,
        suppressed_threshold: f64,
    },
    /// 全局桶超限。total_sum=四维能量和, limit=全局阈值。
    GlobalBucketOverload { total_sum: f64, limit: f64 },
}

impl NeuroEnergyTracker {
    /// 跨维度耦合校验 + 全局桶。超限返回 SafetyBreach 携带遥测数据。
    pub fn verify_cross_dimension(&self, profile: &UserSafetyProfile) -> Result<(), SafetyBreach> {
        let energies = self.all_energies();
        for i in 0..4 {
            let e_norm = energies[i] / profile.critical_thresholds[i].max(1.0);
            let coupling = self.sigma * e_norm;
            for (j, &energy_j) in energies.iter().enumerate() {
                if i == j {
                    continue;
                }
                let effective = profile.critical_thresholds[j] * (1.0 - coupling);
                if energy_j > effective {
                    return Err(SafetyBreach::CrossDimCoupling {
                        source: DIM_ORDER[i],
                        current_energy: energy_j,
                        suppressed_threshold: effective,
                    });
                }
            }
        }
        let energy_global: f64 = DIM_ORDER
            .iter()
            .map(|d| energies[dim_index(*d)])
            .sum::<f64>()
            * self.global_alpha;
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

    pub fn all_energies(&self) -> [f64; 4] {
        [
            self.energy(PbmDimension::Visceral),
            self.energy(PbmDimension::Emotional),
            self.energy(PbmDimension::Tactile),
            self.energy(PbmDimension::Auditory),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::ProfileKind;

    #[test]
    fn global_bucket_catches_alternating_attack() {
        let mut t = NeuroEnergyTracker::new();
        let p = UserSafetyProfile {
            kind: ProfileKind::Standard,
            leak_rates: [0.0; 4],
            critical_thresholds: [100.0; 4],
        };
        let dims = [
            PbmDimension::Visceral,
            PbmDimension::Emotional,
            PbmDimension::Tactile,
            PbmDimension::Auditory,
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
