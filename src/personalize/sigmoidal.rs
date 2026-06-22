// src/personalize/sigmoidal.rs — Sigmoidal 个人强度缩放
//
// ADR 009 §一: sigmoidal_scale() 压缩因子公式
// low ≈ linear / mid slowdown / high near saturation
// 公式: compression(x) = 1 − α×σ(x), σ(x)=1/(1+e^{-k(x-x0)})
