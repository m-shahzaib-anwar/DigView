//! First-run consent. Only explicitly selected native folder grants become roots.
use crate::{
    db::{self, Result},
    engine::Engine,
    scan_policy::Preferences,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

// Version 1 also meant "old library detected"; version 2 requires real confirmation.
const SETUP_VERSION: i64 = 2;

#[derive(Default)]
pub struct Draft {
    folders: HashMap<String, (PathBuf, PathBuf)>,
}
#[derive(Clone, Serialize)]
pub struct Folder {
    pub token: String,
    pub path: String,
}
#[derive(Serialize)]
pub struct Status {
    pub completed: bool,
    pub version: i64,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub folders: Vec<String>,
    pub document_content: bool,
    pub exclusions: Vec<String>,
    pub low_resource: bool,
    pub pause_on_battery: bool,
}
pub fn init(c: &Connection, existing_library: bool) -> Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS setup_state(id INTEGER PRIMARY KEY CHECK(id=1),version INTEGER NOT NULL,completed_at INTEGER);").map_err(|e|e.to_string())?;
    c.execute(
        "INSERT OR IGNORE INTO setup_state VALUES(1,?,?)",
        params![i64::from(existing_library), existing_library.then(db::now)],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn status(c: &Connection) -> Result<Status> {
    c.query_row("SELECT version FROM setup_state WHERE id=1", [], |r| {
        let version: i64 = r.get(0)?;
        Ok(Status {
            completed: version >= SETUP_VERSION,
            version,
        })
    })
    .map_err(|e| e.to_string())
}
fn canonical_folder(path: &Path, engine: &Engine) -> Result<PathBuf> {
    let path =
        fs::canonicalize(path).map_err(|_| "This folder is unavailable. Choose it again.")?;
    if !path.is_dir() {
        return Err("Choose a folder.".into());
    }
    if engine.is_private_data(&path) {
        return Err("Choose a folder outside DigView's application data.".into());
    }
    if path.to_str().is_none() {
        return Err("This folder's path cannot be represented. Choose another folder.".into());
    }
    Ok(path)
}
impl Engine {
    pub fn setup_status(&self) -> Result<Status> {
        status(&db::connect(&self.0.db)?)
    }
    // Native picker or explicit known-folder checkbox, never a renderer-supplied path.
    pub fn grant_setup_folder(&self, path: &Path) -> Result<Folder> {
        if self.setup_status()?.completed {
            return Err("Setup is already complete. Add folders in Settings.".into());
        }
        self.grant_folder_choice(path)
    }
    pub fn grant_folder_choice(&self, path: &Path) -> Result<Folder> {
        let _gate = self.0.operation_gate.lock().map_err(|e| e.to_string())?;
        let canonical = canonical_folder(path, self)?;
        let mut draft = self.0.setup_draft.lock().map_err(|e| e.to_string())?;
        if let Some((token, _)) = draft.folders.iter().find(|(_, (_, p))| *p == canonical) {
            return Ok(Folder {
                token: token.clone(),
                path: canonical.to_string_lossy().into(),
            });
        }
        if draft.folders.len() >= 64 {
            return Err("Too many pending folder choices. Restart DigView to clear them.".into());
        }
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| "Could not authorize this folder. Try again.")?;
        let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        draft
            .folders
            .insert(token.clone(), (path.into(), canonical.clone()));
        Ok(Folder {
            token,
            path: canonical.to_string_lossy().into(),
        })
    }
    pub fn discard_folder_choices(&self, tokens: Vec<String>) -> Result<()> {
        if tokens.len() > 64 {
            return Err("Too many folder choices.".into());
        }
        let mut draft = self.0.setup_draft.lock().map_err(|e| e.to_string())?;
        for token in tokens {
            draft.folders.remove(&token);
        }
        Ok(())
    }
    // Add-folder confirmation after setup: no preference or completion changes.
    pub fn add_folder_choices(&self, tokens: Vec<String>) -> Result<Vec<crate::model::Root>> {
        let _gate = self
            .0
            .operation_gate
            .try_lock()
            .map_err(|_| "A file action is already in progress.")?;
        if !self.setup_status()?.completed {
            return Err("Finish setup before adding folders to the library.".into());
        }
        if self.job().running {
            return Err("Wait for the current library task before changing folders.".into());
        }
        if tokens.is_empty() || tokens.len() > 32 {
            return Err("Choose between 1 and 32 folders.".into());
        }
        let mut c = db::connect(&self.0.db)?;
        let mut draft = self.0.setup_draft.lock().map_err(|e| e.to_string())?;
        let mut seen = HashSet::new();
        let mut paths = db::roots(&c)?
            .into_iter()
            .map(|r| PathBuf::from(r.path))
            .collect::<Vec<_>>();
        let existing = paths.len();
        for token in &tokens {
            if !seen.insert(token) {
                return Err("A folder was selected more than once.".into());
            }
            let (original, approved) = draft
                .folders
                .get(token)
                .ok_or("A folder choice expired. Choose the folder again.")?;
            let path = canonical_folder(original, self)?;
            if path != *approved {
                return Err("A folder's location changed. Choose it again.".into());
            }
            if paths
                .iter()
                .any(|p| path.starts_with(p) || p.starts_with(&path))
            {
                return Err(
                    "Selected folders overlap. Keep either the parent or its subfolder.".into(),
                );
            }
            paths.push(path);
        }
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let added_at = db::now();
        let mut added = Vec::new();
        for path in paths.iter().skip(existing) {
            tx.execute(
                "INSERT INTO roots(path,added_at) VALUES(?,?)",
                params![path.to_string_lossy(), added_at],
            )
            .map_err(|e| e.to_string())?;
            added.push(crate::model::Root {
                id: tx.last_insert_rowid(),
                path: path.to_string_lossy().into_owned(),
                added_at,
                last_scan: None,
            });
        }
        tx.commit().map_err(|e| e.to_string())?;
        for token in &tokens {
            draft.folders.remove(token);
        }
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        Ok(added)
    }
    pub fn finish_setup(&self, request: Request) -> Result<Status> {
        let _gate = self.0.operation_gate.lock().map_err(|e| e.to_string())?;
        let mut c = db::connect(&self.0.db)?;
        let current = status(&c)?;
        if current.completed {
            return Ok(current);
        }
        if self.job().running {
            return Err("Finish or cancel the current library task first.".into());
        }
        if request.folders.len() > 32 {
            return Err("Choose at most 32 folders during setup.".into());
        }
        let mut preferences = Preferences {
            low_resource: request.low_resource,
            pause_on_battery: request.pause_on_battery,
            ..Default::default()
        };
        preferences.exclusions = request.exclusions;
        preferences.validate()?;
        let mut policy = self.0.scan_preferences.lock().map_err(|e| e.to_string())?;
        preferences.paused = policy.paused;
        let mut draft = self.0.setup_draft.lock().map_err(|e| e.to_string())?;
        let mut seen = HashSet::new();
        let mut paths = db::roots(&c)?
            .into_iter()
            .map(|r| PathBuf::from(r.path))
            .collect::<Vec<_>>();
        let existing = paths.len();
        for token in &request.folders {
            if !seen.insert(token) {
                return Err("A folder was selected more than once.".into());
            }
            let (original, approved) = draft
                .folders
                .get(token)
                .ok_or("A folder choice expired. Choose the folder again.")?;
            let path = canonical_folder(original, self)?;
            if path != *approved {
                return Err("A folder's location changed. Choose it again.".into());
            }
            if paths
                .iter()
                .any(|p| path.starts_with(p) || p.starts_with(&path))
            {
                return Err(
                    "Selected folders overlap. Keep either the parent or its subfolder.".into(),
                );
            }
            paths.push(path);
        }
        let tx = c.transaction().map_err(|e| e.to_string())?;
        for path in paths.iter().skip(existing) {
            tx.execute(
                "INSERT INTO roots(path,added_at) VALUES(?,?)",
                params![path.to_string_lossy(), db::now()],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute("INSERT INTO scan_preferences(id,value) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET value=excluded.value",[serde_json::to_string(&preferences).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
        tx.execute(
            "UPDATE document_settings SET enabled=? WHERE id=1",
            [request.document_content],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE setup_state SET version=?,completed_at=? WHERE id=1",
            params![SETUP_VERSION, db::now()],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        *policy = preferences;
        self.0
            .document_enabled
            .store(request.document_content, Ordering::Relaxed);
        self.0.policy_revision.fetch_add(1, Ordering::Relaxed);
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        draft.folders.clear();
        Ok(Status {
            completed: true,
            version: SETUP_VERSION,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(folders: Vec<String>) -> Request {
        Request {
            folders,
            ..Default::default()
        }
    }
    #[test]
    fn confirmed_batches_preserve_preferences_and_discarded_grants_cannot_commit() {
        let t = tempfile::tempdir().unwrap();
        let e = Engine::new(&t.path().join("data")).unwrap();
        let p = t.path().join("picked");
        fs::create_dir(&p).unwrap();
        let g = e.grant_folder_choice(&p).unwrap();
        assert!(e.add_folder_choices(vec![g.token.clone()]).is_err());
        e.finish_setup(Request {
            low_resource: true,
            document_content: true,
            ..Default::default()
        })
        .unwrap();
        let g = e.grant_folder_choice(&p).unwrap();
        e.discard_folder_choices(vec![g.token.clone()]).unwrap();
        assert!(e.add_folder_choices(vec![g.token]).is_err());
        let g = e.grant_folder_choice(&p).unwrap();
        assert_eq!(
            e.add_folder_choices(vec![g.token.clone()]).unwrap().len(),
            1
        );
        assert!(e.add_folder_choices(vec![g.token]).is_err());
        assert!(e.scan_status().unwrap().preferences.low_resource);
        assert!(e.0.document_enabled.load(Ordering::Relaxed));
        assert!(e.setup_status().unwrap().completed);
        assert_eq!(db::stats(&db::connect(&e.0.db).unwrap()).unwrap().count, 0);
    }
    #[test]
    fn batch_overlap_and_sql_failure_do_not_publish_partial_roots() {
        let t = tempfile::tempdir().unwrap();
        let e = Engine::new(&t.path().join("data")).unwrap();
        e.finish_setup(request(vec![])).unwrap();
        let a = t.path().join("a");
        let b = t.path().join("b");
        fs::create_dir_all(a.join("child")).unwrap();
        fs::create_dir(&b).unwrap();
        let ga = e.grant_folder_choice(&a).unwrap();
        let gb = e.grant_folder_choice(&b).unwrap();
        let child = e.grant_folder_choice(&a.join("child")).unwrap();
        assert!(e
            .add_folder_choices(vec![ga.token.clone(), child.token])
            .is_err());
        assert!(e
            .add_folder_choices(vec![ga.token.clone(), ga.token.clone()])
            .is_err());
        let c = db::connect(&e.0.db).unwrap();
        c.execute_batch("CREATE TRIGGER fail_batch BEFORE INSERT ON roots WHEN (SELECT COUNT(*) FROM roots)>0 BEGIN SELECT RAISE(ABORT,'fixture');END;").unwrap();
        assert!(e
            .add_folder_choices(vec![ga.token.clone(), gb.token.clone()])
            .is_err());
        assert!(db::roots(&c).unwrap().is_empty());
        c.execute("DROP TRIGGER fail_batch", []).unwrap();
        assert_eq!(
            e.add_folder_choices(vec![ga.token, gb.token])
                .unwrap()
                .len(),
            2
        );
    }
    #[test]
    fn choices_do_not_register_scan_roots_and_expire_on_restart() {
        let t = tempfile::tempdir().unwrap();
        let folder = t.path().join("picked");
        fs::create_dir(&folder).unwrap();
        let e = Engine::new(&t.path().join("data")).unwrap();
        let grant = e.grant_setup_folder(&folder).unwrap();
        assert!(!e.setup_status().unwrap().completed);
        assert!(db::roots(&db::connect(&e.0.db).unwrap())
            .unwrap()
            .is_empty());
        assert_eq!(db::stats(&db::connect(&e.0.db).unwrap()).unwrap().count, 0);
        let restarted = Engine::new(&t.path().join("data")).unwrap();
        assert!(restarted.finish_setup(request(vec![grant.token])).is_err());
        assert!(!restarted.setup_status().unwrap().completed);
    }
    #[test]
    fn finish_is_atomic_persistent_idempotent_and_does_not_read_contents() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("picked");
        fs::create_dir(&p).unwrap();
        fs::write(p.join("private.txt"), "PRIVATE CONTENT").unwrap();
        let e = Engine::new(&t.path().join("data")).unwrap();
        let grant = e.grant_setup_folder(&p).unwrap();
        let r = Request {
            folders: vec![grant.token],
            document_content: true,
            exclusions: vec!["*.log".into()],
            low_resource: true,
            ..Default::default()
        };
        assert!(e.finish_setup(r).unwrap().completed);
        assert_eq!(db::roots(&db::connect(&e.0.db).unwrap()).unwrap().len(), 1);
        assert_eq!(db::stats(&db::connect(&e.0.db).unwrap()).unwrap().count, 0);
        let e = Engine::new(&t.path().join("data")).unwrap();
        assert!(e.setup_status().unwrap().completed);
        assert!(e.0.document_enabled.load(Ordering::Relaxed));
        assert!(e.scan_status().unwrap().preferences.low_resource);
        assert!(
            e.finish_setup(request(vec!["untrusted".into()]))
                .unwrap()
                .completed
        );
        assert_eq!(db::roots(&db::connect(&e.0.db).unwrap()).unwrap().len(), 1);
    }
    #[test]
    fn invalid_changed_overlapping_and_outside_authority_choices_publish_nothing() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("picked");
        fs::create_dir_all(p.join("child")).unwrap();
        let e = Engine::new(&t.path().join("data")).unwrap();
        let parent = e.grant_setup_folder(&p).unwrap();
        let child = e.grant_setup_folder(&p.join("child")).unwrap();
        assert!(e
            .finish_setup(request(vec![parent.token.clone(), child.token]))
            .is_err());
        assert!(e
            .finish_setup(request(vec![parent.token.clone(), parent.token.clone()]))
            .is_err());
        assert!(e
            .finish_setup(request(vec![p.to_string_lossy().into()]))
            .is_err());
        let invalid = Request {
            folders: vec![parent.token.clone()],
            exclusions: vec!["../escape".into()],
            ..Default::default()
        };
        assert!(e.finish_setup(invalid).is_err());
        assert!(e.grant_setup_folder(&e.0.app_data).is_err());
        fs::rename(&p, t.path().join("moved")).unwrap();
        assert!(e.finish_setup(request(vec![parent.token])).is_err());
        assert!(db::roots(&db::connect(&e.0.db).unwrap())
            .unwrap()
            .is_empty());
        assert!(!e.setup_status().unwrap().completed);
        assert!(!e.0.document_enabled.load(Ordering::Relaxed));
    }
    #[test]
    fn existing_library_needs_confirmation_without_losing_roots() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("picked");
        fs::create_dir(&p).unwrap();
        let data = t.path().join("data");
        let e = Engine::new(&data).unwrap();
        e.add_root(&p).unwrap();
        db::connect(&e.0.db)
            .unwrap()
            .execute("DROP TABLE setup_state", [])
            .unwrap();
        drop(e);
        let e = Engine::new(&data).unwrap();
        assert!(!e.setup_status().unwrap().completed);
        assert_eq!(db::roots(&db::connect(&e.0.db).unwrap()).unwrap().len(), 1);
        assert!(e.finish_setup(request(vec![])).unwrap().completed);
        assert_eq!(db::roots(&db::connect(&e.0.db).unwrap()).unwrap().len(), 1);
    }
    #[test]
    fn empty_completion_and_failed_sql_preserve_correct_state() {
        let t = tempfile::tempdir().unwrap();
        let e = Engine::new(&t.path().join("data")).unwrap();
        let p = t.path().join("picked");
        fs::create_dir(&p).unwrap();
        let g = e.grant_setup_folder(&p).unwrap();
        db::connect(&e.0.db).unwrap().execute_batch("CREATE TRIGGER fail_setup BEFORE UPDATE ON setup_state BEGIN SELECT RAISE(ABORT,'fixture');END;").unwrap();
        assert!(e.finish_setup(request(vec![g.token])).is_err());
        assert!(db::roots(&db::connect(&e.0.db).unwrap())
            .unwrap()
            .is_empty());
        assert!(!e.setup_status().unwrap().completed);
        db::connect(&e.0.db)
            .unwrap()
            .execute("DROP TRIGGER fail_setup", [])
            .unwrap();
        assert!(e.finish_setup(request(vec![])).unwrap().completed);
        assert!(db::roots(&db::connect(&e.0.db).unwrap())
            .unwrap()
            .is_empty());
    }
}
