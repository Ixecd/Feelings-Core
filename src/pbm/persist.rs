use crate::pbm::state::{ColdStartGuard, DampingState};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

const STORE_DIR: &str = ".feelings/pbm";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbmSnapshot {
    pub species: String,
    pub session_id: u64,
    pub session_count: u32,
    pub total_sessions: u32,
    pub label: String,
    pub cold_start_guard: ColdStartGuard,
    pub damping_state: DampingState,
    pub update_strategy: String,
    pub created_at: String,
}

#[derive(Debug)]
pub struct PbmStore {
    species_dir: PathBuf,
}

impl PbmStore {
    pub fn new(species: &str) -> io::Result<Self> {
        let home = env::var("HOME")
            .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "$HOME not set"))?;
        Self::with_root(home, species)
    }

    pub fn with_root(root: impl Into<PathBuf>, species: &str) -> io::Result<Self> {
        let species_dir = root.into().join(STORE_DIR).join(species);
        fs::create_dir_all(&species_dir)?;
        Ok(PbmStore { species_dir })
    }

    pub fn save(&self, snapshot: &PbmSnapshot) -> io::Result<PathBuf> {
        let filename = format!(
            "session_{}_{}.json",
            snapshot.total_sessions,
            sanitize_timestamp(&snapshot.created_at)
        );
        let path = self.species_dir.join(&filename);
        let json = serde_json::to_string_pretty(snapshot)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(&path, json)?;
        self.symlink_latest(&path)?;
        Ok(path)
    }

    pub fn load_latest(&self) -> io::Result<Option<PbmSnapshot>> {
        let latest_link = self.species_dir.join("latest.json");
        if !latest_link.exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&latest_link)?;
        let snapshot: PbmSnapshot = serde_json::from_str(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Some(snapshot))
    }

    pub fn list_sessions(&self) -> io::Result<Vec<PathBuf>> {
        let mut sessions: Vec<PathBuf> = fs::read_dir(&self.species_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().map(|ext| ext == "json").unwrap_or(false)
                    && p.file_name().map(|n| n != "latest.json").unwrap_or(false)
            })
            .collect();
        sessions.sort();
        Ok(sessions)
    }

    fn symlink_latest(&self, target: &PathBuf) -> io::Result<()> {
        let link = self.species_dir.join("latest.json");
        let _ = fs::remove_file(&link);
        fs::copy(target, &link)?;
        Ok(())
    }
}

fn sanitize_timestamp(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CoreConfig;

    fn temp_store() -> PbmStore {
        let tmp = std::env::temp_dir().join("feelings_test_pbm");
        let _ = fs::remove_dir_all(&tmp);
        PbmStore::with_root(&tmp, "human").expect("create store")
    }

    fn sample_snapshot(id: u64, count: u32, total: u32) -> PbmSnapshot {
        let config = CoreConfig::default();
        PbmSnapshot {
            species: "human".into(),
            session_id: id,
            session_count: count,
            total_sessions: total,
            label: "normal".into(),
            cold_start_guard: ColdStartGuard::from_config(&config),
            damping_state: DampingState::default(),
            update_strategy: "Full".into(),
            created_at: "1000".into(),
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let store = temp_store();
        let snap = sample_snapshot(1, 0, 1);
        let path = store.save(&snap).expect("save");
        assert!(path.exists());

        let loaded = store.load_latest().expect("load").expect("snapshot exists");
        assert_eq!(loaded.species, "human");
        assert_eq!(loaded.session_id, 1);
        assert_eq!(loaded.cold_start_guard.session_count, 0);
    }

    #[test]
    fn load_none_when_no_sessions() {
        let store = temp_store();
        assert!(store.load_latest().unwrap().is_none());
    }

    #[test]
    fn updates_latest_symlink() {
        let store = temp_store();
        let s1 = sample_snapshot(1, 0, 1);
        store.save(&s1).unwrap();

        let mut s2 = sample_snapshot(2, 1, 2);
        s2.cold_start_guard.complete_session();
        store.save(&s2).unwrap();

        let latest = store.load_latest().unwrap().unwrap();
        assert_eq!(latest.total_sessions, 2);
    }
}
