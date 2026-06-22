// src/pbm/convergence.rs — PBM 基线收敛算法
//
// Sigmoidal 缩放 + 四维差异化冷启动系数 + DampingMatrix 实时梯度 +
// 跨 Session 基线收敛 + DataConfidence 步长乘数 + 冷启动阻尼淡入窗
//
// 当前逻辑在 Anim src/pbm.rs 和 src/personalize.rs 里——迁移后移入本文件。
