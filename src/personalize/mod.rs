// src/personalize/mod.rs — Pass 6: FSIR × PBM → PSIR
//
// 个人校准——FSIR 通用感受结构 → 织入个人基线 → PSIR
// 职责: sigmoidal_scale / PbmColdStartCoefficients / 冻结维度判定 /
//       damping_window_alpha + freeze_factor / 脱敏检测
//
// 当前逻辑在 Anim src/personalize.rs 里——迁移后移入本模块。

pub mod sigmoidal;
pub mod damping;
