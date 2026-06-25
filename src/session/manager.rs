use crate::config::CoreConfig;
use crate::pbm::persist::{PbmSnapshot, PbmStore};
use crate::pbm::state::{ColdStartGuard, DampingState, PbmUpdateStrategy, SessionLabel};
use crate::pbm::DefenceLevel;
use crate::session::anchor::PersonalityAnchor;
use crate::session::grounding::GroundingSignal;
use crate::session::lifecycle::{Session, SessionConfig, SessionId, SessionPhase};
use crate::species::FeelingTarget;
use crate::tracker::coupling::SafetyBreach;
use crate::tracker::{NeuroEnergyTracker, UserSafetyProfile};
use std::io;

pub struct SessionManager<const D: usize, S: FeelingTarget> {
    pub session: Session<D, S>,
    store: PbmStore,
    guard: ColdStartGuard,
    damping: DampingState,
    total_sessions: u32,
}

impl<const D: usize, S: FeelingTarget> SessionManager<D, S> {
    pub fn new(config: &CoreConfig, session_cfg: SessionConfig) -> io::Result<Self> {
        let species = S::species_name();
        let store = PbmStore::new(species)?;
        Self::from_store(store, config, session_cfg)
    }

    pub fn with_root(
        root: impl Into<std::path::PathBuf>,
        config: &CoreConfig,
        session_cfg: SessionConfig,
    ) -> io::Result<Self> {
        let species = S::species_name();
        let store = PbmStore::with_root(root, species)?;
        Self::from_store(store, config, session_cfg)
    }

    fn from_store(
        store: PbmStore,
        config: &CoreConfig,
        session_cfg: SessionConfig,
    ) -> io::Result<Self> {
        let (guard, damping, total_sessions) = match store.load_latest()? {
            Some(snap) => {
                let mut g = snap.cold_start_guard;
                g.complete_session();
                (g, snap.damping_state, snap.total_sessions + 1)
            }
            None => (
                ColdStartGuard::from_config(config),
                DampingState::from_config(config, true),
                1,
            ),
        };

        let profile = UserSafetyProfile::<D, S>::from_config(config);
        let tracker = NeuroEnergyTracker::<D, S>::from_config(config);

        let session = Session::new(
            SessionId(total_sessions as u64),
            session_cfg,
            profile,
            tracker,
        );

        Ok(SessionManager {
            session,
            store,
            guard,
            damping,
            total_sessions,
        })
    }

    pub fn is_cold_start(&self) -> bool {
        self.guard.is_cold_start()
    }

    pub fn session_label(&self) -> SessionLabel {
        if self.guard.is_cold_start() {
            SessionLabel::ColdStart
        } else {
            SessionLabel::Normal
        }
    }

    pub fn anchor_confidence(&self) -> Option<f64> {
        self.session.config.anchor_confidence
    }

    pub fn defence_level(&self) -> Option<DefenceLevel> {
        self.session.config.defence_level
    }

    pub fn set_personality(&mut self, anchor: PersonalityAnchor) {
        self.session.personality = anchor;
    }
    pub fn tick(
        &mut self,
        intensity: u32,
        dim: S::Dimension,
        now_ns: u64,
    ) -> Result<Option<GroundingSignal<D>>, SafetyBreach<S>> {
        if self.session.phase == SessionPhase::Aborted || self.session.phase == SessionPhase::Ended
        {
            return Ok(None);
        }

        if self.session.phase == SessionPhase::Init {
            self.session.phase = SessionPhase::Running;
        }

        let _ =
            self.session
                .tracker
                .intake_and_verify(intensity, dim, &self.session.profile, now_ns);
        self.session
            .tracker
            .verify_cross_dimension(&self.session.profile)?;
        self.session.tick_frame_window(dim);

        Ok(None)
    }
    pub fn update_damping(&mut self, values: &[f64; 4]) {
        self.damping.update(values);
    }

    pub fn handle_safety_breach(&mut self, source: S::Dimension) -> GroundingSignal<D> {
        let signal = self.session.handle_safety_breach(source);
        let _ = self.persist("aborted");
        signal
    }

    pub fn end_session(&mut self) -> io::Result<()> {
        self.session.phase = SessionPhase::Ended;
        self.guard.complete_session();
        self.persist("normal")
    }

    fn persist(&self, label: &str) -> io::Result<()> {
        let update_strategy = if label == "normal" {
            PbmUpdateStrategy::Full
        } else {
            PbmUpdateStrategy::SafetyOnly
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();

        let snapshot = PbmSnapshot {
            species: S::species_name().to_string(),
            session_id: self.session.id.0,
            session_count: self.guard.session_count,
            total_sessions: self.total_sessions,
            label: label.to_string(),
            cold_start_guard: self.guard.clone(),
            damping_state: self.damping.clone(),
            update_strategy: format!("{:?}", update_strategy),
            created_at: now,
        };
        self.store.save(&snapshot)?;
        Ok(())
    }

    pub fn session_count(&self) -> u32 {
        self.guard.session_count
    }

    pub fn total_sessions(&self) -> u32 {
        self.total_sessions
    }

    pub fn list_past_sessions(&self) -> io::Result<Vec<std::path::PathBuf>> {
        self.store.list_sessions()
    }

    pub fn damping_state(&self) -> &DampingState {
        &self.damping
    }

    pub fn cold_start_guard(&self) -> &ColdStartGuard {
        &self.guard
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

    fn human_manager_at(path: &str) -> SessionManager<{ Human::DIM_COUNT }, Human> {
        let config = CoreConfig::default();
        let session_cfg = SessionConfig::default_human();
        SessionManager::with_root(path, &config, session_cfg).expect("create manager")
    }

    fn isolated_manager() -> SessionManager<{ Human::DIM_COUNT }, Human> {
        let tmp = env::temp_dir().join("feelings_test_mgr_isolated");
        let _ = fs::remove_dir_all(&tmp);
        human_manager_at(tmp.to_str().unwrap())
    }

    #[test]
    fn new_manager_is_cold_start() {
        let mgr = isolated_manager();
        assert!(mgr.is_cold_start());
        assert_eq!(mgr.total_sessions(), 1);
    }

    #[test]
    fn tick_starts_session() {
        let mut mgr = isolated_manager();
        assert_eq!(mgr.session.phase, SessionPhase::Init);
        mgr.tick(50, HumanDimension::Visceral, 1_000_000).unwrap();
        assert_eq!(mgr.session.phase, SessionPhase::Running);
    }

    #[test]
    fn end_session_persists_and_loads() {
        let tmp = env::temp_dir().join("feelings_test_mgr_persist");
        let _ = fs::remove_dir_all(&tmp);
        let root = tmp.to_str().unwrap().to_string();

        let mut mgr = human_manager_at(&root);
        mgr.tick(30, HumanDimension::Visceral, 1_000_000).unwrap();
        mgr.end_session().expect("persist");

        let mut mgr2 = human_manager_at(&root);
        assert_eq!(mgr2.total_sessions(), 2);
        assert!(mgr2.session_count() > 0);
        mgr2.end_session().ok();
    }

    #[test]
    fn safety_breach_aborts_and_persists() {
        let mut mgr = isolated_manager();
        mgr.tick(50, HumanDimension::Visceral, 1_000_000).unwrap();

        let signal = mgr.handle_safety_breach(HumanDimension::Visceral);
        assert_eq!(mgr.session.phase, SessionPhase::Aborted);
        assert!(!signal.channels.is_empty() && signal.channels[0].intensity > 0);
    }
}
