//! Local scan policy. Rules are relative names/paths, never new scan roots.
use crate::{
    db::{self, Result},
    engine::{registered_root, Engine},
};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::atomic::Ordering, time::Duration};

// Shared by full scans, notifications and checked-file actions. Avoid broad names
// such as build/dist/bin/vendor, which can contain a user's intended deliverables.
pub const DEFAULT_EXCLUSIONS: &[&str] = &[
    "node_modules",
    ".git",
    ".hg",
    ".svn",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".angular",
    ".parcel-cache",
    ".turbo",
    ".gradle",
    ".vs",
    ".Trash",
    ".trash",
    "$recycle.bin",
    "system volume information",
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    "*.tmp",
    "*.temp",
    "*.crdownload",
    "*.part",
    "*.partial",
    "*.pyc",
    "*.pyo",
];

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub paused: bool,
    pub low_resource: bool,
    pub pause_on_battery: bool,
    pub exclusions: Vec<String>,
}
impl Preferences {
    fn suspended_for(&self, battery: Option<bool>) -> bool {
        self.paused || (self.pause_on_battery && battery == Some(true))
    }
    pub fn validate(&mut self) -> Result<()> {
        if self.exclusions.len() > 100 {
            return Err("Use at most 100 exclusions".into());
        }
        for rule in &mut self.exclusions {
            *rule = rule.trim().replace('\\', "/");
            if rule.is_empty()
                || rule.len() > 512
                || rule.contains(':')
                || rule.starts_with('/')
                || rule.chars().any(char::is_control)
                || rule
                    .split('/')
                    .any(|p| p.is_empty() || p == "." || p == "..")
                || (rule.contains('*')
                    && !(rule.starts_with("*.")
                        && rule.len() > 2
                        && !rule[2..].contains(['*', '/'])))
            {
                return Err("Use relative folder/file paths, names, or extensions such as *.log; no absolute paths or ..".into());
            }
        }
        Ok(())
    }
    pub fn excludes(&self, path: &Path, root: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(root) else {
            return true;
        };
        let text = relative.to_string_lossy().replace('\\', "/");
        let normalize = |s: &str| {
            if cfg!(windows) {
                s.to_lowercase()
            } else {
                s.to_owned()
            }
        };
        let text = normalize(&text);
        DEFAULT_EXCLUSIONS
            .iter()
            .copied()
            .chain(self.exclusions.iter().map(String::as_str))
            .any(|r| {
                let r = normalize(r);
                if let Some(ext) = r.strip_prefix("*.") {
                    path.extension()
                        .is_some_and(|e| normalize(&e.to_string_lossy()) == ext)
                } else if r.contains('/') {
                    text == r || text.starts_with(&(r + "/"))
                } else {
                    text.split('/').any(|part| part == r)
                }
            })
    }
}
pub fn load(db_path: &Path) -> Result<Preferences> {
    let conn = db::connect(db_path)?;
    conn.execute("CREATE TABLE IF NOT EXISTS scan_preferences (id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL)", []).map_err(|e|e.to_string())?;
    let value = conn.query_row("SELECT value FROM scan_preferences WHERE id=1", [], |r| {
        r.get::<_, String>(0)
    });
    let mut preferences: Preferences = match value {
        Ok(value) => serde_json::from_str(&value).map_err(|e| e.to_string())?,
        Err(rusqlite::Error::QueryReturnedNoRows) => Preferences::default(),
        Err(e) => return Err(e.to_string()),
    };
    preferences.validate()?;
    Ok(preferences)
}

#[derive(Serialize)]
pub struct FolderStatus {
    pub id: i64,
    pub path: String,
    pub last_scan: Option<i64>,
    pub available: bool,
}
#[derive(Serialize)]
pub struct Status {
    pub preferences: Preferences,
    pub default_exclusions: Vec<String>,
    pub battery: Option<bool>,
    pub suspended: bool,
    pub running: bool,
    pub errors: u64,
    pub watch_error: String,
    pub folders: Vec<FolderStatus>,
}
impl Engine {
    pub fn scan_status(&self) -> Result<Status> {
        let preferences = self
            .0
            .scan_preferences
            .lock()
            .map_err(|e| e.to_string())?
            .clone();
        let battery = on_battery();
        let job = self.job();
        Ok(Status {
            default_exclusions: DEFAULT_EXCLUSIONS.iter().map(|r| r.to_string()).collect(),
            suspended: preferences.suspended_for(battery),
            preferences,
            battery,
            running: job.running,
            errors: job.errors,
            watch_error: job.watch_error,
            folders: db::roots(&db::connect(&self.0.db)?)?
                .into_iter()
                .map(|r| FolderStatus {
                    available: registered_root(Path::new(&r.path)).is_ok(),
                    id: r.id,
                    path: r.path,
                    last_scan: r.last_scan,
                })
                .collect(),
        })
    }
    pub fn save_scan_preferences(&self, mut preferences: Preferences) -> Result<()> {
        preferences.validate()?;
        let _gate = self
            .0
            .operation_gate
            .try_lock()
            .map_err(|_| "A file action is in progress")?;
        // Pause/resume can always interrupt a running worker. Rule changes wait for idle.
        let mut current = self.0.scan_preferences.lock().map_err(|e| e.to_string())?;
        if self.job().running && current.exclusions != preferences.exclusions {
            return Err("Cancel or finish the current task before changing exclusions".into());
        }
        db::connect(&self.0.db)?.execute("INSERT INTO scan_preferences(id,value) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET value=excluded.value",
            [serde_json::to_string(&preferences).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
        *current = preferences;
        self.0.policy_revision.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    pub(crate) fn suspended(&self) -> bool {
        let p = self
            .0
            .scan_preferences
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        p.suspended_for(on_battery())
    }
    pub(crate) fn checkpoint(&self) -> bool {
        while self.suspended() {
            if self.0.cancel.load(Ordering::Relaxed) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if self.0.cancel.load(Ordering::Relaxed) {
            return false;
        }
        if self
            .0
            .scan_preferences
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .low_resource
        {
            std::thread::sleep(Duration::from_millis(8));
        }
        !self.0.cancel.load(Ordering::Relaxed)
    }
    pub(crate) fn excluded(&self, path: &Path, root: &Path) -> bool {
        if self.is_private_data(path) {
            return true;
        }
        self.0
            .scan_preferences
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .excludes(path, root)
    }
}

#[cfg(windows)]
pub fn on_battery() -> Option<bool> {
    #[repr(C)]
    struct Power {
        ac: u8,
        flag: u8,
        percent: u8,
        reserved: u8,
        lifetime: u32,
        full: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemPowerStatus(status: *mut Power) -> i32;
    }
    let mut p = Power {
        ac: 255,
        flag: 255,
        percent: 255,
        reserved: 0,
        lifetime: 0,
        full: 0,
    };
    if unsafe { GetSystemPowerStatus(&mut p) } == 0 {
        return None;
    }
    match p.ac {
        0 => Some(true),
        1 => Some(false),
        _ => None,
    }
}
#[cfg(not(windows))]
pub fn on_battery() -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::mpsc, time::Instant};
    fn fixture() -> (tempfile::TempDir, Engine, std::path::PathBuf) {
        let temp =
            tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        let root = temp.path().join("selected");
        fs::create_dir(&root).unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        (temp, engine, root)
    }
    fn count(e: &Engine) -> i64 {
        db::stats(&db::connect(&e.0.db).unwrap()).unwrap().count
    }
    #[test]
    fn battery_policy_resumes_on_ac_and_unknown_power_does_not_freeze_work() {
        let mut p = Preferences {
            pause_on_battery: true,
            ..Default::default()
        };
        assert!(p.suspended_for(Some(true)));
        assert!(!p.suspended_for(Some(false)));
        assert!(!p.suspended_for(None));
        p.pause_on_battery = false;
        assert!(!p.suspended_for(Some(true)));
        p.paused = true;
        for battery in [Some(true), Some(false), None] {
            assert!(p.suspended_for(battery));
        }
    }
    #[test]
    fn relative_rules_boundaries_extensions_and_validation() {
        let (_temp, e, root) = fixture();
        let mut p = Preferences {
            exclusions: vec!["Private".into(), "Work/Secret".into(), "*.log".into()],
            ..Default::default()
        };
        p.validate().unwrap();
        for name in ["Private/a.txt", "Work/Secret/a.txt", "x/run.log"] {
            assert!(p.excludes(&root.join(name), &root));
        }
        for name in ["Private2/a.txt", "Work/Secret2/a.txt", "run.log.txt"] {
            assert!(!p.excludes(&root.join(name), &root));
        }
        assert!(p.excludes(&e.0.app_data, &root));
        for bad in [
            "../private",
            "C:/private",
            "/private",
            "x//y",
            "**",
            "a/./b",
            "*.a/b",
            "x\ny",
            "*.",
        ] {
            let mut p = Preferences {
                exclusions: vec![bad.into()],
                ..Default::default()
            };
            assert!(p.validate().is_err(), "{bad}");
        }
    }
    #[test]
    fn exclusions_reconcile_index_only_and_survive_restart() {
        let (_temp, e, root) = fixture();
        fs::create_dir(root.join("Private")).unwrap();
        fs::write(root.join("Private/a.txt"), b"private").unwrap();
        fs::write(root.join("keep.txt"), b"keep").unwrap();
        e.scan_mode(false).unwrap();
        assert_eq!(count(&e), 2);
        e.save_scan_preferences(Preferences {
            exclusions: vec!["Private".into()],
            low_resource: true,
            ..Default::default()
        })
        .unwrap();
        let private_id: i64 = db::connect(&e.0.db)
            .unwrap()
            .query_row("SELECT id FROM files WHERE name='a.txt'", [], |r| r.get(0))
            .unwrap();
        assert!(e.checked_file(private_id).is_err());
        e.scan_mode(false).unwrap();
        assert_eq!(count(&e), 1);
        assert!(root.join("Private/a.txt").is_file());
        let restarted = Engine::new(&e.0.app_data).unwrap();
        assert!(restarted.scan_status().unwrap().preferences.low_resource);
        restarted.scan_mode(false).unwrap();
        assert_eq!(count(&restarted), 1);
        restarted
            .save_scan_preferences(Preferences::default())
            .unwrap();
        restarted.scan_mode(false).unwrap();
        assert_eq!(count(&restarted), 2);
    }
    #[test]
    fn pause_resume_and_cancel_at_worker_checkpoint() {
        let (_temp, e, _root) = fixture();
        e.save_scan_preferences(Preferences {
            paused: true,
            ..Default::default()
        })
        .unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = e.clone();
        let handle = std::thread::spawn(move || tx.send(worker.checkpoint()).unwrap());
        assert!(rx.recv_timeout(Duration::from_millis(150)).is_err());
        e.save_scan_preferences(Preferences::default()).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(2)).unwrap());
        handle.join().unwrap();
        e.save_scan_preferences(Preferences {
            paused: true,
            ..Default::default()
        })
        .unwrap();
        e.0.cancel.store(true, Ordering::Relaxed);
        assert!(!e.checkpoint());
    }
    #[test]
    fn unavailable_root_preserves_index_and_reconnect_recovers() {
        let (temp, e, root) = fixture();
        fs::write(root.join("old.txt"), b"old").unwrap();
        e.scan_mode(false).unwrap();
        assert_eq!(count(&e), 1);
        let detached = temp.path().join("detached");
        fs::rename(&root, &detached).unwrap();
        assert!(!e.scan_status().unwrap().folders[0].available);
        e.scan_mode(false).unwrap();
        assert_eq!(count(&e), 1);
        fs::write(detached.join("new.txt"), b"new").unwrap();
        fs::rename(&detached, &root).unwrap();
        assert!(e.scan_status().unwrap().folders[0].available);
        e.scan_mode(false).unwrap();
        assert_eq!(count(&e), 2);
    }
    #[test]
    fn rule_changes_are_refused_during_task_and_finish_is_honest() {
        let (_temp, e, _root) = fixture();
        e.begin("scan").unwrap();
        assert!(e
            .save_scan_preferences(Preferences {
                exclusions: vec!["Private".into()],
                ..Default::default()
            })
            .is_err());
        e.save_scan_preferences(Preferences {
            paused: true,
            ..Default::default()
        })
        .unwrap();
        assert!(e.scan_status().unwrap().suspended);
        e.0.job.lock().unwrap().errors = 1;
        e.finish(Instant::now(), Ok(()));
        assert!(!e.job().message.contains("up to date"));
    }
}
