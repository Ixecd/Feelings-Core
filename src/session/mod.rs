// src/session/mod.rs — Session 生命周期管理
//
// Session: start / end / abort + 帧窗口计数器 + user_cap 管理 +
// DefenceLevel 变更 + 跨 Session 状态持久化
//
// 当前逻辑分散在 Anim pbm.rs / safety.rs / personalize.rs / pipeline.rs 里

pub mod lifecycle;
pub mod cooldown;
