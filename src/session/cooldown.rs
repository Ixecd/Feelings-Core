use crate::pbm::DefenceLevel;

const COOLDOWN_6H_NS: u64 = 6 * 3600 * 1_000_000_000;
const COOLDOWN_5MIN_NS: u64 = 5 * 60 * 1_000_000_000;

#[derive(Debug, Clone, Copy, Default)]
struct DimensionCooldown {
    last_high_intensity_ns: u64,
    breach_count: u32,
}

impl DimensionCooldown {
    fn is_cooling(&self, now_ns: u64) -> bool {
        if self.last_high_intensity_ns == 0 {
            return false;
        }
        now_ns.saturating_sub(self.last_high_intensity_ns) < COOLDOWN_6H_NS
    }

    fn attenuation(&self, now_ns: u64) -> f64 {
        if !self.is_cooling(now_ns) {
            return 1.0;
        }
        let elapsed = now_ns.saturating_sub(self.last_high_intensity_ns) as f64;

        if elapsed >= COOLDOWN_6H_NS as f64 {
            return 1.0;
        }
        if elapsed < COOLDOWN_5MIN_NS as f64 {
            return 0.2;
        }
        let factor = 0.2
            + 0.8
                * ((elapsed - COOLDOWN_5MIN_NS as f64)
                    / (COOLDOWN_6H_NS as f64 - COOLDOWN_5MIN_NS as f64));
        // 硬卡槽——clamp 在物理边界内。前面虽然有 >5min 守卫，
        // 但极端情况（时钟回拨、浮点累积误差）必须在同一行兜底。
        factor.clamp(0.2, 1.0)
    }
}

/// 经线冷却跟踪器——跨 Session 高强度维度冷却 + 防御层级渐进恢复。
#[derive(Debug, Clone)]
pub struct CooldownTracker {
    dimensions: [DimensionCooldown; 4],
    /// 跨 session 累计违规次数——触发 DefenceLevel 升级。
    cross_session_breach_count: u32,
    /// 当前经线保护层级——异步升级，渐进降级。
    current_level: DefenceLevel,
    /// 最近一次升级的时间戳——渐进降级计时器。
    last_escalation_ns: u64,
    /// 防御降级的最小间隔——D3→D2→D1 每步至少隔这么久。
    de_escalation_interval_ns: u64,
}

impl Default for CooldownTracker {
    fn default() -> Self {
        CooldownTracker {
            dimensions: [DimensionCooldown::default(); 4],
            cross_session_breach_count: 0,
            current_level: DefenceLevel::D1,
            last_escalation_ns: 0,
            de_escalation_interval_ns: 5 * 60 * 1_000_000_000,
        }
    }
}

impl CooldownTracker {
    pub fn from_config(config: &crate::config::CoreConfig) -> Self {
        CooldownTracker {
            de_escalation_interval_ns: (config.leaky_bucket.max_dt_seconds * 1_000_000_000.0)
                as u64,
            ..Default::default()
        }
    }

    pub fn record_high_intensity(&mut self, dim_idx: usize, now_ns: u64) {
        if dim_idx >= 4 {
            return;
        }
        self.dimensions[dim_idx].last_high_intensity_ns = now_ns;
        self.dimensions[dim_idx].breach_count += 1;
    }

    pub fn attenuation(&self, dim_idx: usize, now_ns: u64) -> f64 {
        if dim_idx >= 4 {
            return 1.0;
        }
        self.dimensions[dim_idx].attenuation(now_ns)
    }

    pub fn all_cool(&self, now_ns: u64) -> bool {
        self.dimensions.iter().all(|d| !d.is_cooling(now_ns))
    }

    /// 记录一次安全熔断——触发防御层级升级。
    /// 升级不受任何时间间隔限制——D2 后几毫秒内二次突破必须立即推入 D3。
    /// 安全不能等。只有降级需要冷却间隔。
    pub fn escalate(&mut self, now_ns: u64) -> DefenceLevel {
        self.cross_session_breach_count += 1;

        self.current_level = match self.current_level {
            DefenceLevel::D1 => DefenceLevel::D2,
            DefenceLevel::D2 | DefenceLevel::D3 => DefenceLevel::D3,
        };

        self.last_escalation_ns = now_ns;
        self.current_level
    }

    /// 渐进降级——距离上次升级已过冷却间隔 → 降一级。
    /// D3→D2→D1 每步至少间隔 de_escalation_interval_ns。
    pub fn try_de_escalate(&mut self, now_ns: u64) -> DefenceLevel {
        if self.current_level == DefenceLevel::D1 {
            return DefenceLevel::D1;
        }

        let elapsed = now_ns.saturating_sub(self.last_escalation_ns);
        if elapsed >= self.de_escalation_interval_ns {
            self.current_level = match self.current_level {
                DefenceLevel::D3 => DefenceLevel::D2,
                DefenceLevel::D2 => DefenceLevel::D1,
                other => other,
            };
            self.last_escalation_ns = now_ns;
        }
        self.current_level
    }

    pub fn current_level(&self) -> DefenceLevel {
        self.current_level
    }

    pub fn breach_count(&self) -> u32 {
        self.cross_session_breach_count
    }

    pub fn d3_duration_seconds(&self, now_ns: u64) -> f64 {
        if self.current_level != DefenceLevel::D3 {
            return 0.0;
        }
        now_ns.saturating_sub(self.last_escalation_ns) as f64 / 1_000_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_cooldown_initially() {
        let ct = CooldownTracker::default();
        assert!(ct.all_cool(0));
        assert!((ct.attenuation(0, 0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn record_high_intensity_starts_cooldown() {
        let mut ct = CooldownTracker::default();
        ct.record_high_intensity(0, 1_000_000_000);
        assert!(ct.attenuation(0, 1_000_000_001) < 1.0);
        assert!(!ct.all_cool(1_000_000_001));
    }

    #[test]
    fn attenuation_returns_after_cooldown() {
        let mut ct = CooldownTracker::default();
        ct.record_high_intensity(0, 1_000_000_000);
        let future = 1_000_000_000 + COOLDOWN_6H_NS + 1;
        assert!((ct.attenuation(0, future) - 1.0).abs() < 0.001);
        assert!(ct.all_cool(future));
    }

    #[test]
    fn strict_attenuation_first_5_minutes() {
        let mut ct = CooldownTracker::default();
        ct.record_high_intensity(0, 1_000_000_000);
        let t = 1_000_000_000 + 4 * 60 * 1_000_000_000;
        assert!((ct.attenuation(0, t) - 0.2).abs() < 0.001);
    }

    #[test]
    fn escalation_d1_to_d2() {
        let mut ct = CooldownTracker::default();
        assert_eq!(ct.current_level(), DefenceLevel::D1);
        let lvl = ct.escalate(1_000_000_000);
        assert_eq!(lvl, DefenceLevel::D2);
        assert_eq!(ct.breach_count(), 1);
    }

    #[test]
    fn de_escalation_after_interval() {
        let mut ct = CooldownTracker::default();
        ct.escalate(1_000_000_000);
        ct.escalate(2_000_000_000);
        assert_eq!(ct.current_level(), DefenceLevel::D3);

        let future = 2_000_000_000 + ct.de_escalation_interval_ns + 1;
        let lvl = ct.try_de_escalate(future);
        assert_eq!(lvl, DefenceLevel::D2);
    }

    #[test]
    fn independent_dimension_cooldowns() {
        let mut ct = CooldownTracker::default();
        ct.record_high_intensity(0, 1_000_000_000);
        assert!(ct.attenuation(0, 1_000_000_001) < 1.0);
        assert!((ct.attenuation(1, 1_000_000_001) - 1.0).abs() < 0.001);
    }

    #[test]
    fn attenuation_clamped_at_one() {
        let mut ct = CooldownTracker::default();
        ct.record_high_intensity(0, 1_000_000_000);
        // 6h boundary — must be exactly 1.0, never > 1.0
        let at_6h = 1_000_000_000 + COOLDOWN_6H_NS;
        assert!((ct.attenuation(0, at_6h) - 1.0).abs() < 0.001);
        // well past 6h
        let past = 1_000_000_000 + 2 * COOLDOWN_6H_NS;
        assert!((ct.attenuation(0, past) - 1.0).abs() < 0.001);
    }

    #[test]
    fn attenuation_boundary_at_5min_minus_one_ns() {
        let mut ct = CooldownTracker::default();
        ct.record_high_intensity(0, 1_000_000_000);
        // just before 5min boundary — still strict attenuation
        let t = 1_000_000_000 + COOLDOWN_5MIN_NS - 1;
        assert!((ct.attenuation(0, t) - 0.2).abs() < 0.001);
    }

    #[test]
    fn attenuation_boundary_at_6h_minus_one_ns() {
        let mut ct = CooldownTracker::default();
        ct.record_high_intensity(0, 1_000_000_000);
        // just before 6h boundary — should be near 1.0 but not clamped
        let t = 1_000_000_000 + COOLDOWN_6H_NS - 1;
        let a = ct.attenuation(0, t);
        assert!(a >= 0.99 && a <= 1.0);
    }

    #[test]
    fn escalation_no_interval_restriction() {
        let mut ct = CooldownTracker::default();
        ct.escalate(1_000_000_000);
        assert_eq!(ct.current_level(), DefenceLevel::D2);
        // escalate again 1ms later — must go to D3 immediately
        ct.escalate(1_000_000_000 + 1_000_000);
        assert_eq!(ct.current_level(), DefenceLevel::D3);
    }
}
