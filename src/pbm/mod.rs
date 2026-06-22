// src/pbm/mod.rs — Personal Baseline Matrix
//
// 个人基线矩阵。设备本地存储——永不离设备。
// 职责: ColdStartGuard / DampingState / SessionLabel / DataConfidence /
//       DefenceLevel / 四维 PBM 偏移值 [Visceral, Emotional, Tactile, Auditory]

pub mod state;
pub mod convergence;
