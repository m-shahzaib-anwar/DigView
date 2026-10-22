//! Selected-root document FTS indexing with a cancellable isolated parser.
use crate::{
    db::{self, Result},
    document_extract::{self, Extracted},
    engine::{check_identity, hash_file, Engine},
    model::{map_file, FileRecord, FILE_COLUMNS},
};
use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

pub fn init(c: &Connection) -> Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS document_settings(id INTEGER PRIMARY KEY CHECK(id=1),enabled INTEGER NOT NULL);
      INSERT OR IGNORE INTO document_settings VALUES(1,0);
      CREATE TABLE IF NOT EXISTS document_text(file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
        size INTEGER NOT NULL,modified_token TEXT NOT NULL,digest TEXT NOT NULL,text TEXT NOT NULL,status TEXT NOT NULL,version INTEGER NOT NULL);
      CREATE VIRTUAL TABLE IF NOT EXISTS document_text_fts USING fts5(text,content='document_text',content_rowid='file_id',tokenize='unicode61');
      CREATE TRIGGER IF NOT EXISTS document_text_ai AFTER INSERT ON document_text BEGIN INSERT INTO document_text_fts(rowid,text) VALUES(new.file_id,new.text); END;
      CREATE TRIGGER IF NOT EXISTS document_text_ad AFTER DELETE ON document_text BEGIN INSERT INTO document_text_fts(document_text_fts,rowid,text) VALUES('delete',old.file_id,old.text); END;
      CREATE TRIGGER IF NOT EXISTS document_text_au AFTER UPDATE ON document_text BEGIN
        INSERT INTO document_text_fts(document_text_fts,rowid,text) VALUES('delete',old.file_id,old.text);
        INSERT INTO document_text_fts(rowid,text) VALUES(new.file_id,new.text); END;
      CREATE TRIGGER IF NOT EXISTS document_text_invalidate AFTER UPDATE OF size,modified_token,path ON files
        WHEN old.size!=new.size OR old.modified_token!=new.modified_token OR old.path!=new.path BEGIN DELETE FROM document_text WHERE file_id=new.id; END;").map_err(|e|e.to_string())
}
pub fn enabled(c: &Connection) -> Result<bool> {
    c.query_row(
        "SELECT enabled FROM document_settings WHERE id=1",
        [],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}
fn supported_sql() -> String {
    let extensions = document_extract::TEXT_EXTENSIONS
        .iter()
        .copied()
        .chain(["pdf", "docx"])
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(",");
    let names = document_extract::TEXT_NAMES
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(",");
    format!("(v.extension IN ({extensions}) OR (v.extension='' AND lower(v.name) IN ({names})))")
}
#[derive(Serialize)]
pub struct Status {
    pub enabled: bool,
    pub indexed: i64,
    pub partial: i64,
    pub empty: i64,
    pub failed: i64,
    pub pending: i64,
}
impl Engine {
    pub fn document_index_status(&self) -> Result<Status> {
        let c = db::connect(&self.0.db)?;
        let (indexed,partial,empty,failed)=c.query_row("SELECT coalesce(sum(d.status='indexed'),0),coalesce(sum(d.status='partial'),0),coalesce(sum(d.status='empty'),0),coalesce(sum(d.status='failed'),0) FROM document_text d JOIN visible_files v ON v.id=d.file_id",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|e|e.to_string())?;
        let pending=c.query_row(&format!("SELECT count(*) FROM visible_files v LEFT JOIN document_text d ON d.file_id=v.id WHERE {} AND (d.file_id IS NULL OR d.version<>1)",supported_sql()),[],|r|r.get(0)).map_err(|e|e.to_string())?;
        Ok(Status {
            enabled: enabled(&c)?,
            indexed,
            partial,
            empty,
            failed,
            pending,
        })
    }
    pub fn set_document_index_enabled(&self, value: bool) -> Result<()> {
        let _gate = self.0.operation_gate.lock().map_err(|e| e.to_string())?;
        if value && self.job().running {
            return Err("Finish or cancel the current library task first.".into());
        }
        db::connect(&self.0.db)?
            .execute("UPDATE document_settings SET enabled=? WHERE id=1", [value])
            .map_err(|e| e.to_string())?;
        self.0.document_enabled.store(value, Ordering::Relaxed);
        if !value && self.job().kind == "content" {
            self.0.cancel.store(true, Ordering::Relaxed);
        }
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    pub fn clear_document_index(&self) -> Result<()> {
        let _gate = self.0.operation_gate.lock().map_err(|e| e.to_string())?;
        if self.job().running {
            return Err("Finish or cancel the current library task first.".into());
        }
        let mut c = db::connect(&self.0.db)?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        tx.execute("UPDATE document_settings SET enabled=0 WHERE id=1", [])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM document_text", [])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        self.0.document_enabled.store(false, Ordering::Relaxed);
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    pub fn start_document_index(&self, retry: bool) -> Result<()> {
        if !self.0.document_enabled.load(Ordering::Relaxed) {
            return Err("Enable document content indexing first.".into());
        }
        self.begin("content")?;
        let e = self.clone();
        std::thread::spawn(move || {
            let started = Instant::now();
            let result = e.index_documents(retry, |kind, bytes| {
                run_parser(kind, bytes, || {
                    e.0.cancel.load(Ordering::Relaxed)
                        || !e.0.document_enabled.load(Ordering::Relaxed)
                })
            });
            e.finish(started, result);
        });
        Ok(())
    }
    pub(crate) fn maybe_start_document_index(&self) {
        if self.0.document_enabled.load(Ordering::Relaxed) && !self.job().running {
            if self.document_index_status().is_ok_and(|s| s.pending > 0) {
                let _ = self.start_document_index(false);
            }
        }
    }
    fn index_documents(
        &self,
        retry: bool,
        mut parser: impl FnMut(u8, Vec<u8>) -> Result<Extracted>,
    ) -> Result<()> {
        if retry {
            db::connect(&self.0.db)?
                .execute(
                    "DELETE FROM document_text WHERE status IN ('failed','empty')",
                    [],
                )
                .map_err(|e| e.to_string())?;
        }
        let columns = FILE_COLUMNS
            .split(',')
            .map(|s| format!("v.{s}"))
            .collect::<Vec<_>>()
            .join(",");
        let sql=format!("SELECT {columns} FROM visible_files v LEFT JOIN document_text d ON d.file_id=v.id WHERE {} AND (d.file_id IS NULL OR d.version<>1) ORDER BY v.id LIMIT 128",supported_sql());
        let mut processed = 0u64;
        loop {
            if !self.0.document_enabled.load(Ordering::Relaxed) || !self.checkpoint() {
                break;
            }
            let c = db::connect(&self.0.db)?;
            let rows = c
                .prepare(&sql)
                .map_err(|e| e.to_string())?
                .query_map([], map_file)
                .map_err(|e| e.to_string())?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?;
            if rows.is_empty() {
                break;
            }
            for record in rows {
                if !self.0.document_enabled.load(Ordering::Relaxed) || !self.checkpoint() {
                    return Ok(());
                }
                let result = self.extract_document(&record, &mut parser);
                let _gate = self.0.operation_gate.lock().map_err(|e| e.to_string())?;
                if self.0.cancel.load(Ordering::Relaxed)
                    || !self.0.document_enabled.load(Ordering::Relaxed)
                {
                    return Ok(());
                }
                let (text, status, digest) = match result {
                    Ok((result, digest)) => {
                        let status = if result.truncated {
                            "partial"
                        } else if result.text.trim().is_empty() {
                            "empty"
                        } else {
                            "indexed"
                        };
                        (result.text, status, digest)
                    }
                    Err(_) => {
                        self.update_document_job(processed, true);
                        (String::new(), "failed", String::new())
                    }
                };
                // Reconciliation may have replaced this ID while the parser ran.
                c.execute("INSERT INTO document_text(file_id,size,modified_token,digest,text,status,version)
                    SELECT id,size,modified_token,?,?,?,1 FROM files WHERE id=? AND path=? AND size=? AND modified_token=? AND id IN (SELECT id FROM visible_files)
                    ON CONFLICT(file_id) DO UPDATE SET size=excluded.size,modified_token=excluded.modified_token,digest=excluded.digest,text=excluded.text,status=excluded.status,version=excluded.version",
                    params![digest,text,status,record.id,record.path,record.size,record.modified_token]).map_err(|e|e.to_string())?;
                processed += 1;
                self.update_document_job(processed, false);
                self.0.revision.fetch_add(1, Ordering::Relaxed);
            }
        }
        Ok(())
    }
    fn update_document_job(&self, processed: u64, error: bool) {
        if let Ok(mut job) = self.0.job.lock() {
            job.processed = processed;
            if error {
                job.errors += 1;
            }
            job.message = format!("Indexing document contents · {processed} files processed");
        }
    }
    fn extract_document(
        &self,
        record: &FileRecord,
        parser: &mut impl FnMut(u8, Vec<u8>) -> Result<Extracted>,
    ) -> Result<(Extracted, String)> {
        let (current, path) = self.checked_file(record.id)?;
        if current.path != record.path
            || current.modified_token != record.modified_token
            || current.size != record.size
        {
            return Err("changed".into());
        }
        check_identity(record, &path)?;
        let kind = document_extract::kind(&record.extension, &record.name).ok_or("unsupported")?;
        if record.size > document_extract::INPUT_LIMIT as i64 {
            return Err("too_large".into());
        }
        let limit = document_extract::INPUT_LIMIT;
        let mut file = File::open(&path).map_err(|_| "unavailable")?;
        let mut bytes = vec![];
        loop {
            if !self.checkpoint() {
                return Err("cancelled".into());
            }
            let mut chunk = [0u8; 65536];
            let read = file.read(&mut chunk).map_err(|_| "unavailable")?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..read]);
            if bytes.len() > limit {
                return Err("too_large".into());
            }
        }
        check_identity(record, &path)?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        if kind == b't' {
            bytes.truncate(document_extract::TEXT_READ_LIMIT + 1);
        }
        let mut result = parser(kind, bytes)?;
        result.truncated |= kind == b't' && record.size > document_extract::TEXT_READ_LIMIT as i64;
        if result.text.len() > document_extract::OUTPUT_LIMIT {
            return Err("invalid_worker_reply".into());
        }
        let (latest, latest_path) = self.checked_file(record.id)?;
        if latest_path != path
            || latest.modified_token != record.modified_token
            || latest.size != record.size
        {
            return Err("changed".into());
        }
        check_identity(record, &path)?;
        if hash_file(&path, &self.0.cancel)? != digest {
            return Err("changed".into());
        }
        Ok((result, digest))
    }
}

/// Bytes-only protocol; the worker entry point opens no paths or library DB.
pub fn run_parser(kind: u8, bytes: Vec<u8>, cancel: impl Fn() -> bool) -> Result<Extracted> {
    run_parser_at(
        &std::env::current_exe().map_err(|_| "worker_unavailable")?,
        kind,
        bytes,
        cancel,
        Duration::from_secs(30),
    )
}
fn run_parser_at(
    executable: &Path,
    kind: u8,
    bytes: Vec<u8>,
    cancel: impl Fn() -> bool,
    timeout: Duration,
) -> Result<Extracted> {
    if bytes.len() > document_extract::INPUT_LIMIT {
        return Err("too_large".into());
    }
    let mut command = Command::new(executable);
    command
        .args(["--digview-content-worker", &(kind as char).to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|_| "worker_unavailable")?;
    #[cfg(windows)]
    let _memory = match memory_limit(&child) {
        Ok(job) => job,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
    };
    let mut input = child.stdin.take().ok_or("worker_unavailable")?;
    let output = child.stdout.take().ok_or("worker_unavailable")?;
    let writer = std::thread::spawn(move || input.write_all(&bytes));
    let reader = std::thread::spawn(move || {
        let mut bytes = vec![];
        let _ = output
            .take((document_extract::OUTPUT_LIMIT * 6 + 4096) as u64)
            .read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if cancel() || started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = writer.join();
            let _ = reader.join();
            return Err("cancelled_or_timeout".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = writer.join();
                let _ = reader.join();
                return Err("worker_failed".into());
            }
        }
    };
    writer
        .join()
        .map_err(|_| "worker_failed")?
        .map_err(|_| "worker_failed")?;
    let bytes = reader.join().map_err(|_| "worker_failed")?;
    if !status.success() {
        return Err("extraction_failed".into());
    }
    let result: std::result::Result<Extracted, String> =
        serde_json::from_slice(&bytes).map_err(|_| "invalid_worker_reply")?;
    let result = result.map_err(|_| "extraction_failed")?;
    if result.text.len() > document_extract::OUTPUT_LIMIT {
        return Err("invalid_worker_reply".into());
    }
    Ok(result)
}

#[cfg(windows)]
struct MemoryJob(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Drop for MemoryJob {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(windows)]
fn memory_limit(child: &std::process::Child) -> Result<MemoryJob> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{Foundation::HANDLE, System::JobObjects::*};
    unsafe {
        let job = MemoryJob(CreateJobObjectW(None, None).map_err(|_| "worker_limit_unavailable")?);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_PROCESS_MEMORY | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        limits.ProcessMemoryLimit = 512 * 1024 * 1024;
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
        .map_err(|_| "worker_limit_unavailable")?;
        AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle()))
            .map_err(|_| "worker_limit_unavailable")?;
        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Query;
    use std::{fs, sync::atomic::AtomicUsize};
    fn fixture() -> (tempfile::TempDir, Engine, std::path::PathBuf) {
        let t = tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        let root = t.path().join("selected");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("notes.txt"), b"sapphiretextneedle").unwrap();
        fs::write(root.join("sample.rs"), b"fn emeraldcodeneedle() {} ").unwrap();
        fs::write(root.join("report.pdf"), document_extract::tests::pdf()).unwrap();
        fs::write(root.join("letter.docx"),document_extract::tests::docx("<w:document xmlns:w='urn:test'><w:p><w:r><w:t>amethystdocxneedle</w:t></w:r></w:p></w:document>")).unwrap();
        let e = Engine::new(&t.path().join("data")).unwrap();
        e.add_root(&root).unwrap();
        e.scan_mode(false).unwrap();
        e.set_document_index_enabled(true).unwrap();
        (t, e, root)
    }
    fn matches(e: &Engine, term: &str) -> i64 {
        db::query(
            &db::connect(&e.0.db).unwrap(),
            Query {
                terms: vec![term.into()],
                limit: 200,
                ..Default::default()
            },
        )
        .unwrap()
        .total
    }
    fn index(e: &Engine, retry: bool) {
        e.index_documents(retry, |kind, bytes| document_extract::extract(kind, &bytes))
            .unwrap();
    }
    #[test]
    fn all_formats_are_searchable_incremental_hidden_filterable_and_clear_preserves_originals() {
        let (t, e, root) = fixture();
        index(&e, false);
        for needle in [
            "sapphiretextneedle",
            "emeraldcodeneedle",
            "quartzpdfneedle",
            "amethystdocxneedle",
        ] {
            assert_eq!(matches(&e, needle), 1);
        }
        assert_eq!(e.document_index_status().unwrap().indexed, 4);
        e.index_documents(false, |_, _| panic!("unchanged documents must not reparse"))
            .unwrap();
        let c = db::connect(&e.0.db).unwrap();
        let id = db::query(
            &c,
            Query {
                terms: vec!["notes".into()],
                ..Default::default()
            },
        )
        .unwrap()
        .files[0]
            .id;
        e.favourite(id, true).unwrap();
        assert_eq!(
            db::query(
                &c,
                Query {
                    terms: vec!["sapphiretextneedle".into()],
                    favourites: true,
                    ..Default::default()
                }
            )
            .unwrap()
            .total,
            1
        );
        e.hide_files(vec![id], false).unwrap();
        assert_eq!(matches(&e, "sapphiretextneedle"), 0);
        e.unhide(e.hidden_items().unwrap()[0].id).unwrap();
        assert_eq!(matches(&e, "sapphiretextneedle"), 1);
        fs::write(root.join("notes.txt"), b"changedcontentneedle").unwrap();
        e.scan_mode(false).unwrap();
        assert_eq!(matches(&e, "sapphiretextneedle"), 0);
        index(&e, false);
        assert_eq!(matches(&e, "changedcontentneedle"), 1);
        let reopened = Engine::new(&t.path().join("data")).unwrap();
        assert_eq!(matches(&reopened, "changedcontentneedle"), 1);
        fs::remove_file(root.join("letter.docx")).unwrap();
        e.scan_mode(false).unwrap();
        assert_eq!(matches(&e, "amethystdocxneedle"), 0);
        e.clear_document_index().unwrap();
        assert_eq!(matches(&e, "changedcontentneedle"), 0);
        assert!(!e.document_index_status().unwrap().enabled);
        assert!(root.join("notes.txt").is_file());
        assert_eq!(e.personal_library().unwrap().favourite_ids, vec![id]);
    }
    #[test]
    fn cancellation_does_not_publish_partial_results_and_failures_retry_explicitly() {
        let (_t, e, _root) = fixture();
        e.index_documents(false, |kind, bytes| {
            e.0.cancel.store(true, Ordering::Relaxed);
            document_extract::extract(kind, &bytes)
        })
        .unwrap();
        assert_eq!(e.document_index_status().unwrap().indexed, 0);
        e.0.cancel.store(false, Ordering::Relaxed);
        e.index_documents(false, |_, _| Err("fixture parser failure".into()))
            .unwrap();
        assert_eq!(e.document_index_status().unwrap().failed, 4);
        e.index_documents(false, |_, _| {
            panic!("failed inputs wait for explicit retry")
        })
        .unwrap();
        index(&e, true);
        assert_eq!(e.document_index_status().unwrap().indexed, 4);
    }
    #[test]
    fn preserved_metadata_changes_cannot_publish_stale_document_text() {
        let (_t, e, root) = fixture();
        let c = db::connect(&e.0.db).unwrap();
        let record = db::query(
            &c,
            Query {
                terms: vec!["notes".into()],
                ..Default::default()
            },
        )
        .unwrap()
        .files
        .remove(0);
        let path = root.join("notes.txt");
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let result = e.extract_document(&record, &mut |kind, bytes| {
            let result = document_extract::extract(kind, &bytes);
            fs::write(&path, vec![b'x'; record.size as usize]).unwrap();
            File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(modified))
                .unwrap();
            check_identity(&record, &fs::canonicalize(&path).unwrap()).unwrap();
            result
        });
        assert!(result.is_err());
        assert_eq!(matches(&e, "sapphiretextneedle"), 0);
    }
    #[test]
    fn hidden_excluded_and_removed_roots_do_not_gain_new_content() {
        let (_t, e, root) = fixture();
        let c = db::connect(&e.0.db).unwrap();
        let file = db::query(
            &c,
            Query {
                terms: vec!["notes".into()],
                ..Default::default()
            },
        )
        .unwrap()
        .files
        .remove(0);
        e.hide_files(vec![file.id], false).unwrap();
        index(&e, false);
        assert_eq!(e.document_index_status().unwrap().indexed, 3);
        let mut prefs = e.0.scan_preferences.lock().unwrap().clone();
        prefs.exclusions.push("*.rs".into());
        e.save_scan_preferences(prefs).unwrap();
        e.scan_mode(false).unwrap();
        assert_eq!(matches(&e, "emeraldcodeneedle"), 0);
        let outside = root.parent().unwrap().join("not-selected.txt");
        fs::write(&outside, b"privateoutsideneedle").unwrap();
        index(&e, false);
        assert_eq!(matches(&e, "privateoutsideneedle"), 0);
        e.remove_root(file.root_id).unwrap();
        assert_eq!(e.document_index_status().unwrap().indexed, 0);
        assert_eq!(
            c.query_row("SELECT count(*) FROM document_text", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    #[ignore = "explicit isolated parser protocol fixture; build examples/content-worker first"]
    fn isolated_worker_protocol_timeout_and_cancellation() {
        let executable = std::env::var_os("DIGVIEW_TEST_CONTENT_WORKER")
            .expect("set DIGVIEW_TEST_CONTENT_WORKER to generated fixture helper");
        let executable = Path::new(&executable);
        let result = run_parser_at(
            executable,
            b'p',
            document_extract::tests::pdf(),
            || false,
            Duration::from_secs(30),
        )
        .unwrap();
        assert!(result.text.contains("quartzpdfneedle"));
        let result = run_parser_at(
            executable,
            b'd',
            document_extract::tests::docx("<document><p><t>amethystdocxneedle</t></p></document>"),
            || false,
            Duration::from_secs(30),
        )
        .unwrap();
        assert!(result.text.contains("amethystdocxneedle"));
        let (_temp, engine, _root) = fixture();
        engine
            .index_documents(false, |kind, bytes| {
                run_parser_at(executable, kind, bytes, || false, Duration::from_secs(30))
            })
            .unwrap();
        for needle in [
            "sapphiretextneedle",
            "emeraldcodeneedle",
            "quartzpdfneedle",
            "amethystdocxneedle",
        ] {
            assert_eq!(matches(&engine, needle), 1);
        }
        assert!(run_parser_at(
            executable,
            b'p',
            b"not a pdf".to_vec(),
            || false,
            Duration::from_secs(30)
        )
        .is_err());
        let checks = AtomicUsize::new(0);
        assert!(run_parser_at(
            executable,
            b't',
            b"fixture".to_vec(),
            || {
                checks.fetch_add(1, Ordering::Relaxed);
                true
            },
            Duration::from_secs(30)
        )
        .is_err());
        assert!(checks.load(Ordering::Relaxed) > 0);
        assert!(run_parser_at(
            executable,
            b't',
            b"fixture".to_vec(),
            || false,
            Duration::ZERO
        )
        .is_err());
    }
}
