//! Native notifications are hints, never authorization to index another folder.
use crate::{
    db::{self, Result},
    engine::{self, Engine},
    model::{FileRecord, Root},
};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use rusqlite::params;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const QUIET: Duration = Duration::from_secs(2);
const CAPACITY: usize = 4096;

pub struct WatchService {
    stop: Arc<AtomicBool>,
}
impl Drop for WatchService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Signature {
    size: u64,
    modified: SystemTime,
}
fn signature(path: &Path) -> Option<Signature> {
    let meta = fs::metadata(path).ok()?;
    Some(Signature {
        size: meta.len(),
        modified: meta.modified().ok()?,
    })
}
struct Pending {
    since: Instant,
    signature: Option<Signature>,
}
impl Pending {
    fn new(path: &Path) -> Self {
        Self {
            since: Instant::now(),
            signature: signature(path),
        }
    }
    fn ready(&mut self, path: &Path) -> bool {
        if self.since.elapsed() < QUIET {
            return false;
        }
        let current = signature(path);
        if self.signature != current {
            self.signature = current;
            self.since = Instant::now();
            return false;
        }
        self.since.elapsed() >= QUIET
    }
}

impl Engine {
    /// One service per app; worker holds only a Weak engine between ticks.
    pub fn watch_selected(&self) -> WatchService {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let weak = Arc::downgrade(&self.0);
        std::thread::spawn(move || {
            let (tx, rx) = mpsc::sync_channel::<notify::Result<Event>>(CAPACITY);
            let overflow = Arc::new(AtomicBool::new(false));
            let full = overflow.clone();
            let mut watcher = match notify::recommended_watcher(move |event| {
                if tx.try_send(event).is_err() {
                    full.store(true, Ordering::Relaxed);
                }
            }) {
                Ok(w) => w,
                Err(error) => {
                    if let Some(inner) = weak.upgrade() {
                        *inner.watch_error.lock().unwrap_or_else(|e| e.into_inner()) =
                            format!("Automatic updates unavailable: {error}. Use Refresh.");
                    }
                    return;
                }
            };
            let mut watched = HashSet::<PathBuf>::new();
            let mut pending = HashMap::<PathBuf, Pending>::new();
            let mut recovery: Option<Instant> = None;
            let mut roots = Vec::<Root>::new();
            let mut cancellation_seen = false;
            let mut policy_revision = 0u64;
            let mut paused_until = Instant::now();
            let mut sync_at = Instant::now() - Duration::from_secs(5);
            let mut content_at = Instant::now();
            while !stopping.load(Ordering::Relaxed) {
                // Bound both callback memory and work per tick during event storms.
                std::thread::sleep(Duration::from_millis(200));
                if stopping.load(Ordering::Relaxed) {
                    break;
                }
                let Some(inner) = weak.upgrade() else { break };
                let engine = Engine(inner);
                let next_policy = engine.0.policy_revision.load(Ordering::Relaxed);
                if next_policy != policy_revision {
                    policy_revision = next_policy;
                    recovery = Some(Instant::now());
                }
                if sync_at.elapsed() >= Duration::from_secs(1) {
                    sync_at = Instant::now();
                    roots = match db::connect(&engine.0.db).and_then(|c| db::roots(&c)) {
                        Ok(roots) => roots,
                        Err(error) => {
                            engine.watch_error(error);
                            continue;
                        }
                    };
                    let desired: HashSet<PathBuf> =
                        roots.iter().map(|r| PathBuf::from(&r.path)).collect();
                    for path in watched.clone() {
                        if !desired.contains(&path) || engine::registered_root(&path).is_err() {
                            let _ = watcher.unwatch(&path);
                            watched.remove(&path);
                        }
                    }
                    let mut errors = vec![];
                    for path in &desired {
                        if !watched.contains(path) {
                            let result = engine::registered_root(path).and_then(|_| {
                                watcher
                                    .watch(path, RecursiveMode::Recursive)
                                    .map_err(|e| e.to_string())
                            });
                            match result {
                                Ok(()) => {
                                    watched.insert(path.clone());
                                    recovery = Some(Instant::now());
                                }
                                Err(error) => errors.push(error),
                            }
                        }
                    }
                    engine.watch_error(if errors.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "Some folders cannot be watched: {}. Use Refresh or re-add the folder.",
                            errors.join("; ")
                        )
                    });
                    pending.retain(|p, _| roots.iter().any(|r| p.starts_with(&r.path)));
                }
                for _ in 0..CAPACITY {
                    match rx.try_recv() {
                        Ok(Ok(event)) => {
                            if matches!(event.kind, EventKind::Access(_)) {
                                continue;
                            }
                            if event.need_rescan() {
                                recovery = Some(Instant::now());
                            }
                            for path in event.paths {
                                if !roots.iter().any(|r| contained(&engine, &path, r)) {
                                    // Windows case-only renames can leave the old spelling
                                    // resolvable. Reconcile the old index row, not an alias.
                                    if roots.iter().any(|r| {
                                        allowed(&engine, &path, r)
                                            && fs::canonicalize(&path).is_ok_and(|actual| {
                                                actual != path && actual.starts_with(&r.path)
                                            })
                                    }) {
                                        recovery = Some(Instant::now());
                                    }
                                    continue;
                                }
                                if pending.len() >= CAPACITY && !pending.contains_key(&path) {
                                    recovery = Some(Instant::now());
                                } else {
                                    pending.insert(path.clone(), Pending::new(&path));
                                }
                            }
                        }
                        Ok(Err(error)) => {
                            engine.watch_error(format!(
                                "Watcher missed changes: {error}. Recovering index."
                            ));
                            recovery = Some(Instant::now());
                        }
                        Err(_) => break,
                    }
                }
                if overflow.swap(false, Ordering::Relaxed) {
                    recovery = Some(Instant::now());
                }
                let job = engine.job();
                if job.cancelled && !cancellation_seen {
                    paused_until = Instant::now() + Duration::from_secs(30);
                }
                cancellation_seen = job.cancelled;
                if job.running || engine.suspended() || Instant::now() < paused_until {
                    continue;
                }
                pending.retain(|path, item| {
                    item.since.elapsed() < QUIET
                        || roots.iter().any(|r| contained(&engine, path, r))
                });
                let ready: Vec<PathBuf> = pending
                    .iter_mut()
                    .filter_map(|(p, item)| item.ready(p).then(|| p.clone()))
                    .take(256)
                    .collect();
                for path in ready {
                    if !engine.checkpoint() {
                        break;
                    }
                    match engine.reconcile_path(&path) {
                        Ok(Change::Done) => {
                            pending.remove(&path);
                        }
                        Ok(Change::Tree) => {
                            pending.remove(&path);
                            recovery = Some(Instant::now());
                        }
                        Err(_) => {
                            pending.remove(&path);
                            recovery = Some(Instant::now());
                        }
                    }
                }
                if recovery.is_some_and(|t| t.elapsed() >= QUIET) && engine.begin("watch").is_ok() {
                    let started = Instant::now();
                    let result = engine.scan_mode(true);
                    // Preserve missed-event work on cancellation; retry after a pause.
                    recovery = match &result {
                        Ok(false) if !engine.0.cancel.load(Ordering::Relaxed) => None,
                        _ if engine.0.cancel.load(Ordering::Relaxed) => {
                            Some(Instant::now() + Duration::from_secs(28))
                        }
                        _ => Some(Instant::now()),
                    };
                    engine.finish(started, result.map(|_| ()));
                }
                if pending.is_empty()
                    && recovery.is_none()
                    && content_at.elapsed() >= Duration::from_secs(5)
                {
                    content_at = Instant::now();
                    engine.maybe_start_document_index();
                }
            }
        });
        WatchService { stop }
    }

    fn watch_error(&self, message: String) {
        *self.0.watch_error.lock().unwrap_or_else(|e| e.into_inner()) = message;
    }

    fn reconcile_path(&self, path: &Path) -> Result<Change> {
        let _guard = self.0.operation_gate.try_lock().map_err(|_| "Busy")?;
        if self.job().running || self.suspended() {
            return Err("Busy".into());
        }
        let mut conn = db::connect(&self.0.db)?;
        let roots = db::roots(&conn)?;
        let Some(root) = roots.iter().find(|r| allowed(self, path, r)) else {
            return Ok(Change::Done);
        };
        let root_path = engine::registered_root(Path::new(&root.path))?;
        // Check the nearest existing ancestor too: removed paths have no canonical form.
        let mut ancestor = path;
        loop {
            match fs::symlink_metadata(ancestor) {
                Ok(meta) => {
                    if meta.file_type().is_symlink() {
                        return Ok(Change::Done);
                    }
                    let canonical = fs::canonicalize(ancestor).map_err(|e| e.to_string())?;
                    if canonical != ancestor || !canonical.starts_with(&root_path) {
                        return Ok(Change::Done);
                    }
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    ancestor = ancestor.parent().ok_or("No selected ancestor")?;
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        let changed = match fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let prefix = format!("{}{}", path.to_string_lossy(), std::path::MAIN_SEPARATOR);
                conn.execute(
                    "DELETE FROM files WHERE root_id=? AND (path=? OR substr(path,1,length(?))=?)",
                    params![root.id, path.to_string_lossy(), prefix, prefix],
                )
                .map_err(|e| e.to_string())?
                    > 0
            }
            Err(e) => return Err(e.to_string()),
            Ok(meta) if meta.is_dir() => return Ok(Change::Tree),
            Ok(meta) if meta.is_file() => {
                let canonical = engine::validate_path(path, &root_path)?;
                let extension = path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if temporary(&extension) {
                    return Ok(Change::Done);
                }
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let modified = meta
                    .modified()
                    .map_err(|e| e.to_string())?
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| e.to_string())?;
                let record = FileRecord {
                    id: 0,
                    root_id: root.id,
                    path: path.to_string_lossy().into(),
                    name: name.clone(),
                    extension: extension.clone(),
                    category: engine::classify(&extension).into(),
                    size: meta.len().min(i64::MAX as u64) as i64,
                    modified: modified.as_secs() as i64,
                    modified_token: modified.as_nanos().to_string(),
                    created: meta
                        .created()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64),
                    source: crate::platform::origin(&canonical),
                    screenshot: engine::classify(&extension) == "images"
                        && engine::is_screenshot(&name),
                    hash: None,
                };
                engine::check_identity(&record, &canonical)?;
                engine::flush(&mut conn, &[record], 0)?;
                true
            }
            _ => false,
        };
        if changed {
            self.0.revision.fetch_add(1, Ordering::Relaxed);
        }
        Ok(Change::Done)
    }
}

enum Change {
    Done,
    Tree,
}
fn temporary(extension: &str) -> bool {
    matches!(extension, "crdownload" | "part" | "partial" | "tmp")
}
fn allowed(engine: &Engine, path: &Path, root: &Root) -> bool {
    let root = Path::new(&root.path);
    if !path.is_absolute()
        || !path.starts_with(root)
        || path.starts_with(&engine.0.app_data)
        || engine.excluded(path, root)
    {
        return false;
    }
    !path
        .strip_prefix(root)
        .unwrap()
        .components()
        .any(|part| match part {
            Component::ParentDir => true,
            Component::Normal(name) => cfg!(windows) && name.to_string_lossy().contains(':'),
            _ => false,
        })
}

fn contained(engine: &Engine, path: &Path, root: &Root) -> bool {
    if !allowed(engine, path, root) || engine::registered_root(Path::new(&root.path)).is_err() {
        return false;
    }
    let mut ancestor = path;
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) => {
                return !meta.file_type().is_symlink()
                    && fs::canonicalize(ancestor).is_ok_and(|canonical| {
                        canonical == ancestor && canonical.starts_with(&root.path)
                    })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(parent) = ancestor.parent() else {
                    return false;
                };
                ancestor = parent;
            }
            Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn count(engine: &Engine) -> i64 {
        db::stats(&db::connect(&engine.0.db).unwrap())
            .unwrap()
            .count
    }
    fn fixture() -> (tempfile::TempDir, Engine, PathBuf) {
        let temp =
            tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        let root = temp.path().join("selected");
        fs::create_dir(&root).unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        (temp, engine, root)
    }
    #[test]
    fn watcher_excludes_canonical_application_data_under_a_selected_parent() {
        let temp =
            tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        let root = temp.path().join("parent");
        fs::create_dir(&root).unwrap();
        let engine = Engine::new(&root.join("data")).unwrap();
        engine.add_root(&root).unwrap();
        engine
            .reconcile_path(&fs::canonicalize(&engine.0.db).unwrap())
            .unwrap();
        assert_eq!(count(&engine), 0);
        let ordinary = root.join("ordinary.txt");
        fs::write(&ordinary, b"index me").unwrap();
        engine
            .reconcile_path(&fs::canonicalize(ordinary).unwrap())
            .unwrap();
        assert_eq!(count(&engine), 1);
    }
    #[test]
    fn default_exclusions_match_full_scan_and_incremental_updates() {
        let (_temp, engine, root) = fixture();
        let mut excluded = vec![];
        for rule in crate::scan_policy::DEFAULT_EXCLUSIONS {
            let path = if let Some(ext) = rule.strip_prefix("*.") {
                root.join(format!("download.{ext}"))
            } else if matches!(*rule, ".DS_Store" | "Thumbs.db" | "desktop.ini") {
                root.join(rule)
            } else {
                let directory = root.join(rule);
                fs::create_dir_all(&directory).unwrap();
                directory.join("generated.txt")
            };
            fs::write(&path, b"excluded").unwrap();
            excluded.push(path);
        }
        // These broad names can contain useful deliverables and must stay visible.
        for name in ["build", "dist", "bin", "vendor"] {
            fs::create_dir(root.join(name)).unwrap();
            fs::write(root.join(name).join("keep.txt"), b"keep").unwrap();
        }
        engine.scan_mode(false).unwrap();
        assert_eq!(count(&engine), 4);
        for path in excluded {
            fs::write(&path, b"changed").unwrap();
            engine.reconcile_path(&path).unwrap();
            assert!(path.is_file());
        }
        assert_eq!(count(&engine), 4);
        assert!(engine
            .scan_status()
            .unwrap()
            .default_exclusions
            .contains(&"node_modules".to_string()));
        #[cfg(windows)]
        assert!(engine.excluded(&root.join("NODE_MODULES/child.txt"), &root));
    }
    #[test]
    fn settling_restarts_when_download_grows() {
        let (_temp, _engine, root) = fixture();
        let path = root.join("download.pdf");
        fs::write(&path, b"partial").unwrap();
        let mut pending = Pending::new(&path);
        assert!(!pending.ready(&path));
        pending.since = Instant::now() - QUIET;
        fs::write(&path, b"finished download").unwrap();
        assert!(!pending.ready(&path));
        pending.since = Instant::now() - QUIET;
        assert!(pending.ready(&path));
    }
    #[test]
    fn incremental_create_change_rename_remove_and_boundaries() {
        let (temp, engine, root) = fixture();
        let path = root.join("one.txt");
        fs::write(&path, b"one").unwrap();
        engine.reconcile_path(&path).unwrap();
        assert_eq!(count(&engine), 1);
        let conn = db::connect(&engine.0.db).unwrap();
        conn.execute("UPDATE files SET hash='old-proof'", [])
            .unwrap();
        fs::write(&path, b"changed").unwrap();
        engine.reconcile_path(&path).unwrap();
        let (size, hash): (i64, Option<String>) = conn
            .query_row("SELECT size,hash FROM files", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(size, 7);
        assert_eq!(hash, None);
        let renamed = root.join("two.txt");
        fs::rename(&path, &renamed).unwrap();
        engine.reconcile_path(&path).unwrap();
        engine.reconcile_path(&renamed).unwrap();
        assert_eq!(count(&engine), 1);
        let outside = temp.path().join("outside.txt");
        fs::write(&outside, b"private").unwrap();
        engine.reconcile_path(&outside).unwrap();
        let partial = root.join("browser.crdownload");
        fs::write(&partial, b"downloading").unwrap();
        engine.reconcile_path(&partial).unwrap();
        assert_eq!(count(&engine), 1);
        engine.begin("duplicates").unwrap();
        fs::remove_file(&renamed).unwrap();
        assert!(engine.reconcile_path(&renamed).is_err());
        assert_eq!(count(&engine), 1);
        engine.finish(Instant::now(), Ok(()));
        engine.reconcile_path(&renamed).unwrap();
        assert_eq!(count(&engine), 0);
        let root_id = db::roots(&conn).unwrap()[0].id;
        engine.remove_root(root_id).unwrap();
        engine.reconcile_path(&partial).unwrap();
        assert_eq!(count(&engine), 0);
    }
    #[test]
    fn deleted_directory_removes_only_its_own_index_rows() {
        let (_temp, engine, root) = fixture();
        for name in ["folder", "folder-other"] {
            fs::create_dir(root.join(name)).unwrap();
            let path = root.join(name).join("file.txt");
            fs::write(&path, b"keep").unwrap();
            engine.reconcile_path(&path).unwrap();
        }
        // Delete only generated fixture files, never personal folders.
        fs::remove_file(root.join("folder/file.txt")).unwrap();
        fs::remove_dir(root.join("folder")).unwrap();
        engine.reconcile_path(&root.join("folder")).unwrap();
        assert_eq!(count(&engine), 1);
    }
    #[test]
    fn recovery_defers_recent_files_and_preserves_old_rows() {
        let (_temp, engine, root) = fixture();
        let path = root.join("growing.pdf");
        fs::write(&path, b"old").unwrap();
        engine.reconcile_path(&path).unwrap();
        fs::write(&path, b"still growing").unwrap();
        assert!(engine.scan_mode(true).unwrap());
        assert_eq!(count(&engine), 1);
        let size: i64 = db::connect(&engine.0.db)
            .unwrap()
            .query_row("SELECT size FROM files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(size, 3);
        engine.0.cancel.store(true, Ordering::Relaxed);
        fs::remove_file(path).unwrap();
        engine.scan_mode(true).unwrap();
        assert_eq!(count(&engine), 1);
    }
    #[cfg(windows)]
    #[test]
    fn cancelled_manual_job_does_not_immediately_restart_as_recovery() {
        let (_temp, engine, root) = fixture();
        engine.begin("scan").unwrap();
        let service = engine.watch_selected();
        fs::write(root.join("pending.txt"), b"wait").unwrap();
        std::thread::sleep(Duration::from_secs(1));
        engine.0.cancel.store(true, Ordering::Relaxed);
        engine.finish(Instant::now(), Ok(()));
        std::thread::sleep(Duration::from_secs(3));
        assert_eq!(engine.job().kind, "scan");
        assert!(engine.job().cancelled);
        assert_eq!(count(&engine), 0);
        drop(service);
    }
    #[cfg(windows)]
    #[test]
    fn native_reconnect_and_custom_exclusions_recover_while_paused() {
        fn wait_until(mut condition: impl FnMut() -> bool) {
            let deadline = Instant::now() + Duration::from_secs(20);
            while !condition() {
                assert!(Instant::now() < deadline, "Reconnect recovery timed out");
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        let (temp, engine, root) = fixture();
        engine
            .save_scan_preferences(crate::scan_policy::Preferences {
                paused: true,
                exclusions: vec!["*.log".into()],
                ..Default::default()
            })
            .unwrap();
        fs::write(root.join("keep.txt"), b"keep").unwrap();
        fs::write(root.join("skip.log"), b"skip").unwrap();
        let service = engine.watch_selected();
        std::thread::sleep(Duration::from_secs(3));
        assert_eq!(count(&engine), 0);
        engine
            .save_scan_preferences(crate::scan_policy::Preferences {
                exclusions: vec!["*.log".into()],
                ..Default::default()
            })
            .unwrap();
        wait_until(|| count(&engine) == 1 && !engine.job().running);
        let detached = temp.path().join("detached");
        fs::rename(&root, &detached).unwrap();
        std::thread::sleep(Duration::from_secs(2));
        assert!(!engine.scan_status().unwrap().folders[0].available);
        assert_eq!(count(&engine), 1);
        fs::write(detached.join("new.txt"), b"new").unwrap();
        fs::rename(&detached, &root).unwrap();
        wait_until(|| count(&engine) == 2 && !engine.job().running);
        assert!(root.join("skip.log").is_file());
        drop(service);
    }
    #[cfg(windows)]
    #[test]
    fn windows_native_notifications_update_index_without_manual_scan() {
        fn wait_until(mut condition: impl FnMut() -> bool) {
            let deadline = Instant::now() + Duration::from_secs(20);
            while !condition() {
                assert!(
                    Instant::now() < deadline,
                    "Native watcher failed to reconcile within 20 seconds"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        let (_temp, engine, root) = fixture();
        // Existing file proves startup reconciliation, not just a live create event.
        fs::write(root.join("existing.txt"), b"existing").unwrap();
        let service = engine.watch_selected();
        wait_until(|| count(&engine) == 1 && !engine.job().running);
        fs::rename(root.join("existing.txt"), root.join("EXISTING.txt")).unwrap();
        wait_until(|| {
            count(&engine) == 1
                && db::connect(&engine.0.db)
                    .unwrap()
                    .query_row("SELECT name FROM files", [], |r| r.get::<_, String>(0))
                    .unwrap()
                    == "EXISTING.txt"
        });
        let partial = root.join("download.crdownload");
        fs::write(&partial, b"partial").unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(count(&engine), 1);
        let final_path = root.join("download.pdf");
        fs::rename(&partial, &final_path).unwrap();
        wait_until(|| count(&engine) == 2);
        fs::remove_file(&final_path).unwrap();
        wait_until(|| count(&engine) == 1);
        let id = db::roots(&db::connect(&engine.0.db).unwrap()).unwrap()[0].id;
        engine.remove_root(id).unwrap();
        fs::write(root.join("not-selected-anymore.txt"), b"private").unwrap();
        std::thread::sleep(Duration::from_secs(3));
        assert_eq!(count(&engine), 0);
        drop(service);
    }
}
