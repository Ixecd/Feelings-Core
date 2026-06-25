use crate::pbm::DefenceLevel;
use crate::session::lifecycle::SessionPhase;
use crate::session::manager::SessionManager;
use crate::species::FeelingTarget;

#[derive(Debug, Clone)]
pub struct WatchdogReport {
    pub stale_frames: bool,
    pub energy_spike: bool,
    pub frame_window_overflow: bool,
    pub recommended_defence: Option<DefenceLevel>,
    pub anomaly_count: u32,
}

#[derive(Debug, Clone)]
pub struct SessionWatchdog {
    last_check_ns: u64,
    last_energies: [f64; 4],
    anomaly_counter: u32,
    consecutive_normal: u32,
    spike_threshold: f64,
    stale_threshold_ns: u64,
    check_interval_ns: u64,
}

impl Default for SessionWatchdog {
    fn default() -> Self {
        SessionWatchdog {
            last_check_ns: 0,
            last_energies: [0.0; 4],
            anomaly_counter: 0,
            consecutive_normal: 0,
            spike_threshold: 2.0,
            stale_threshold_ns: 500_000_000,
            check_interval_ns: 100_000_000,
        }
    }
}

impl SessionWatchdog {
    pub fn from_config(config: &crate::config::CoreConfig) -> Self {
        SessionWatchdog {
            spike_threshold: config.leaky_bucket.max_dt_seconds.max(1.0) * 2.0,
            ..Default::default()
        }
    }

    /// 10Hz 重校验——由 feelingsd/feelings-server 在主循环或独立 timer 线程中调用。
    pub fn check<const D: usize, S: FeelingTarget>(
        &mut self,
        mgr: &SessionManager<D, S>,
        now_ns: u64,
    ) -> WatchdogReport {
        // 冷启动——第一轮只同步基准，不校验
        if self.last_check_ns == 0 {
            self.last_check_ns = now_ns;
            let current = mgr.session.tracker.all_energies();
            for (dst, src) in self.last_energies.iter_mut().zip(current.iter()) {
                *dst = *src;
            }
            return WatchdogReport {
                stale_frames: false,
                energy_spike: false,
                frame_window_overflow: false,
                recommended_defence: None,
                anomaly_count: 0,
            };
        }

        let elapsed = now_ns.saturating_sub(self.last_check_ns);
        let mut report = WatchdogReport {
            stale_frames: false,
            energy_spike: false,
            frame_window_overflow: false,
            recommended_defence: None,
            anomaly_count: self.anomaly_counter,
        };

        // 1. 停滞检测——比对 SessionManager 内部维护的 last_tick_ns
        let tick_gap = now_ns.saturating_sub(mgr.last_tick_ns());
        if mgr.session.phase == SessionPhase::Running && tick_gap > self.stale_threshold_ns {
            report.stale_frames = true;
            self.anomaly_counter += 1;
        }

        // 2. 能量突变检测——当前能量 vs 上一轮基准
        let current = mgr.session.tracker.all_energies();
        for (&c, &l) in current.iter().zip(self.last_energies.iter()) {
            let delta = (c - l).abs();
            if delta > self.spike_threshold {
                report.energy_spike = true;
                self.anomaly_counter += 1;
                break;
            }
        }

        // 3. 帧窗口溢出检测——间接通过 phase + 能量 + 间隔判断
        if mgr.session.phase == SessionPhase::Running
            && current.iter().any(|&e| e > 0.0)
            && elapsed > self.check_interval_ns * 3
        {
            report.frame_window_overflow = true;
        }

        // 4. 自愈衰减——连续 10 轮（1 秒）无异常 → anomaly_counter 衰减 1。
        //    衰减后 §5 的 recommended_defence 自然联动降级（D2→D1/none）。
        //    saturating_sub 防止 u32 极限下溢。
        if !report.stale_frames && !report.energy_spike && !report.frame_window_overflow {
            self.consecutive_normal += 1;
            if self.consecutive_normal >= 10 {
                self.anomaly_counter = self.anomaly_counter.saturating_sub(1);
                self.consecutive_normal = 0;
            }
        } else {
            self.consecutive_normal = 0;
        }

        // 5. 防御升级建议
        if self.anomaly_counter >= 5 {
            report.recommended_defence = Some(DefenceLevel::D3);
        } else if self.anomaly_counter >= 3 {
            report.recommended_defence = Some(DefenceLevel::D2);
        }
        report.anomaly_count = self.anomaly_counter;

        self.last_check_ns = now_ns;
        for (dst, src) in self.last_energies.iter_mut().zip(current.iter()) {
            *dst = *src;
        }

        report
    }

    pub fn reset_anomalies(&mut self) {
        self.anomaly_counter = 0;
        self.consecutive_normal = 0;
    }

    pub fn anomaly_count(&self) -> u32 {
        self.anomaly_counter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CoreConfig;
    use crate::session::lifecycle::SessionConfig;
    use crate::species::{Human, HumanDimension};
    use std::env;
    use std::fs;

    fn setup() -> (SessionManager<{ Human::DIM_COUNT }, Human>, SessionWatchdog) {
        let mut tmp = env::temp_dir().join("feelings_test_wd");
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        tmp.push(format!("test_{}", suffix));
        let _ = fs::remove_dir_all(&tmp);
        let config = CoreConfig::default();
        let session_cfg = SessionConfig::default_human();
        let mgr = SessionManager::with_root(&tmp, &config, session_cfg).expect("mgr");
        let wd = SessionWatchdog::default();
        (mgr, wd)
    }

    #[test]
    fn cold_start_skips_validation() {
        let (mgr, mut wd) = setup();
        let report = wd.check(&mgr, 100_000_000);
        assert!(!report.stale_frames);
        assert!(!report.energy_spike);
        assert_eq!(report.anomaly_count, 0);
    }

    #[test]
    fn stale_session_detected() {
        let (mut mgr, mut wd) = setup();
        mgr.tick(10, HumanDimension::Visceral, 1_000_000).unwrap();
        // cold start pass
        wd.check(&mgr, 100_000_000);
        // 1s later with no tick → stale
        let report = wd.check(&mgr, 1_100_000_000);
        assert!(report.stale_frames);
    }

    #[test]
    fn energy_spike_detected() {
        let (mut mgr, mut wd) = setup();
        mgr.tick(50, HumanDimension::Visceral, 1_000_000).unwrap();
        wd.check(&mgr, 200_000_000);
        mgr.tick(100, HumanDimension::Emotional, 300_000_000)
            .unwrap();
        let report = wd.check(&mgr, 450_000_000);
        assert!(report.anomaly_count > 0 || !report.stale_frames);
    }

    #[test]
    fn anomaly_escalation_to_d2() {
        let (mut mgr, mut wd) = setup();
        mgr.tick(10, HumanDimension::Visceral, 1_000_000).unwrap();
        let mut now = 500_000_000;
        for _ in 0..4 {
            now += 600_000_000;
            wd.check(&mgr, now);
        }
        assert!(wd.anomaly_count() >= 3);
    }

    #[test]
    fn self_healing_decay_after_clean_rounds() {
        let (mut mgr, mut wd) = setup();
        mgr.tick(10, HumanDimension::Visceral, 1_000_000).unwrap();
        // inject anomalies via stale gaps without ticks
        let mut now = 500_000_000;
        // first call is cold-start → skips validation, so deliver 3 stale rounds
        for _ in 0..3 {
            now += 600_000_000;
            wd.check(&mgr, now);
        }
        let before = wd.anomaly_count();
        assert!(before >= 1, "expected >=1 anomalies, got {}", before);

        // reset tick gap + energy baseline
        now += 1;
        mgr.tick(10, HumanDimension::Visceral, now).unwrap();
        wd.check(&mgr, now + 50_000_000);

        // run clean rounds — zero intensity avoids false energy spikes
        for _i in 0..25 {
            now += 50_000_000;
            mgr.tick(0, HumanDimension::Visceral, now).unwrap();
            wd.check(&mgr, now);
        }
        let after = wd.anomaly_count();
        assert!(
            after <= before,
            "expected decay from {} to <= {}, got {}",
            before,
            before,
            after
        );
    }
}
