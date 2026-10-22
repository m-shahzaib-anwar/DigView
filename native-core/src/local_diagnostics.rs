//! Bounded local diagnostic records. No arguments, file paths, account values or raw errors.
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

static LOGGER: OnceLock<Arc<Diagnostics>> = OnceLock::new();
const MAX_BYTES: u64 = 1024 * 1024;
const LOGS: [&str; 4] = [
    "diagnostics.3.jsonl",
    "diagnostics.2.jsonl",
    "diagnostics.1.jsonl",
    "diagnostics.jsonl",
];
pub struct Diagnostics {
    directory: PathBuf,
    gate: Mutex<()>,
    healthy: AtomicBool,
}
#[derive(Deserialize, Serialize, Default)]
pub struct Entry {
    pub event: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub error_name: Option<String>,
    #[serde(default)]
    pub frames: Vec<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub processed: Option<u64>,
    #[serde(default)]
    pub errors: Option<u64>,
    #[serde(default)]
    pub cancelled: Option<bool>,
}
pub fn error_code(error: &str) -> &'static str {
    let error = error.to_ascii_lowercase();
    if error.contains("changed") || error.contains("identity") {
        "file_changed"
    } else if error.contains("permission") || error.contains("access is denied") {
        "permission_denied"
    } else if error.contains("not found")
        || error.contains("no longer")
        || error.contains("os error 2")
    {
        "not_found"
    } else if error.contains("cancel") {
        "cancelled"
    } else if error.contains("unsupported") || error.contains("unavailable for this file type") {
        "unsupported"
    } else if error.contains("expired") || error.contains("preview closed") {
        "preview_expired"
    } else {
        "unknown"
    }
}
fn safe_frame(frame: &str) -> bool {
    let pieces: Vec<_> = frame.split(':').collect();
    pieces.len() == 3
        && pieces[0].len() <= 100
        && pieces[0]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        && [".js", ".mjs", ".ts", ".tsx"]
            .iter()
            .any(|ext| pieces[0].ends_with(ext))
        && pieces[1..].iter().all(|piece| {
            !piece.is_empty() && piece.len() <= 10 && piece.bytes().all(|c| c.is_ascii_digit())
        })
}
fn sanitize(mut entry: Entry) -> Option<Entry> {
    if ![
        "app_started",
        "frontend_error",
        "react_render_failed",
        "native_action",
        "native_failed",
        "job_finished",
        "window_unload",
        "preview_failed",
    ]
    .contains(&entry.event.as_str())
    {
        return None;
    }
    entry.action = entry.action.filter(|s| {
        [
            "bootstrap",
            "job_status",
            "choose_folder",
            "remove_root",
            "start_scan",
            "start_duplicates",
            "cancel_job",
            "query_files",
            "duplicate_groups",
            "get_thumbnail",
            "open_file",
            "reveal_file",
            "recognize_text",
            "trash_files",
            "personal_library",
            "set_favourite",
            "save_search",
            "remove_search",
            "open_preview",
            "close_preview",
            "ocr_identity",
            "save_image_text",
            "clear_image_text",
            "get_image_text",
            "scan_status",
            "set_scan_preferences",
            "background_settings",
            "set_background_settings",
            "update_status",
            "check_update",
            "install_update",
            "scan",
            "duplicates",
            "watch",
        ]
        .contains(&s.as_str())
    });
    entry.code = entry.code.filter(|s| {
        [
            "unhandled_error",
            "unhandled_rejection",
            "render_failed",
            "request_failed",
            "load_failed",
            "decode_failed",
            "worker_failed",
            "cancelled",
            "unsupported",
            "file_changed",
            "not_found",
            "permission_denied",
            "preview_expired",
            "unknown",
        ]
        .contains(&s.as_str())
    });
    entry.error_name = entry.error_name.filter(|s| {
        [
            "Error",
            "TypeError",
            "RangeError",
            "SyntaxError",
            "ReferenceError",
            "URIError",
            "EvalError",
            "DOMException",
            "NativeError",
        ]
        .contains(&s.as_str())
    });
    entry.frames = entry
        .frames
        .into_iter()
        .filter(|s| safe_frame(s))
        .take(12)
        .collect();
    entry.duration_ms = entry.duration_ms.map(|v| v.min(86400000));
    entry.processed = entry.processed.map(|v| v.min(1_000_000_000));
    entry.errors = entry.errors.map(|v| v.min(1_000_000_000));
    Some(entry)
}
fn milliseconds() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
impl Diagnostics {
    pub fn initialize(data: &Path, version: &str) -> Arc<Self> {
        let directory = data.join("logs");
        let healthy = fs::create_dir_all(&directory).is_ok();
        let logger = Arc::new(Self {
            directory,
            gate: Mutex::new(()),
            healthy: AtomicBool::new(healthy),
        });
        let previous_unclean = logger.directory.join("session-open").exists();
        if fs::write(logger.directory.join("session-open"), b"open").is_err() {
            logger.healthy.store(false, Ordering::Relaxed);
        }
        let _ = LOGGER.set(logger.clone());
        logger.write(json!({"event":"native_started", "version":version, "platform":std::env::consts::OS, "architecture":std::env::consts::ARCH, "previous_session_unclean":previous_unclean}));
        let prior = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if let Some(logger) = LOGGER.get() {
                let location = info.location().map(|loc| {
                    format!(
                        "{}:{}",
                        loc.file().rsplit(['/', '\\']).next().unwrap_or("unknown"),
                        loc.line()
                    )
                });
                // Panic text/backtrace can contain user paths and contents. Only source location is recorded.
                if let Ok(_guard) = logger.gate.try_lock() {
                    logger.write_locked(json!({"event":"native_panic", "location":location}));
                }
            }
            prior(info);
        }));
        logger
    }
    fn write(&self, value: serde_json::Value) {
        let Ok(_gate) = self.gate.lock() else {
            return;
        };
        self.write_locked(value);
    }
    fn write_locked(&self, value: serde_json::Value) {
        let result = (|| -> std::io::Result<()> {
            let current = self.directory.join("diagnostics.jsonl");
            if fs::metadata(&current)
                .map(|m| m.len() >= MAX_BYTES)
                .unwrap_or(false)
            {
                // Only the four known generated diagnostic files are rotated.
                let oldest = self.directory.join(LOGS[0]);
                if oldest.exists() {
                    fs::remove_file(oldest)?;
                }
                for index in 1..LOGS.len() {
                    let from = self.directory.join(LOGS[index]);
                    if from.exists() {
                        fs::rename(from, self.directory.join(LOGS[index - 1]))?;
                    }
                }
            }
            let record = json!({"timestamp_ms": milliseconds(), "record": value});
            let mut output = OpenOptions::new().create(true).append(true).open(current)?;
            serde_json::to_writer(&mut output, &record)?;
            output.write_all(b"\n")?;
            output.flush()
        })();
        if result.is_err() {
            self.healthy.store(false, Ordering::Relaxed);
        }
    }
    pub fn record(&self, entries: Vec<Entry>) {
        for entry in entries.into_iter().take(30).filter_map(sanitize) {
            self.write(serde_json::to_value(entry).unwrap_or_default());
        }
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }
    pub fn export(&self, destination: &Path) -> Result<(), String> {
        let _gate = self.gate.lock().map_err(|e| e.to_string())?;
        let parent = destination
            .parent()
            .ok_or("Invalid export location")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if parent.starts_with(self.directory.canonicalize().map_err(|e| e.to_string())?) {
            return Err("Choose a location outside the live logs folder.".into());
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|_| "Choose a new file name for the diagnostic export.")?;
        for filename in LOGS {
            let path = self.directory.join(filename);
            if path.exists() {
                let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                if !metadata.is_file()
                    || metadata.file_type().is_symlink()
                    || metadata.len() > MAX_BYTES + 16 * 1024
                {
                    return Err("A diagnostic log is invalid or too large to export.".into());
                }
                let mut input = fs::File::open(path).map_err(|e| e.to_string())?;
                std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
            }
        }
        output.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn clean_shutdown(&self) {
        self.write(json!({"event":"native_exit"}));
        let _ = fs::remove_file(self.directory.join("session-open"));
    }
}
pub fn background_failure(error: &str) {
    if let Some(logger) = LOGGER.get() {
        logger.write(json!({"event":"native_background_failed", "code":error_code(error)}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Diagnostics) {
        let temporary =
            tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        let directory = temporary.path().join("logs");
        fs::create_dir(&directory).unwrap();
        (
            temporary,
            Diagnostics {
                directory,
                gate: Mutex::new(()),
                healthy: AtomicBool::new(true),
            },
        )
    }
    #[test]
    fn diagnostic_records_cannot_include_raw_errors_paths_or_credentials() {
        let (_temp, logger) = fixture();
        let entry: Entry = serde_json::from_value(json!({"event":"frontend_error", "action":"query_files", "code":"unhandled_error", "error_name":"person@example.invalid secret-token", "frames":["C:\\Users\\Private\\file.txt:2:30", "https://private.invalid/file.js:2:30", "index-abc.js:2:30"], "raw_error":"secret-token", "email":"person@example.invalid"})).unwrap();
        logger.record(vec![entry]);
        let text = fs::read_to_string(logger.directory.join("diagnostics.jsonl")).unwrap();
        assert!(text.contains("index-abc.js:2:30"));
        assert!(text.contains("query_files"));
        for secret in [
            "secret-token",
            "person@",
            "Private",
            "https://private",
            "raw_error",
        ] {
            assert!(!text.contains(secret));
        }
        assert!(sanitize(Entry {
            event: "arbitrary private text".into(),
            ..Default::default()
        })
        .is_none());
    }
    #[test]
    fn rotation_and_exports_are_bounded_and_do_not_overwrite_other_files() {
        let (temp, logger) = fixture();
        for sequence in 0..1200 {
            logger.write(json!({"event":"generated_fixture", "sequence": sequence, "padding":"x".repeat(4000)}));
        }
        let files: Vec<_> = fs::read_dir(&logger.directory).unwrap().collect();
        assert_eq!(files.len(), 4);
        for name in LOGS {
            assert!(fs::metadata(logger.directory.join(name)).unwrap().len() <= MAX_BYTES + 8192);
        }
        fs::write(logger.directory.join("session-open"), "not exported").unwrap();
        let destination = temp.path().join("export.jsonl");
        logger.export(&destination).unwrap();
        let text = fs::read_to_string(&destination).unwrap();
        assert!(!text.contains("not exported"));
        for line in text.lines() {
            assert!(serde_json::from_str::<serde_json::Value>(line).is_ok());
        }
        assert!(logger.export(&destination).is_err());
        assert_eq!(fs::read_to_string(&destination).unwrap(), text);
        assert!(logger
            .export(&logger.directory.join("custom.jsonl"))
            .is_err());
        logger.clean_shutdown();
        assert!(!logger.directory.join("session-open").exists());
    }
    #[test]
    fn unavailable_log_storage_does_not_panic_or_break_file_actions() {
        let (_temp, mut logger) = fixture();
        logger.directory = logger.directory.join("missing/subfolder");
        logger.record(vec![Entry {
            event: "app_started".into(),
            ..Default::default()
        }]);
        assert!(!logger.healthy());
    }
}
