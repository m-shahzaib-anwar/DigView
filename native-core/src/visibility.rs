//! Persistent visibility rules. These never modify originals or scan authority.
use crate::{
    db::{self, Result},
    engine::{registered_root, Engine},
};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::{path::Path, sync::atomic::Ordering};

#[derive(Serialize, Debug)]
pub struct HiddenItem {
    pub id: i64,
    pub path: String,
    pub folder: bool,
}

pub fn init(c: &Connection) -> Result<()> {
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS hidden_paths(
        id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT NOT NULL,
        path_key TEXT NOT NULL, folder INTEGER NOT NULL CHECK(folder IN (0,1)),
        UNIQUE(path_key,folder));
        CREATE INDEX IF NOT EXISTS hidden_paths_folders ON hidden_paths(folder,path_key);",
    )
    .map_err(|e| e.to_string())?;
    let key = path_key_sql("files.path");
    c.execute_batch(&format!(
        "CREATE VIEW IF NOT EXISTS visible_files AS
        SELECT files.* FROM files
        WHERE NOT EXISTS (SELECT 1 FROM hidden_paths h WHERE {key}=h.path_key)
        AND NOT EXISTS (SELECT 1 FROM hidden_paths h WHERE h.folder=1
          AND substr({key},1,length(h.path_key)+1)=h.path_key||'/');"
    ))
    .map_err(|e| e.to_string())
}

fn path_key_sql(value: &str) -> String {
    if cfg!(windows) {
        format!("lower(replace({value},char(92),'/'))")
    } else {
        value.to_owned()
    }
}

pub fn has_hidden(c: &Connection) -> Result<bool> {
    c.query_row("SELECT EXISTS(SELECT 1 FROM hidden_paths)", [], |r| {
        r.get(0)
    })
    .map_err(|e| e.to_string())
}

pub fn is_hidden(c: &Connection, id: i64) -> Result<bool> {
    c.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM visible_files WHERE id=?)",
        [id],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}

fn insert(c: &Connection, path: &Path, folder: bool) -> Result<()> {
    let path = path.to_string_lossy();
    // The only SQL interpolation is our own expression, never a filename.
    let key = path_key_sql("?");
    c.execute(
        &format!(
            "INSERT OR IGNORE INTO hidden_paths(path,path_key,folder) VALUES(?,rtrim({key},'/'),?)"
        ),
        params![path.as_ref(), path.as_ref(), folder],
    )
    .map_err(|e| e.to_string())?;
    let count: i64 = c
        .query_row("SELECT count(*) FROM hidden_paths", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if count > 1000 {
        return Err("Hidden item limit reached (1000). Unhide an item first.".into());
    }
    Ok(())
}

impl Engine {
    pub fn hidden_items(&self) -> Result<Vec<HiddenItem>> {
        let c = db::connect(&self.0.db)?;
        let mut stmt = c
            .prepare("SELECT id,path,folder FROM hidden_paths ORDER BY folder DESC,path,id")
            .map_err(|e| e.to_string())?;
        let items = stmt
            .query_map([], |r| {
                Ok(HiddenItem {
                    id: r.get(0)?,
                    path: r.get(1)?,
                    folder: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        Ok(items)
    }

    pub fn hide_files(&self, ids: Vec<i64>, containing_folders: bool) -> Result<()> {
        if ids.is_empty() || ids.len() > 500 {
            return Err("Choose between 1 and 500 files.".into());
        }
        let _gate = self.0.operation_gate.lock().map_err(|e| e.to_string())?;
        if self.job().running {
            return Err("Finish or cancel the library task before hiding items.".into());
        }
        let mut c = db::connect(&self.0.db)?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        // Validate every ID first; a bad batch must not partially change visibility.
        let paths = ids
            .into_iter()
            .map(|id| {
                let (_, path) = self.checked_file(id)?;
                if containing_folders {
                    Ok(path
                        .parent()
                        .ok_or("Containing folder unavailable")?
                        .to_path_buf())
                } else {
                    Ok(path)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        for path in paths {
            insert(&tx, &path, containing_folders)?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub fn hide_folder(&self, path: &Path) -> Result<()> {
        let _gate = self.0.operation_gate.lock().map_err(|e| e.to_string())?;
        if self.job().running {
            return Err("Finish or cancel the library task before hiding items.".into());
        }
        let canonical = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        if !canonical.is_dir() {
            return Err("Choose a folder inside your selected library folders.".into());
        }
        let mut c = db::connect(&self.0.db)?;
        let contained = db::roots(&c)?.into_iter().any(|root| {
            registered_root(Path::new(&root.path)).is_ok_and(|root| canonical.starts_with(root))
        });
        if !contained {
            return Err("Choose a folder inside your selected library folders.".into());
        }
        let tx = c.transaction().map_err(|e| e.to_string())?;
        insert(&tx, &canonical, true)?;
        tx.commit().map_err(|e| e.to_string())?;
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub fn hide_root(&self, id: i64) -> Result<()> {
        let root = db::roots(&db::connect(&self.0.db)?)?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or("Selected folder unavailable")?;
        let path = registered_root(Path::new(&root.path))?;
        self.hide_folder(&path)
    }

    pub fn unhide(&self, id: i64) -> Result<()> {
        let _gate = self.0.operation_gate.lock().map_err(|e| e.to_string())?;
        db::connect(&self.0.db)?
            .execute("DELETE FROM hidden_paths WHERE id=?", [id])
            .map_err(|e| e.to_string())?;
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        content_preview::PreviewStore,
        model::{FileRecord, Query, TrashSelection},
    };
    use std::{fs, path::PathBuf};

    fn fixture() -> (tempfile::TempDir, Engine, PathBuf) {
        let t = tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        let root = t.path().join("selected");
        fs::create_dir_all(root.join("Private %_ ' ویڈیو/nested")).unwrap();
        fs::create_dir_all(root.join("Private %_ ' ویڈیو-other")).unwrap();
        for name in [
            "Private %_ ' ویڈیو/first.txt",
            "Private %_ ' ویڈیو/nested/second.txt",
            "Private %_ ' ویڈیو-other/visible.txt",
        ] {
            fs::write(root.join(name), b"same generated content").unwrap();
        }
        fs::write(root.join("clip.mp4"), b"generated media fixture").unwrap();
        let e = Engine::new(&t.path().join("data")).unwrap();
        e.add_root(&root).unwrap();
        e.scan_mode(false).unwrap();
        (t, e, root)
    }
    fn files(e: &Engine) -> Vec<FileRecord> {
        db::query(
            &db::connect(&e.0.db).unwrap(),
            Query {
                limit: 200,
                ..Default::default()
            },
        )
        .unwrap()
        .files
    }
    fn named(e: &Engine, name: &str) -> FileRecord {
        files(e).into_iter().find(|f| f.name == name).unwrap()
    }

    #[test]
    fn hide_filters_search_favourites_counts_sources_cleanup_and_duplicates_without_deleting_index_rows(
    ) {
        let (_t, e, _root) = fixture();
        let file = named(&e, "first.txt");
        e.favourite(file.id, true).unwrap();
        let c = db::connect(&e.0.db).unwrap();
        c.execute("UPDATE files SET source='example.test',screenshot=1,modified=? WHERE category='documents'",[db::now()-100*86400]).unwrap();
        c.execute(
            "UPDATE files SET hash='generated-shared-hash' WHERE category='documents'",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO image_text(file_id,size,modified_token,text) VALUES(?,?,?,'secretclue')",
            params![file.id, file.size, file.modified_token],
        )
        .unwrap();
        e.hide_files(vec![file.id], false).unwrap();
        assert_eq!(db::query(&c, Query::default()).unwrap().total, 3);
        for query in [
            Query {
                terms: vec!["first".into()],
                ..Default::default()
            },
            Query {
                terms: vec!["secretclue".into()],
                ..Default::default()
            },
            Query {
                favourites: true,
                ..Default::default()
            },
        ] {
            assert_eq!(db::query(&c, query).unwrap().total, 0);
        }
        for query in [
            Query {
                category: Some("documents".into()),
                ..Default::default()
            },
            Query {
                source: Some("example.test".into()),
                ..Default::default()
            },
            Query {
                cleanup: Some("downloads".into()),
                ..Default::default()
            },
            Query {
                cleanup: Some("screenshots".into()),
                ..Default::default()
            },
            Query {
                min_size: Some(1),
                category: Some("documents".into()),
                ..Default::default()
            },
        ] {
            assert_eq!(db::query(&c, query).unwrap().total, 2);
        }
        let stats = db::stats(&c).unwrap();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.bytes, files(&e).iter().map(|f| f.size).sum::<i64>());
        assert_eq!(stats.sources[0].count, 2);
        assert_eq!(stats.screenshots, 2);
        assert_eq!(stats.downloads, 2);
        assert_eq!(stats.duplicate_groups, 1);
        assert_eq!(stats.duplicate_bytes, file.size);
        assert_eq!(db::duplicate_groups(&c).unwrap()[0].files.len(), 2);
        assert!(e.personal_library().unwrap().favourite_ids.is_empty());
        assert!(e.checked_file(file.id).is_err());
        assert_eq!(
            c.query_row("SELECT count(*) FROM files", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            4
        );
        e.unhide(e.hidden_items().unwrap()[0].id).unwrap();
        assert_eq!(
            db::query(
                &c,
                Query {
                    terms: vec!["secretclue".into()],
                    ..Default::default()
                }
            )
            .unwrap()
            .total,
            1
        );
        assert_eq!(e.personal_library().unwrap().favourite_ids, vec![file.id]);
        assert_eq!(db::stats(&c).unwrap().count, 4);
        assert_eq!(db::duplicate_groups(&c).unwrap()[0].files.len(), 3);
    }

    #[test]
    fn folder_rules_are_literal_component_bounded_and_cover_new_files_after_rescan_and_restart() {
        let (t, e, root) = fixture();
        let first = named(&e, "first.txt");
        e.hide_files(vec![first.id], true).unwrap();
        assert_eq!(files(&e).len(), 2);
        assert!(files(&e).iter().any(|f| f.name == "visible.txt"));
        fs::write(
            root.join("Private %_ ' ویڈیو/nested/new.txt"),
            b"new generated file",
        )
        .unwrap();
        e.scan_mode(false).unwrap();
        assert_eq!(files(&e).len(), 2);
        let reopened = Engine::new(&t.path().join("data")).unwrap();
        assert_eq!(files(&reopened).len(), 2);
        let c = db::connect(&reopened.0.db).unwrap();
        assert_eq!(
            db::query(
                &c,
                Query {
                    root_id: Some(first.root_id),
                    ..Default::default()
                }
            )
            .unwrap()
            .total,
            2
        );
        reopened
            .unhide(reopened.hidden_items().unwrap()[0].id)
            .unwrap();
        assert_eq!(files(&reopened).len(), 5);
        assert_eq!(
            fs::read(root.join("Private %_ ' ویڈیو/first.txt")).unwrap(),
            b"same generated content"
        );
    }

    #[test]
    fn nested_rules_restore_independently_and_exact_file_rules_survive_recreation_and_root_reimport(
    ) {
        let (_t, e, root) = fixture();
        let first = named(&e, "first.txt");
        e.hide_files(vec![first.id], false).unwrap();
        e.hide_folder(&root.join("Private %_ ' ویڈیو")).unwrap();
        let folder = e
            .hidden_items()
            .unwrap()
            .into_iter()
            .find(|r| r.folder)
            .unwrap();
        e.unhide(folder.id).unwrap();
        assert_eq!(files(&e).len(), 3);
        fs::remove_file(root.join("Private %_ ' ویڈیو/first.txt")).unwrap();
        e.scan_mode(false).unwrap();
        fs::write(root.join("Private %_ ' ویڈیو/first.txt"), b"replacement").unwrap();
        e.scan_mode(false).unwrap();
        assert_eq!(files(&e).len(), 3);
        e.remove_root(first.root_id).unwrap();
        e.add_root(&root).unwrap();
        e.scan_mode(false).unwrap();
        assert_eq!(files(&e).len(), 3);
        e.unhide(e.hidden_items().unwrap()[0].id).unwrap();
        assert_eq!(files(&e).len(), 4);
        assert_eq!(named(&e, "first.txt").size, 11);
    }

    #[test]
    fn batches_are_atomic_limits_are_transactional_and_outside_folders_are_refused() {
        let (t, e, _root) = fixture();
        let first = named(&e, "first.txt");
        assert!(e.hide_files(vec![first.id, i64::MAX], false).is_err());
        assert!(e.hidden_items().unwrap().is_empty());
        assert!(e.hide_files(vec![], false).is_err());
        assert!(e.hide_files(vec![first.id; 501], false).is_err());
        fs::create_dir(t.path().join("outside")).unwrap();
        assert!(e.hide_folder(&t.path().join("outside")).is_err());
        assert!(e.hide_root(i64::MAX).is_err());
        let mut c = db::connect(&e.0.db).unwrap();
        let tx = c.transaction().unwrap();
        for i in 0..1000 {
            tx.execute(
                "INSERT INTO hidden_paths(path,path_key,folder) VALUES(?,?,0)",
                params![format!("unused-{i}"), format!("unused-{i}")],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        assert!(e.hide_files(vec![first.id], false).is_err());
        assert_eq!(e.hidden_items().unwrap().len(), 1000);
        assert!(e.checked_file(first.id).is_ok());
    }

    #[test]
    fn hidden_media_revokes_access_and_stale_trash_requests_preserve_originals() {
        let (_t, e, root) = fixture();
        let file = named(&e, "clip.mp4");
        let store = PreviewStore::default();
        let token = store.open(&e, file.id).unwrap().token.unwrap();
        assert!(store.read(&e, &token, None, false).is_ok());
        e.hide_files(vec![file.id], false).unwrap();
        assert!(store.read(&e, &token, None, false).is_err());
        assert!(store.open(&e, file.id).is_err());
        assert!(crate::preview::thumbnail(&e, file.id, false).is_err());
        let result = e.trash(vec![TrashSelection::from(&file)], false).unwrap();
        assert!(result.trashed.is_empty());
        assert_eq!(result.failed.len(), 1);
        assert_eq!(
            fs::read(root.join("clip.mp4")).unwrap(),
            b"generated media fixture"
        );
        e.unhide(e.hidden_items().unwrap()[0].id).unwrap();
        assert!(store.read(&e, &token, None, false).is_ok());
    }

    #[test]
    fn root_hiding_deduplicates_rules_and_hidden_sizes_follow_scan_updates() {
        let (_t, e, root) = fixture();
        let file = named(&e, "clip.mp4");
        e.hide_root(file.root_id).unwrap();
        e.hide_root(file.root_id).unwrap();
        assert_eq!(e.hidden_items().unwrap().len(), 1);
        assert_eq!(db::stats(&db::connect(&e.0.db).unwrap()).unwrap().count, 0);
        fs::write(
            root.join("clip.mp4"),
            b"a different generated media payload",
        )
        .unwrap();
        e.scan_mode(false).unwrap();
        assert!(files(&e).is_empty());
        e.unhide(e.hidden_items().unwrap()[0].id).unwrap();
        let stats = db::stats(&db::connect(&e.0.db).unwrap()).unwrap();
        assert_eq!(stats.count, 4);
        assert_eq!(stats.bytes, files(&e).iter().map(|f| f.size).sum::<i64>());
    }
}
