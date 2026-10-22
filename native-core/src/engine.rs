use crate::{
    db::{self, Result},
    model::*,
};
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Instant, UNIX_EPOCH},
};
use walkdir::WalkDir;

pub struct Inner {
    pub setup_draft: Mutex<crate::setup::Draft>,
    pub app_data: PathBuf,
    pub private_data_dirs: Vec<PathBuf>,
    pub db: PathBuf,
    pub cache: PathBuf,
    pub cache_bytes: AtomicU64,
    pub job: Mutex<Job>,
    pub cancel: AtomicBool,
    pub thumbnail_gate: Mutex<()>,
    pub operation_gate: Mutex<()>,
    pub revision: AtomicU64,
    pub watch_error: Mutex<String>,
    pub scan_preferences: Mutex<crate::scan_policy::Preferences>,
    pub policy_revision: AtomicU64,
    pub document_enabled: AtomicBool,
}
#[derive(Clone)]
pub struct Engine(pub Arc<Inner>);
impl Engine {
    pub fn new(data: &Path) -> Result<Self> {
        Self::new_with_private_data(data, &[])
    }
    pub fn new_with_private_data(data: &Path, private_data_dirs: &[PathBuf]) -> Result<Self> {
        fs::create_dir_all(data).map_err(|e| e.to_string())?;
        let cache = data.join("thumbnails");
        fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
        let db = data.join("library.sqlite3");
        db::init(&db)?;
        let preferences = crate::scan_policy::load(&db)?;
        let document_enabled = crate::document_index::enabled(&db::connect(&db)?)?;
        Ok(Self(Arc::new(Inner {
            setup_draft: Mutex::new(crate::setup::Draft::default()),
            app_data: fs::canonicalize(data).map_err(|e| e.to_string())?,
            private_data_dirs: private_data_dirs
                .iter()
                .map(|p| fs::canonicalize(p).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>>>()?,
            db,
            cache,
            cache_bytes: AtomicU64::new(0),
            job: Mutex::new(Job::default()),
            cancel: AtomicBool::new(false),
            thumbnail_gate: Mutex::new(()),
            operation_gate: Mutex::new(()),
            revision: AtomicU64::new(0),
            watch_error: Mutex::new(String::new()),
            scan_preferences: Mutex::new(preferences),
            policy_revision: AtomicU64::new(0),
            document_enabled: AtomicBool::new(document_enabled),
        })))
    }
    pub(crate) fn is_private_data(&self, path: &Path) -> bool {
        path.starts_with(&self.0.app_data)
            || self.0.private_data_dirs.iter().any(|p| path.starts_with(p))
    }
    pub fn job(&self) -> Job {
        let mut job = self.0.job.lock().unwrap_or_else(|e| e.into_inner()).clone();
        job.revision = self.0.revision.load(Ordering::Relaxed);
        job.watch_error = self
            .0
            .watch_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        job
    }
    fn update(&self, f: impl FnOnce(&mut Job)) {
        f(&mut self.0.job.lock().unwrap_or_else(|e| e.into_inner()));
    }
    pub fn begin(&self, kind: &str) -> Result<()> {
        let _guard = self
            .0
            .operation_gate
            .try_lock()
            .map_err(|_| "A file action is already in progress.")?;
        let mut job = self.0.job.lock().map_err(|e| e.to_string())?;
        if job.running {
            return Err(
                "A library task is already running. Cancel or wait for it to finish.".into(),
            );
        }
        self.0.cancel.store(false, Ordering::Relaxed);
        *job = Job {
            running: true,
            kind: kind.into(),
            message: "Preparing…".into(),
            ..Default::default()
        };
        Ok(())
    }
    pub fn start_scan(&self) -> Result<()> {
        self.begin("scan")?;
        let engine = self.clone();
        std::thread::spawn(move || {
            let started = Instant::now();
            let result = engine.scan();
            engine.finish(started, result);
        });
        Ok(())
    }
    pub fn start_duplicates(&self) -> Result<()> {
        self.begin("duplicates")?;
        let engine = self.clone();
        std::thread::spawn(move || {
            let started = Instant::now();
            let result = engine.find_duplicates();
            engine.finish(started, result);
        });
        Ok(())
    }
    pub(crate) fn finish(&self, started: Instant, result: Result<()>) {
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        let cancelled = self.0.cancel.load(Ordering::Relaxed);
        self.update(|j| {
            j.running = false;
            j.cancelled = cancelled;
            j.duration_ms = started.elapsed().as_millis() as u64;
            j.message = match result {
                Err(e) => {
                    j.errors += 1;
                    e
                }
                Ok(()) if cancelled => "Cancelled. Existing indexed files are preserved.".into(),
                Ok(()) if j.errors > 0 => {
                    "Finished with skipped files or unavailable folders. Review scan status.".into()
                }
                Ok(()) => "Library is up to date".into(),
            };
        });
    }
    pub fn add_root(&self, path: &Path) -> Result<Root> {
        let _guard = self
            .0
            .operation_gate
            .try_lock()
            .map_err(|_| "A file action is already in progress.")?;
        if self.job().running {
            return Err("Wait for the current library task before changing folders.".into());
        }
        let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
        if !path.is_dir() {
            return Err("Choose a folder.".into());
        }
        let data = fs::canonicalize(
            self.0
                .db
                .parent()
                .ok_or("Application data directory is unavailable")?,
        )
        .map_err(|e| e.to_string())?;
        if path.starts_with(&data) || self.is_private_data(&path) {
            return Err("Choose a folder outside DigView's application data.".into());
        }
        let conn = db::connect(&self.0.db)?;
        for root in db::roots(&conn)? {
            let existing = Path::new(&root.path);
            if path.starts_with(existing) || existing.starts_with(&path) {
                return Err("This folder overlaps a folder already in your library.".into());
            }
        }
        conn.execute(
            "INSERT INTO roots(path,added_at) VALUES(?,?)",
            params![path.to_string_lossy(), db::now()],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        Ok(Root {
            id,
            path: path.to_string_lossy().into(),
            added_at: db::now(),
            last_scan: None,
        })
    }
    pub fn remove_root(&self, id: i64) -> Result<()> {
        let _guard = self
            .0
            .operation_gate
            .try_lock()
            .map_err(|_| "A file action is already in progress.")?;
        if self.job().running {
            return Err("Wait for the current library task before changing folders.".into());
        }
        db::connect(&self.0.db)?
            .execute("DELETE FROM roots WHERE id=?", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn checked_file(&self, id: i64) -> Result<(FileRecord, PathBuf)> {
        let conn = db::connect(&self.0.db)?;
        let record = db::file(&conn, id)?;
        if crate::visibility::is_hidden(&conn, id)? {
            return Err("This item is hidden from the app. Unhide it in Settings first.".into());
        }
        let root: String = conn
            .query_row("SELECT path FROM roots WHERE id=?", [record.root_id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        let root = registered_root(Path::new(&root))?;
        if self.excluded(Path::new(&record.path), &root) {
            return Err("This file is excluded from the library. Refresh the index.".into());
        }
        let path = validate_path(Path::new(&record.path), &root)?;
        Ok((record, path))
    }
    fn scan(&self) -> Result<()> {
        self.scan_mode(false).map(|_| ())
    }
    // Automatic recovery scans defer recently written files without reconciling
    // their old rows away. A later retry handles files still being downloaded.
    pub(crate) fn scan_mode(&self, automatic: bool) -> Result<bool> {
        let mut deferred = false;
        let mut conn = db::connect(&self.0.db)?;
        let roots = db::roots(&conn)?;
        if roots.is_empty() {
            return if automatic {
                Ok(false)
            } else {
                Err("Add a folder to start your library.".into())
            };
        }
        let generation = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i64;
        let app_data = fs::canonicalize(
            self.0
                .db
                .parent()
                .ok_or("Application data directory is unavailable")?,
        )
        .map_err(|e| e.to_string())?;
        let mut processed = 0u64;
        for root in roots {
            if !self.checkpoint() {
                break;
            }
            let root_path = Path::new(&root.path);
            if registered_root(root_path).is_err() {
                self.update(|j| {
                    j.errors += 1;
                    j.message = format!("Folder unavailable: {}", root.path);
                });
                continue;
            }
            let mut batch: Vec<FileRecord> = Vec::with_capacity(256);
            let mut root_errors = 0;
            self.update(|j| j.message = format!("Scanning {}", display_path(root_path)));
            let walk = WalkDir::new(root_path)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    if e.path().starts_with(&app_data) || self.excluded(e.path(), root_path) {
                        return false;
                    }
                    true
                });
            for entry in walk {
                if !self.checkpoint() {
                    break;
                }
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => {
                        root_errors += 1;
                        continue;
                    }
                };
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                if path.starts_with(&app_data) {
                    continue;
                }
                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => {
                        root_errors += 1;
                        continue;
                    }
                };
                let name = entry.file_name().to_string_lossy().into_owned();
                let extension = path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if matches!(
                    extension.as_str(),
                    "crdownload" | "part" | "partial" | "tmp"
                ) {
                    continue;
                }
                if automatic
                    && metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|age| age < std::time::Duration::from_secs(2))
                {
                    conn.execute(
                        "UPDATE files SET seen=? WHERE path=?",
                        params![generation, path.to_string_lossy()],
                    )
                    .map_err(|e| e.to_string())?;
                    deferred = true;
                    continue;
                }
                let category = classify(&extension).to_owned();
                let screenshot = category == "images" && is_screenshot(&name);
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let created = metadata
                    .created()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);
                let modified_token = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos().to_string())
                    .unwrap_or_default();
                batch.push(FileRecord {
                    id: 0,
                    root_id: root.id,
                    path: path.to_string_lossy().into(),
                    name,
                    extension,
                    category,
                    size: metadata.len().min(i64::MAX as u64) as i64,
                    modified,
                    created,
                    source: crate::platform::origin(path),
                    screenshot,
                    hash: None,
                    modified_token,
                });
                processed += 1;
                if batch.len() >= 256 {
                    flush(&mut conn, &batch, generation)?;
                    batch.clear();
                    self.update(|j| {
                        j.processed = processed;
                        j.message =
                            format!("Scanning {} · {} files", display_path(root_path), processed);
                    });
                }
            }
            if !batch.is_empty() {
                flush(&mut conn, &batch, generation)?;
            }
            if registered_root(root_path).is_err() {
                root_errors += 1;
            }
            if !self.0.cancel.load(Ordering::Relaxed) && root_errors == 0 {
                let tx = conn.transaction().map_err(|e| e.to_string())?;
                tx.execute(
                    "DELETE FROM files WHERE root_id=? AND seen<>?",
                    params![root.id, generation],
                )
                .map_err(|e| e.to_string())?;
                tx.execute(
                    "UPDATE roots SET last_scan=? WHERE id=?",
                    params![db::now(), root.id],
                )
                .map_err(|e| e.to_string())?;
                tx.commit().map_err(|e| e.to_string())?;
            }
            self.update(|j| {
                j.processed = processed;
                j.errors += root_errors;
            });
        }
        Ok(deferred)
    }
    fn find_duplicates(&self) -> Result<()> {
        let conn = db::connect(&self.0.db)?;
        let mut stmt=conn.prepare(&format!("SELECT {FILE_COLUMNS} FROM visible_files WHERE size IN (SELECT size FROM visible_files WHERE size>0 GROUP BY size HAVING count(*)>1) ORDER BY size ASC")).map_err(|e|e.to_string())?;
        let candidates = stmt
            .query_map([], map_file)
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        drop(conn);
        let total = candidates.len();
        // Read streams sequentially to avoid saturating the disk or allocating whole files.
        for (index, record) in candidates.into_iter().enumerate() {
            if !self.checkpoint() {
                break;
            }
            self.update(|j| {
                j.processed = index as u64;
                j.message = format!("Checking {} of {} · {}", index + 1, total, record.name);
            });
            let result: Result<()> = (|| {
                let (_, path) = self.checked_file(record.id)?;
                check_identity(&record, &path)?;
                // Rehash candidates on explicit duplicate checks: metadata alone cannot prove content equality.
                let hash = hash_file_controlled(&path, &self.0.cancel, || self.checkpoint())?;
                check_identity(&record, &path)?;
                db::connect(&self.0.db)?
                    .execute(
                        "UPDATE files SET hash=? WHERE id=? AND size=? AND modified=?",
                        params![hash, record.id, record.size, record.modified],
                    )
                    .map_err(|e| e.to_string())?;
                Ok(())
            })();
            if result.is_err() && !self.0.cancel.load(Ordering::Relaxed) {
                self.update(|j| j.errors += 1);
            }
            self.update(|j| j.processed = (index + 1) as u64);
        }
        Ok(())
    }
    pub fn trash(&self, files: Vec<TrashSelection>, duplicate_only: bool) -> Result<TrashResult> {
        let _guard = self
            .0
            .operation_gate
            .try_lock()
            .map_err(|_| "A file action is already in progress.")?;
        if self.job().running {
            return Err("Finish or cancel the library task before cleanup.".into());
        }
        if files.is_empty() || files.len() > 500 {
            return Err("Choose between 1 and 500 files.".into());
        }
        let selected: HashSet<i64> = files.iter().map(|f| f.id).collect();
        let mut result = TrashResult {
            trashed: vec![],
            failed: vec![],
        };
        for id in &selected {
            let action = (|| {
                let (record, path) = self.checked_file(*id)?;
                check_identity(&record, &path)?;
                let approved = files
                    .iter()
                    .find(|f| f.id == *id)
                    .ok_or("Missing reviewed file identity")?;
                if approved.path != record.path
                    || approved.size != record.size
                    || approved.modified_token != record.modified_token
                {
                    return Err(
                        "This file changed after it was selected. Review it again before cleanup."
                            .into(),
                    );
                }
                if duplicate_only {
                    let hash = record
                        .hash
                        .as_ref()
                        .ok_or("Run an exact duplicate check first.")?;
                    let conn = db::connect(&self.0.db)?;
                    let mut stmt = conn
                        .prepare(&format!(
                            "SELECT {FILE_COLUMNS} FROM visible_files WHERE hash=? AND size=?"
                        ))
                        .map_err(|e| e.to_string())?;
                    let peers = stmt
                        .query_map(params![hash, record.size], map_file)
                        .map_err(|e| e.to_string())?
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .map_err(|e| e.to_string())?;
                    let mut keeper = false;
                    for peer in peers.iter().filter(|p| !selected.contains(&p.id)) {
                        if let Ok((_, p)) = self.checked_file(peer.id) {
                            if p != path
                                && check_identity(peer, &p).is_ok()
                                && hash_file(&p, &AtomicBool::new(false))? == *hash
                            {
                                keeper = true;
                                break;
                            }
                        }
                    }
                    if !keeper {
                        return Err(
                            "No verified unselected copy remains. Keep at least one current copy."
                                .into(),
                        );
                    }
                    if hash_file(&path, &AtomicBool::new(false))? != *hash {
                        return Err("File contents changed. Run the duplicate check again.".into());
                    }
                }
                check_identity(&record, &path)?;
                trash::delete(&path).map_err(|e| format!("Could not move to system trash: {e}"))?;
                db::connect(&self.0.db)?
                    .execute("DELETE FROM files WHERE id=?", [id])
                    .map_err(|e| e.to_string())?;
                Ok(())
            })();
            match action {
                Ok(()) => result.trashed.push(*id),
                Err(reason) => result.failed.push(TrashFailure { id: *id, reason }),
            }
        }
        Ok(result)
    }
}
pub(crate) fn flush(
    conn: &mut rusqlite::Connection,
    batch: &[FileRecord],
    generation: i64,
) -> Result<()> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    {
        let mut stmt=tx.prepare_cached("INSERT INTO files(root_id,path,name,extension,category,size,modified,created,source,screenshot,seen,modified_token) VALUES(?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(path) DO UPDATE SET name=excluded.name,extension=excluded.extension,category=excluded.category,size=excluded.size,modified=excluded.modified,modified_token=excluded.modified_token,created=excluded.created,source=excluded.source,screenshot=excluded.screenshot,seen=excluded.seen,hash=CASE WHEN files.size=excluded.size AND files.modified_token=excluded.modified_token THEN files.hash ELSE NULL END").map_err(|e|e.to_string())?;
        for f in batch {
            stmt.execute(params![
                f.root_id,
                f.path,
                f.name,
                f.extension,
                f.category,
                f.size,
                f.modified,
                f.created,
                f.source,
                f.screenshot,
                generation,
                f.modified_token
            ])
            .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}
// Roots are stored in canonical form at selection time. Refuse a subsequently
// redirected root (including a replaced ancestor) instead of trusting its new target.
pub(crate) fn registered_root(root: &Path) -> Result<PathBuf> {
    let current = fs::canonicalize(root).map_err(|e| e.to_string())?;
    if current != root || !current.is_dir() {
        return Err("Selected folder moved or was redirected. Remove it and add it again.".into());
    }
    Ok(current)
}
pub fn validate_path(path: &Path, root: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|e| e.to_string())?;
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    if !canonical.starts_with(&root) || !canonical.is_file() {
        return Err("File is outside the selected folder or no longer exists.".into());
    }
    let link = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if link.file_type().is_symlink() {
        return Err("Symbolic links cannot be used for file actions.".into());
    }
    Ok(canonical)
}
pub fn check_identity(record: &FileRecord, path: &Path) -> Result<()> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let token = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|t| t.as_nanos().to_string())
        .unwrap_or_default();
    if record.modified_token.is_empty()
        || meta.len() != record.size as u64
        || token != record.modified_token
    {
        return Err("File changed since the last scan or its identity is unavailable. Refresh your library before cleanup.".into());
    }
    Ok(())
}
pub fn hash_file(path: &Path, cancel: &AtomicBool) -> Result<String> {
    hash_file_controlled(path, cancel, || !cancel.load(Ordering::Relaxed))
}
fn hash_file_controlled(
    path: &Path,
    cancel: &AtomicBool,
    checkpoint: impl Fn() -> bool,
) -> Result<String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 256 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) || !checkpoint() {
            return Err("Cancelled".into());
        }
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
pub fn classify(extension: &str) -> &'static str {
    match extension {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "heic" | "avif"
        | "svg" | "ico" => "images",
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" | "wmv" => "videos",
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "opus" | "wma" => "music",
        "pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "odt" | "xlsx" | "xls" | "csv" | "ppt"
        | "pptx" | "epub" => "documents",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" => "archives",
        "exe" | "msi" | "msix" | "appx" | "dmg" | "pkg" | "deb" | "rpm" | "appimage" | "apk" => {
            "apps"
        }
        _ => "other",
    }
}
pub(crate) fn is_screenshot(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        "screenshot",
        "screen shot",
        "screen_shot",
        "screen-shot",
        "capture",
        "snip",
    ]
    .iter()
    .any(|s| lower.contains(s))
}
fn display_path(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn canonical_alias_cannot_count_as_a_duplicate_keeper() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        let path = root.join("Original.txt");
        fs::write(&path, b"only original").unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        engine.scan().unwrap();
        let mut conn = db::connect(&engine.0.db).unwrap();
        let original = db::query(&conn, Query::default()).unwrap().files.remove(0);
        let mut alias = original.clone();
        alias.path = Path::new(&original.path)
            .with_file_name("ORIGINAL.txt")
            .to_string_lossy()
            .into();
        alias.name = "ORIGINAL.txt".into();
        assert_eq!(
            fs::canonicalize(&alias.path).unwrap(),
            fs::canonicalize(&original.path).unwrap()
        );
        flush(&mut conn, &[alias], 0).unwrap();
        engine.find_duplicates().unwrap();
        let reviewed = db::file(&conn, original.id).unwrap();
        let result = engine
            .trash(vec![TrashSelection::from(&reviewed)], true)
            .unwrap();
        assert!(result.trashed.is_empty());
        assert_eq!(result.failed.len(), 1);
        assert!(path.exists());
    }
    #[test]
    fn scan_reconciles_and_cancellation_preserves_index() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("one.pdf"), b"first").unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        engine.scan().unwrap();
        assert_eq!(
            db::stats(&db::connect(&engine.0.db).unwrap())
                .unwrap()
                .count,
            1
        );
        fs::remove_file(root.join("one.pdf")).unwrap();
        engine.0.cancel.store(true, Ordering::Relaxed);
        engine.scan().unwrap();
        assert_eq!(
            db::stats(&db::connect(&engine.0.db).unwrap())
                .unwrap()
                .count,
            1
        );
        engine.0.cancel.store(false, Ordering::Relaxed);
        engine.scan().unwrap();
        assert_eq!(
            db::stats(&db::connect(&engine.0.db).unwrap())
                .unwrap()
                .count,
            0
        );
    }
    #[test]
    fn equal_size_does_not_imply_duplicate() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        for (name, bytes) in [("a.txt", "same"), ("b.txt", "same"), ("c.txt", "diff")] {
            fs::write(root.join(name), bytes).unwrap();
        }
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        engine.scan().unwrap();
        engine.find_duplicates().unwrap();
        let groups = db::duplicate_groups(&db::connect(&engine.0.db).unwrap()).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
    }
    #[test]
    fn path_escape_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("inside");
        fs::create_dir(&root).unwrap();
        let outside = temp.path().join("outside.txt");
        fs::write(&outside, b"no").unwrap();
        assert!(validate_path(&outside, &root).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn replaced_root_cannot_redirect_file_actions_or_scanning() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("selected");
        let outside = temp.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(root.join("same.txt"), b"inside").unwrap();
        fs::write(outside.join("same.txt"), b"outside").unwrap();
        let e = Engine::new(&temp.path().join("data")).unwrap();
        e.add_root(&root).unwrap();
        e.scan().unwrap();
        let conn = db::connect(&e.0.db).unwrap();
        let id: i64 = conn
            .query_row("SELECT id FROM files", [], |r| r.get(0))
            .unwrap();
        fs::rename(&root, temp.path().join("original")).unwrap();
        symlink(&outside, &root).unwrap();
        assert!(e.checked_file(id).is_err());
        e.scan().unwrap();
        assert_eq!(db::file(&conn, id).unwrap().size, 6);
        assert_eq!(e.job().errors, 1);
    }
    #[test]
    fn scan_excludes_its_own_database_and_cache() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("kept.txt"), b"keep").unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(temp.path()).unwrap();
        engine.scan().unwrap();
        assert_eq!(
            db::stats(&db::connect(&engine.0.db).unwrap())
                .unwrap()
                .count,
            1
        );
        assert!(engine.add_root(&temp.path().join("data")).is_err());
    }
    #[test]
    fn overlapping_roots_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("child")).unwrap();
        let e = Engine::new(&temp.path().join("data")).unwrap();
        e.add_root(&root).unwrap();
        assert!(e.add_root(&root.join("child")).is_err());
    }
    #[test]
    fn sql_search_handles_quotes_and_filters() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("report final.pdf"), b"report").unwrap();
        let e = Engine::new(&temp.path().join("data")).unwrap();
        e.add_root(&root).unwrap();
        e.scan().unwrap();
        let conn = db::connect(&e.0.db).unwrap();
        let result = db::query(
            &conn,
            Query {
                terms: vec!["report".into(), "ext:pdf".into()],
                limit: 100,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.total, 1);
        let result = db::query(
            &conn,
            Query {
                terms: vec!["\" OR 1=1 --".into()],
                limit: 100,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.total, 0);
    }

    #[test]
    fn selecting_every_duplicate_is_rejected_without_deleting() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        for name in ["a.txt", "b.txt"] {
            fs::write(root.join(name), b"keep me").unwrap();
        }
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        engine.scan().unwrap();
        engine.find_duplicates().unwrap();
        let groups = db::duplicate_groups(&db::connect(&engine.0.db).unwrap()).unwrap();
        let selected = groups[0].files.iter().map(TrashSelection::from).collect();
        let result = engine.trash(selected, true).unwrap();
        assert!(result.trashed.is_empty());
        assert_eq!(result.failed.len(), 2);
        assert!(root.join("a.txt").exists());
        assert!(root.join("b.txt").exists());
    }
    #[test]
    fn stale_cleanup_review_is_refused_even_after_a_rescan() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        let path = root.join("a.txt");
        fs::write(&path, b"original").unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        engine.scan().unwrap();
        let file = db::query(
            &db::connect(&engine.0.db).unwrap(),
            Query {
                limit: 1,
                ..Default::default()
            },
        )
        .unwrap()
        .files
        .remove(0);
        fs::write(&path, b"changed after the review").unwrap();
        engine.scan().unwrap();
        let result = engine
            .trash(vec![TrashSelection::from(&file)], false)
            .unwrap();
        assert!(result.trashed.is_empty());
        assert_eq!(result.failed.len(), 1);
        assert!(path.exists());
    }
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "explicit OS-trash integration test; only a generated disposable file is moved"]
    fn trash_integration_preserves_a_verified_keeper() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("disposable-fixtures");
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("digview-disposable-extra.txt"),
            b"generated DigView integration fixture",
        )
        .unwrap();
        fs::write(
            root.join("digview-disposable-keeper.txt"),
            b"generated DigView integration fixture",
        )
        .unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        engine.scan().unwrap();
        engine.find_duplicates().unwrap();
        let groups = db::duplicate_groups(&db::connect(&engine.0.db).unwrap()).unwrap();
        let extra = groups[0]
            .files
            .iter()
            .find(|f| f.name == "digview-disposable-extra.txt")
            .unwrap();
        let result = engine
            .trash(vec![TrashSelection::from(extra)], true)
            .unwrap();
        assert!(result.failed.is_empty());
        assert_eq!(result.trashed.len(), 1);
        assert!(!root.join("digview-disposable-extra.txt").exists());
        assert!(root.join("digview-disposable-keeper.txt").exists());
        assert_eq!(
            db::stats(&db::connect(&engine.0.db).unwrap())
                .unwrap()
                .count,
            1
        );
    }
    #[test]
    fn modification_within_same_second_invalidates_identity() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        let path = root.join("a.txt");
        fs::write(&path, b"before").unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        engine.scan().unwrap();
        let record = db::query(
            &db::connect(&engine.0.db).unwrap(),
            Query {
                limit: 1,
                ..Default::default()
            },
        )
        .unwrap()
        .files
        .remove(0);
        let old = fs::metadata(&path).unwrap().modified().unwrap();
        let handle = fs::OpenOptions::new().write(true).open(&path).unwrap();
        handle
            .set_times(
                fs::FileTimes::new().set_modified(old + std::time::Duration::from_nanos(100_000)),
            )
            .unwrap();
        assert!(check_identity(&record, &path).is_err());
    }
    #[test]
    fn cancelled_hash_does_not_return_a_partial_digest() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("file");
        fs::write(&path, b"contents").unwrap();
        assert!(hash_file(&path, &AtomicBool::new(true)).is_err());
    }
    #[test]
    fn controlled_hash_obeys_cancellation_between_chunks() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("file");
        fs::write(&path, vec![7u8; 1024 * 1024]).unwrap();
        let checkpoints = AtomicU64::new(0);
        assert!(hash_file_controlled(&path, &AtomicBool::new(false), || {
            checkpoints.fetch_add(1, Ordering::Relaxed) < 2
        })
        .is_err());
        assert_eq!(checkpoints.load(Ordering::Relaxed), 3);
        assert_eq!(
            hash_file_controlled(&path, &AtomicBool::new(false), || true).unwrap(),
            hash_file(&path, &AtomicBool::new(false)).unwrap()
        );
    }
    #[test]
    fn cached_totals_follow_scans_size_changes_deletions_and_root_removal() {
        let temp = tempfile::tempdir().unwrap();
        let files = temp.path().join("files");
        fs::create_dir(&files).unwrap();
        fs::write(files.join("a.txt"), b"ab").unwrap();
        fs::write(files.join("b.png"), b"xyz").unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        let root = engine.add_root(&files).unwrap();
        engine.scan().unwrap();
        let conn = db::connect(&engine.0.db).unwrap();
        let initial = db::stats(&conn).unwrap();
        assert_eq!((initial.count, initial.bytes), (2, 5));
        drop(conn);
        fs::write(files.join("a.txt"), b"1234567").unwrap();
        engine.scan().unwrap();
        let conn = db::connect(&engine.0.db).unwrap();
        let updated = db::stats(&conn).unwrap();
        assert_eq!((updated.count, updated.bytes), (2, 10));
        let documents = db::query(
            &conn,
            Query {
                category: Some("documents".into()),
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((documents.total, documents.bytes), (1, 7));
        drop(conn);
        fs::remove_file(files.join("b.png")).unwrap();
        engine.scan().unwrap();
        let conn = db::connect(&engine.0.db).unwrap();
        let remaining = db::query(
            &conn,
            Query {
                root_id: Some(root.id),
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((remaining.total, remaining.bytes), (1, 7));
        drop(conn);
        engine.remove_root(root.id).unwrap();
        let empty = db::stats(&db::connect(&engine.0.db).unwrap()).unwrap();
        assert_eq!((empty.count, empty.bytes), (0, 0));
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn source_metadata_changes_update_source_totals_without_duplicate_counts() {
        let temp = tempfile::tempdir().unwrap();
        let files = temp.path().join("files");
        fs::create_dir(&files).unwrap();
        let file = files.join("download.pdf");
        fs::write(&file, b"demo").unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&files).unwrap();
        fs::write(
            format!("{}:Zone.Identifier", file.display()),
            b"[ZoneTransfer]\nHostUrl=https://www.github.com/example/project\n",
        )
        .unwrap();
        engine.scan().unwrap();
        let first = db::stats(&db::connect(&engine.0.db).unwrap()).unwrap();
        assert_eq!(first.sources[0].source, "github.com");
        assert_eq!(first.count, 1);
        fs::write(
            format!("{}:Zone.Identifier", file.display()),
            b"[ZoneTransfer]\nHostUrl=https://drive.google.com/example\n",
        )
        .unwrap();
        engine.scan().unwrap();
        let conn = db::connect(&engine.0.db).unwrap();
        let second = db::stats(&conn).unwrap();
        assert_eq!(second.sources.len(), 1);
        assert_eq!(second.sources[0].source, "drive.google.com");
        assert_eq!(second.count, 1);
        let page = db::query(
            &conn,
            Query {
                source: Some("drive.google.com".into()),
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((page.total, page.bytes), (1, 4));
    }
    #[test]
    #[ignore = "explicit performance measurement, not part of unit tests"]
    fn benchmark_index() {
        let temp = tempfile::tempdir().unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        let root = engine.add_root(&root).unwrap();
        let mut conn = db::connect(&engine.0.db).unwrap();
        let insert_start = Instant::now();
        for batch_start in (0..100_000).step_by(256) {
            let batch = (batch_start..(batch_start + 256).min(100_000))
                .map(|i| FileRecord {
                    id: 0,
                    root_id: root.id,
                    path: format!("{}/report-{i}.pdf", root.path),
                    name: format!("report-{i}.pdf"),
                    extension: "pdf".into(),
                    category: if i % 3 == 0 {
                        "images".into()
                    } else {
                        "documents".into()
                    },
                    size: (i as i64 + 1) * 1024,
                    modified: db::now() - i as i64 * 60,
                    created: None,
                    source: if i % 4 == 0 {
                        Some("example.com".into())
                    } else {
                        None
                    },
                    screenshot: false,
                    hash: None,
                    modified_token: format!("{i}"),
                })
                .collect::<Vec<_>>();
            flush(&mut conn, &batch, 1).unwrap();
        }
        let insert_ms = insert_start.elapsed().as_secs_f64() * 1000.;
        let mut results = Vec::new();
        for label in [
            "recent", "category", "filename", "size", "name", "root", "source",
        ] {
            let mut timings = Vec::new();
            for _ in 0..25 {
                let q = Query {
                    limit: 120,
                    sort: if label == "size" || label == "name" {
                        label.into()
                    } else {
                        "recent".into()
                    },
                    category: if label == "category" {
                        Some("images".into())
                    } else {
                        None
                    },
                    terms: if label == "filename" {
                        vec!["report-999".into()]
                    } else {
                        vec![]
                    },
                    root_id: if label == "root" { Some(root.id) } else { None },
                    source: if label == "source" {
                        Some("example.com".into())
                    } else {
                        None
                    },
                    ..Default::default()
                };
                let start = Instant::now();
                let page = db::query(&conn, q).unwrap();
                assert!(page.files.len() <= 120);
                assert!(page.total > 0);
                if label == "filename" {
                    assert_eq!(page.total, 111);
                }
                timings.push(start.elapsed().as_secs_f64() * 1000.);
            }
            timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
            results.push(
                serde_json::json!({"query":label,"median_ms":timings[12],"p95_ms":timings[23]}),
            );
        }
        println!(
            "BENCH_RESULT {}",
            serde_json::json!({"records":100000,"profile":"debug","cache":"warm SQLite, temporary disk database","insert_ms":insert_ms,"queries":results})
        );
    }
}
