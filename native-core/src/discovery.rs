//! Device-local bookmarks and identity-bound recognized image text.
use crate::{
    db::{self, Result},
    engine::{check_identity, hash_file, Engine},
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

pub fn init(c: &Connection) -> Result<()> {
    c.execute_batch("
      CREATE TABLE IF NOT EXISTS favourites(file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS saved_searches(id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT NOT NULL,state TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS image_text(file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,size INTEGER NOT NULL,modified_token TEXT NOT NULL,text TEXT NOT NULL);
      CREATE VIRTUAL TABLE IF NOT EXISTS image_text_fts USING fts5(text,content='image_text',content_rowid='file_id',tokenize='unicode61');
      CREATE TRIGGER IF NOT EXISTS image_text_ai AFTER INSERT ON image_text BEGIN INSERT INTO image_text_fts(rowid,text) VALUES(new.file_id,new.text); END;
      CREATE TRIGGER IF NOT EXISTS image_text_ad AFTER DELETE ON image_text BEGIN INSERT INTO image_text_fts(image_text_fts,rowid,text) VALUES('delete',old.file_id,old.text); END;
      CREATE TRIGGER IF NOT EXISTS image_text_au AFTER UPDATE ON image_text BEGIN
        INSERT INTO image_text_fts(image_text_fts,rowid,text) VALUES('delete',old.file_id,old.text);
        INSERT INTO image_text_fts(rowid,text) VALUES(new.file_id,new.text); END;
      CREATE TRIGGER IF NOT EXISTS image_text_invalidate AFTER UPDATE OF size,modified_token,path ON files
        WHEN old.size!=new.size OR old.modified_token!=new.modified_token OR old.path!=new.path BEGIN DELETE FROM image_text WHERE file_id=new.id; END;
    ").map_err(|e| e.to_string())
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SearchState {
    pub text: String,
    pub category: Option<String>,
    pub root_id: Option<i64>,
    pub source: Option<String>,
    pub cleanup: Option<String>,
    pub sort: String,
    pub favourites: bool,
}
impl SearchState {
    fn validate(&self) -> Result<()> {
        if self.text.len() > 2048
            || self.text.chars().count() > 512
            || !matches!(
                self.category.as_deref(),
                None | Some(
                    "images" | "videos" | "music" | "documents" | "archives" | "apps" | "other"
                )
            )
            || !matches!(
                self.cleanup.as_deref(),
                None | Some("large" | "screenshots" | "downloads")
            )
            || !matches!(self.sort.as_str(), "recent" | "oldest" | "name" | "size")
            || self.root_id.is_some_and(|id| id <= 0)
            || self
                .source
                .as_ref()
                .is_some_and(|s| s.len() > 253 || s.chars().any(char::is_control))
        {
            return Err("Invalid saved search filters".into());
        }
        Ok(())
    }
}
#[derive(Serialize)]
pub struct SavedSearch {
    pub id: i64,
    pub name: String,
    pub state: SearchState,
}
#[derive(Serialize)]
pub struct PersonalLibrary {
    pub favourite_ids: Vec<i64>,
    pub searches: Vec<SavedSearch>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct OcrIdentity {
    pub id: i64,
    pub path: String,
    pub size: i64,
    pub modified_token: String,
    pub digest: String,
}
impl Engine {
    pub fn finish_update(&self, result: Result<()>) {
        self.finish(std::time::Instant::now(), result);
    }
    pub fn personal_library(&self) -> Result<PersonalLibrary> {
        let c = db::connect(&self.0.db)?;
        let favourite_ids = c
            .prepare("SELECT file_id FROM favourites WHERE file_id IN (SELECT id FROM visible_files) ORDER BY file_id LIMIT 5000")
            .map_err(|e| e.to_string())?
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        let mut stmt = c
            .prepare("SELECT id,name,state FROM saved_searches ORDER BY id DESC LIMIT 100")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut searches = vec![];
        for row in rows {
            let (id, name, state) = row.map_err(|e| e.to_string())?;
            let state: SearchState = serde_json::from_str(&state).map_err(|e| e.to_string())?;
            state.validate()?;
            searches.push(SavedSearch { id, name, state });
        }
        Ok(PersonalLibrary {
            favourite_ids,
            searches,
        })
    }
    pub fn favourite(&self, id: i64, enabled: bool) -> Result<()> {
        let _gate = self.0.operation_gate.lock().map_err(|e| e.to_string())?;
        let mut c = db::connect(&self.0.db)?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        if enabled {
            db::file(&tx, id)?;
            let count: i64 = tx
                .query_row("SELECT count(*) FROM favourites", [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            if count >= 5000 {
                return Err("Favourite limit reached (5000)".into());
            }
            tx.execute("INSERT OR IGNORE INTO favourites VALUES(?)", [id])
                .map_err(|e| e.to_string())?;
        } else {
            tx.execute("DELETE FROM favourites WHERE file_id=?", [id])
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    pub fn save_search(&self, name: String, state: SearchState) -> Result<()> {
        state.validate()?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err("Use a search name of 1–80 characters".into());
        }
        let mut c = db::connect(&self.0.db)?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let count: i64 = tx
            .query_row("SELECT count(*) FROM saved_searches", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if count >= 100 {
            return Err("Saved search limit reached (100)".into());
        }
        tx.execute(
            "INSERT INTO saved_searches(name,state) VALUES(?,?)",
            params![
                name,
                serde_json::to_string(&state).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn remove_search(&self, id: i64) -> Result<()> {
        db::connect(&self.0.db)?
            .execute("DELETE FROM saved_searches WHERE id=?", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn ocr_identity(&self, id: i64) -> Result<OcrIdentity> {
        let (r, path) = self.checked_file(id)?;
        check_identity(&r, &path)?;
        if r.category != "images" || r.size > 40 * 1024 * 1024 {
            return Err("Choose an indexed image under 40 MB".into());
        }
        let digest = hash_file(&path, &AtomicBool::new(false))?;
        check_identity(&r, &path)?;
        Ok(OcrIdentity {
            id,
            path: r.path,
            size: r.size,
            modified_token: r.modified_token,
            digest,
        })
    }
    pub fn save_image_text(&self, identity: OcrIdentity, text: String) -> Result<()> {
        if text.len() > 512 * 1024 {
            return Err("Recognized text is too large to index (512 KiB limit)".into());
        }
        let _gate = self.0.operation_gate.lock().map_err(|e| e.to_string())?;
        let current = self.ocr_identity(identity.id)?;
        if current.path != identity.path
            || current.size != identity.size
            || current.modified_token != identity.modified_token
            || current.digest != identity.digest
        {
            return Err("Image changed during recognition; refresh and try again".into());
        }
        let c = db::connect(&self.0.db)?;
        let changed=c.execute("INSERT INTO image_text(file_id,size,modified_token,text) SELECT id,size,modified_token,? FROM files WHERE id=? AND path=? AND size=? AND modified_token=? ON CONFLICT(file_id) DO UPDATE SET text=excluded.text,size=excluded.size,modified_token=excluded.modified_token",
            params![text,identity.id,identity.path,identity.size,identity.modified_token]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Image index changed during recognition; refresh and try again".into());
        }
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    pub fn clear_image_text(&self) -> Result<()> {
        db::connect(&self.0.db)?
            .execute("DELETE FROM image_text", [])
            .map_err(|e| e.to_string())?;
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    pub fn image_text(&self, id: i64) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        let (record, path) = self.checked_file(id)?;
        check_identity(&record, &path)?;
        db::connect(&self.0.db)?
            .query_row(
                "SELECT text FROM image_text WHERE file_id=? AND size=? AND modified_token=?",
                params![id, record.size, record.modified_token],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())
    }
    pub fn update_snapshot(&self) -> Result<PathBuf> {
        let directory = self.0.app_data.join("update-backups");
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let target = directory.join(format!("library-{stamp}.sqlite3"));
        db::connect(&self.0.db)?
            .execute("VACUUM INTO ?", [target.to_string_lossy().as_ref()])
            .map_err(|e| e.to_string())?;
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Query, QueryResult};
    use std::fs;
    fn fixture() -> (tempfile::TempDir, Engine, PathBuf, i64) {
        let t =
            tempfile::tempdir_in(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target"))
                .unwrap();
        let root = t.path().join("selected");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("image.png"), b"generated image fixture").unwrap();
        let e = Engine::new(&t.path().join("data")).unwrap();
        e.add_root(&root).unwrap();
        e.scan_mode(false).unwrap();
        let id = db::query(&db::connect(&e.0.db).unwrap(), Query::default())
            .unwrap()
            .files[0]
            .id;
        (t, e, root, id)
    }
    fn search(e: &Engine, terms: &[&str], favourites: bool) -> QueryResult {
        db::query(
            &db::connect(&e.0.db).unwrap(),
            Query {
                terms: terms.iter().map(|s| s.to_string()).collect(),
                favourites,
                ..Default::default()
            },
        )
        .unwrap()
    }
    #[test]
    fn local_ocr_search_is_filterable_replaced_and_cleared() {
        let (_t, e, _root, id) = fixture();
        let identity = e.ocr_identity(id).unwrap();
        e.save_image_text(identity.clone(), "Invoice Zenith alpha".into())
            .unwrap();
        assert_eq!(
            e.image_text(id).unwrap().as_deref(),
            Some("Invoice Zenith alpha")
        );
        assert_eq!(search(&e, &["Zenith"], false).total, 1);
        assert_eq!(search(&e, &["image", "alpha"], false).total, 1);
        assert_eq!(search(&e, &["ext:pdf", "Zenith"], false).total, 0);
        assert_eq!(search(&e, &["\" OR private"], false).total, 0);
        e.save_image_text(identity, "Updated Beta".into()).unwrap();
        assert_eq!(search(&e, &["Zenith"], false).total, 0);
        assert_eq!(search(&e, &["Beta"], true).total, 0);
        e.favourite(id, true).unwrap();
        assert_eq!(search(&e, &["Beta"], true).total, 1);
        e.clear_image_text().unwrap();
        assert!(e.image_text(id).unwrap().is_none());
        assert_eq!(search(&e, &["Beta"], false).total, 0);
        assert_eq!(search(&e, &["image"], false).total, 1);
    }
    #[test]
    fn changed_image_rejects_stale_text_and_rescan_invalidates_old_text() {
        let (_t, e, root, id) = fixture();
        let identity = e.ocr_identity(id).unwrap();
        e.save_image_text(identity.clone(), "Oldcontent".into())
            .unwrap();
        fs::write(root.join("image.png"), b"changed").unwrap();
        assert!(e.save_image_text(identity, "stale".into()).is_err());
        e.scan_mode(false).unwrap();
        assert_eq!(search(&e, &["Oldcontent"], false).total, 0);
    }
    #[test]
    fn preserved_metadata_does_not_allow_changed_image_text() {
        let (_t, e, root, id) = fixture();
        let identity = e.ocr_identity(id).unwrap();
        let path = root.join("image.png");
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        fs::write(&path, vec![b'x'; identity.size as usize]).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(before))
            .unwrap();
        assert!(e.save_image_text(identity, "wrong".into()).is_err());
    }
    #[test]
    fn additive_migrations_and_update_snapshot_preserve_local_state() {
        let (_t, e, _root, id) = fixture();
        e.save_scan_preferences(crate::scan_policy::Preferences {
            low_resource: true,
            exclusions: vec!["Private".into()],
            ..Default::default()
        })
        .unwrap();
        e.favourite(id, true).unwrap();
        e.save_search(
            "Recent images".into(),
            SearchState {
                text: "images from last week".into(),
                sort: "size".into(),
                favourites: true,
                ..Default::default()
            },
        )
        .unwrap();
        e.save_image_text(e.ocr_identity(id).unwrap(), "Persistedword".into())
            .unwrap();
        db::init(&e.0.db).unwrap();
        db::init(&e.0.db).unwrap();
        let restarted = Engine::new(&e.0.app_data).unwrap();
        assert_eq!(
            restarted.personal_library().unwrap().favourite_ids,
            vec![id]
        );
        assert_eq!(
            restarted.personal_library().unwrap().searches[0].state.text,
            "images from last week"
        );
        assert_eq!(search(&restarted, &["Persistedword"], false).total, 1);
        let backup = e.update_snapshot().unwrap();
        let c = db::connect(&backup).unwrap();
        let preferences: String = c
            .query_row("SELECT value FROM scan_preferences WHERE id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        let preferences: crate::scan_policy::Preferences =
            serde_json::from_str(&preferences).unwrap();
        assert!(preferences.low_resource);
        assert_eq!(preferences.exclusions, vec!["Private"]);
        assert_eq!(db::stats(&c).unwrap().count, 1);
        assert_eq!(
            c.query_row("SELECT count(*) FROM saved_searches", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM favourites", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        e.remove_root(db::roots(&db::connect(&e.0.db).unwrap()).unwrap()[0].id)
            .unwrap();
        assert!(e.personal_library().unwrap().favourite_ids.is_empty());
        assert_eq!(search(&e, &["Persistedword"], false).total, 0);
        assert_eq!(e.personal_library().unwrap().searches.len(), 1);
    }
    #[test]
    fn saved_search_validation_and_ocr_limits_are_enforced() {
        let (_t, e, _root, id) = fixture();
        assert!(e
            .save_search(
                "".into(),
                SearchState {
                    sort: "recent".into(),
                    ..Default::default()
                }
            )
            .is_err());
        assert!(e
            .save_search(
                "Bad".into(),
                SearchState {
                    sort: "DROP TABLE files".into(),
                    ..Default::default()
                }
            )
            .is_err());
        assert!(e
            .save_image_text(e.ocr_identity(id).unwrap(), "x".repeat(512 * 1024 + 1))
            .is_err());
        e.save_search(
            "A".into(),
            SearchState {
                sort: "recent".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let search = e.personal_library().unwrap().searches.remove(0);
        e.remove_search(search.id).unwrap();
        assert!(e.personal_library().unwrap().searches.is_empty());
    }
}
