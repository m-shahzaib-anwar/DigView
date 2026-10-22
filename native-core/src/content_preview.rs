//! Short-lived, ID-scoped local previews. No renderer-provided filesystem paths.
use crate::{
    db::Result,
    engine::{check_identity, Engine},
    model::FileRecord,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant, UNIX_EPOCH},
};

pub const TEXT_LIMIT: usize = 256 * 1024;
pub const PDF_LIMIT: u64 = 40 * 1024 * 1024;
pub const CHUNK_LIMIT: u64 = 2 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(3600);

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Pdf,
    Video,
    Audio,
    Text,
}

#[derive(Serialize, Debug)]
pub struct PreviewInfo {
    pub kind: Kind,
    pub mime: &'static str,
    pub token: Option<String>,
    pub text: Option<String>,
    pub truncated: bool,
    pub encoding: Option<&'static str>,
}
#[derive(Clone)]
struct Grant {
    record: FileRecord,
    path: PathBuf,
    kind: Kind,
    mime: &'static str,
    touched: Instant,
}
#[derive(Default)]
pub struct PreviewStore {
    grants: Mutex<HashMap<String, Grant>>,
    io_gate: Mutex<()>,
}
pub struct PreviewBytes {
    pub status: u16,
    pub mime: &'static str,
    pub total: u64,
    pub start: u64,
    pub length: u64,
    pub bytes: Vec<u8>,
}

pub fn format(extension: &str, name: &str) -> Option<(Kind, &'static str)> {
    use Kind::*;
    Some(match extension.to_ascii_lowercase().as_str() {
        "pdf" => (Pdf, "application/pdf"),
        "mp4" | "m4v" => (Video, "video/mp4"),
        "mov" => (Video, "video/quicktime"),
        "webm" => (Video, "video/webm"),
        "mkv" => (Video, "video/x-matroska"),
        "avi" => (Video, "video/x-msvideo"),
        "wmv" => (Video, "video/x-ms-wmv"),
        "mp3" => (Audio, "audio/mpeg"),
        "wav" => (Audio, "audio/wav"),
        "flac" => (Audio, "audio/flac"),
        "aac" => (Audio, "audio/aac"),
        "m4a" => (Audio, "audio/mp4"),
        "ogg" | "oga" | "opus" => (Audio, "audio/ogg"),
        "wma" => (Audio, "audio/x-ms-wma"),
        "txt" | "text" | "md" | "markdown" | "log" | "csv" | "tsv" | "json" | "jsonl" | "xml"
        | "yaml" | "yml" | "toml" | "ini" | "cfg" | "conf" | "html" | "htm" | "css" | "scss"
        | "js" | "jsx" | "ts" | "tsx" | "rs" | "py" | "java" | "c" | "h" | "cpp" | "hpp" | "cs"
        | "go" | "sql" | "sh" | "ps1" | "bat" | "cmd" | "srt" | "vtt" => {
            (Text, "text/plain; charset=utf-8")
        }
        "" if matches!(
            name.to_ascii_lowercase().as_str(),
            "readme" | "license" | "licence" | "notice" | "changelog" | "makefile" | "dockerfile"
        ) =>
        {
            (Text, "text/plain; charset=utf-8")
        }
        _ => return None,
    })
}

fn check_handle(record: &FileRecord, file: &File) -> Result<()> {
    let meta = file.metadata().map_err(|e| e.to_string())?;
    let token = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|t| t.as_nanos().to_string())
        .unwrap_or_default();
    if !meta.is_file()
        || meta.len() != record.size as u64
        || token.is_empty()
        || token != record.modified_token
    {
        return Err("File changed. Refresh the library and reopen the preview.".into());
    }
    Ok(())
}
fn check_location(engine: &Engine, record: &FileRecord, path: &Path) -> Result<()> {
    let (current, current_path) = engine.checked_file(record.id)?;
    if current_path != path
        || current.path != record.path
        || current.size != record.size
        || current.modified_token != record.modified_token
    {
        return Err("Preview file changed or is no longer in the selected library.".into());
    }
    check_identity(record, path)
}
fn revalidate(engine: &Engine, record: &FileRecord, path: &Path, file: &File) -> Result<()> {
    check_location(engine, record, path)?;
    check_handle(record, file)
}

pub(crate) fn decode_text(bytes: &[u8], truncated: bool) -> Result<(String, &'static str)> {
    let (text, encoding) = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        let mut units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| {
                if little {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        if truncated && units.last().is_some_and(|v| (0xd800..=0xdbff).contains(v)) {
            units.pop();
        }
        (
            String::from_utf16(&units)
                .map_err(|_| "This file's text encoding is unsupported. Open the original.")?,
            if little { "UTF-16 LE" } else { "UTF-16 BE" },
        )
    } else {
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        let valid = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) if truncated && error.error_len().is_none() => std::str::from_utf8(&bytes[..error.valid_up_to()]).map_err(|e| e.to_string())?,
            Err(_) => return Err("Text preview supports UTF-8 and UTF-16 with a byte-order mark. Open this file in its original app.".into()),
        };
        (valid.to_owned(), "UTF-8")
    };
    if text
        .chars()
        .any(|c| c == '\0' || (c.is_control() && !matches!(c, '\n' | '\r' | '\t' | '\u{c}')))
    {
        return Err("This file contains binary data and cannot be shown as text.".into());
    }
    Ok((text, encoding))
}

impl PreviewStore {
    pub fn open(&self, engine: &Engine, id: i64) -> Result<PreviewInfo> {
        let _io = self.io_gate.lock().map_err(|e| e.to_string())?;
        let (record, path) = engine.checked_file(id)?;
        check_identity(&record, &path)?;
        let (kind, mime) = format(&record.extension, &record.name)
            .ok_or("Inline preview is unavailable for this file type. Open the original.")?;
        if kind == Kind::Pdf && record.size as u64 > PDF_LIMIT {
            return Err(
                "This PDF exceeds the 40 MB inline preview limit. Open the original.".into(),
            );
        }
        let mut file = File::open(&path).map_err(|e| e.to_string())?;
        check_handle(&record, &file)?;
        if kind == Kind::Text {
            let mut bytes = Vec::new();
            (&mut file)
                .take(TEXT_LIMIT as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            revalidate(engine, &record, &path, &file)?;
            let truncated = record.size as u64 > TEXT_LIMIT as u64;
            let (text, encoding) = decode_text(&bytes, truncated)?;
            return Ok(PreviewInfo {
                kind,
                mime,
                token: None,
                text: Some(text),
                truncated,
                encoding: Some(encoding),
            });
        }
        if kind == Kind::Pdf {
            let mut header = Vec::new();
            (&mut file)
                .take(1024)
                .read_to_end(&mut header)
                .map_err(|e| e.to_string())?;
            if !header.windows(5).any(|w| w == b"%PDF-") {
                return Err(
                    "This file does not have a valid PDF header. Open the original.".into(),
                );
            }
        }
        revalidate(engine, &record, &path, &file)?;
        let mut entropy = [0u8; 32];
        getrandom::fill(&mut entropy).map_err(|_| "Unable to create a private preview session.")?;
        let token: String = entropy.iter().map(|b| format!("{b:02x}")).collect();
        let mut grants = self.grants.lock().map_err(|e| e.to_string())?;
        grants.retain(|_, grant| grant.touched.elapsed() < TTL);
        if grants.len() >= 8 {
            return Err("Too many previews are open. Close a preview and try again.".into());
        }
        grants.insert(
            token.clone(),
            Grant {
                record,
                path,
                kind,
                mime,
                touched: Instant::now(),
            },
        );
        Ok(PreviewInfo {
            kind,
            mime,
            token: Some(token),
            text: None,
            truncated: false,
            encoding: None,
        })
    }
    pub fn close(&self, token: &str) -> Result<()> {
        self.grants.lock().map_err(|e| e.to_string())?.remove(token);
        Ok(())
    }
    pub fn read(
        &self,
        engine: &Engine,
        token: &str,
        range: Option<&str>,
        head: bool,
    ) -> Result<PreviewBytes> {
        let _io = self.io_gate.lock().map_err(|e| e.to_string())?;
        let grant = {
            let mut grants = self.grants.lock().map_err(|e| e.to_string())?;
            grants.retain(|_, grant| grant.touched.elapsed() < TTL);
            let grant = grants
                .get_mut(token)
                .ok_or("Preview closed or expired. Reopen it.")?;
            grant.touched = Instant::now();
            grant.clone()
        };
        check_location(engine, &grant.record, &grant.path)?;
        let mut file = File::open(&grant.path).map_err(|e| e.to_string())?;
        revalidate(engine, &grant.record, &grant.path, &file)?;
        let total = grant.record.size as u64;
        let window = if head && range.is_none() {
            Ok((0, total, false))
        } else {
            byte_window(
                range,
                total,
                if grant.kind == Kind::Pdf && range.is_none() {
                    PDF_LIMIT
                } else {
                    CHUNK_LIMIT
                },
            )
        };
        let Ok((start, length, partial)) = window else {
            return Ok(PreviewBytes {
                status: 416,
                mime: grant.mime,
                total,
                start: 0,
                length: 0,
                bytes: vec![],
            });
        };
        let mut bytes = Vec::new();
        if !head {
            file.seek(SeekFrom::Start(start))
                .map_err(|e| e.to_string())?;
            (&mut file)
                .take(length)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 != length {
                return Err("File changed during preview.".into());
            }
        }
        revalidate(engine, &grant.record, &grant.path, &file)?;
        // A close during a disk read must not deliver the old session afterwards.
        if !self
            .grants
            .lock()
            .map_err(|e| e.to_string())?
            .contains_key(token)
        {
            return Err("Preview closed.".into());
        }
        Ok(PreviewBytes {
            status: if partial { 206 } else { 200 },
            mime: grant.mime,
            total,
            start,
            length,
            bytes,
        })
    }
}

fn byte_window(range: Option<&str>, size: u64, limit: u64) -> Result<(u64, u64, bool)> {
    if let Some(range) = range {
        let value = range.strip_prefix("bytes=").ok_or("Invalid range")?;
        let (start, end) = value.split_once('-').ok_or("Invalid range")?;
        if size == 0 || value.contains(',') {
            return Err("Unsatisfiable range".into());
        }
        let (start, end) = if start.is_empty() {
            let suffix: u64 = end.parse().map_err(|_| "Invalid suffix range")?;
            if suffix == 0 {
                return Err("Invalid suffix range".into());
            }
            (size.saturating_sub(suffix), size - 1)
        } else {
            let start: u64 = start.parse().map_err(|_| "Invalid range start")?;
            let end = if end.is_empty() {
                size - 1
            } else {
                end.parse::<u64>()
                    .map_err(|_| "Invalid range end")?
                    .min(size - 1)
            };
            (start, end)
        };
        if start >= size || end < start {
            return Err("Unsatisfiable range".into());
        }
        Ok((start, (end - start + 1).min(limit), true))
    } else {
        Ok((0, size.min(limit), size > limit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, model::Query};
    use std::fs;

    fn fixture(name: &str, bytes: &[u8]) -> (tempfile::TempDir, Engine, PathBuf, i64) {
        let temporary =
            tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        let root = temporary.path().join("selected");
        fs::create_dir(&root).unwrap();
        fs::write(root.join(name), bytes).unwrap();
        let engine = Engine::new(&temporary.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        engine.scan_mode(false).unwrap();
        let id = db::query(&db::connect(&engine.0.db).unwrap(), Query::default())
            .unwrap()
            .files[0]
            .id;
        (temporary, engine, root, id)
    }
    #[test]
    fn media_seek_ranges_are_bounded_and_reassembled_without_whole_file_reads() {
        let bytes: Vec<u8> = (0..(CHUNK_LIMIT as usize + 13))
            .map(|i| (i % 251) as u8)
            .collect();
        let (_temp, engine, _root, id) = fixture("generated.mp4", &bytes);
        let store = PreviewStore::default();
        let info = store.open(&engine, id).unwrap();
        let token = info.token.unwrap();
        let first = store.read(&engine, &token, None, false).unwrap();
        assert_eq!(first.status, 206);
        assert_eq!(first.bytes.len() as u64, CHUNK_LIMIT);
        assert_eq!(first.total as usize, bytes.len());
        let last = store
            .read(
                &engine,
                &token,
                Some(&format!("bytes={CHUNK_LIMIT}-")),
                false,
            )
            .unwrap();
        assert_eq!(last.bytes, bytes[CHUNK_LIMIT as usize..]);
        let tail = store
            .read(&engine, &token, Some("bytes=-5"), false)
            .unwrap();
        assert_eq!(tail.bytes, bytes[bytes.len() - 5..]);
        let invalid = store
            .read(&engine, &token, Some("bytes=999999999-"), false)
            .unwrap();
        assert_eq!(invalid.status, 416);
        assert!(invalid.bytes.is_empty());
        let head = store.read(&engine, &token, None, true).unwrap();
        assert_eq!(head.length as usize, bytes.len());
        assert!(head.bytes.is_empty());
        store.close(&token).unwrap();
        assert!(store.read(&engine, &token, None, false).is_err());
    }
    #[test]
    fn sessions_expire_and_do_not_accept_paths_or_other_tokens() {
        let (_temp, engine, _root, id) = fixture("generated.mp3", b"generated audio fixture");
        let store = PreviewStore::default();
        let token = store.open(&engine, id).unwrap().token.unwrap();
        assert_eq!(token.len(), 64);
        assert!(store
            .read(&engine, "../generated.mp3", None, false)
            .is_err());
        store
            .grants
            .lock()
            .unwrap()
            .get_mut(&token)
            .unwrap()
            .touched = Instant::now() - TTL - Duration::from_secs(1);
        assert!(store.read(&engine, &token, None, false).is_err());
    }
    #[test]
    fn changed_removed_and_excluded_files_cannot_keep_streaming() {
        let (_temp, engine, root, id) = fixture("generated.mp4", b"generated video fixture");
        let store = PreviewStore::default();
        let token = store.open(&engine, id).unwrap().token.unwrap();
        fs::write(root.join("generated.mp4"), b"different sized bytes").unwrap();
        assert!(store.read(&engine, &token, None, false).is_err());
        engine.scan_mode(false).unwrap();
        let token = store.open(&engine, id).unwrap().token.unwrap();
        engine
            .save_scan_preferences(crate::scan_policy::Preferences {
                exclusions: vec!["*.mp4".into()],
                ..Default::default()
            })
            .unwrap();
        assert!(store.read(&engine, &token, None, false).is_err());
        engine.save_scan_preferences(Default::default()).unwrap();
        let root_id = db::file(&db::connect(&engine.0.db).unwrap(), id)
            .unwrap()
            .root_id;
        engine.remove_root(root_id).unwrap();
        assert!(store.read(&engine, &token, None, false).is_err());
    }
    #[test]
    fn redirected_preview_path_cannot_expose_an_unselected_file() {
        let (temp, engine, root, id) = fixture("generated.mp4", b"generated video fixture");
        let store = PreviewStore::default();
        let token = store.open(&engine, id).unwrap().token.unwrap();
        let moved = temp.path().join("moved-selected");
        fs::rename(&root, &moved).unwrap();
        fs::create_dir(&root).unwrap();
        fs::write(root.join("generated.mp4"), b"outside replacement").unwrap();
        assert!(store.read(&engine, &token, None, false).is_err());
    }
    #[test]
    fn text_preview_is_bounded_and_keeps_html_as_plain_text() {
        let mut bytes = b"<script>window.BAD = true</script>\n".to_vec();
        bytes.resize(TEXT_LIMIT + 40, b'a');
        let (_temp, engine, _root, id) = fixture("generated.html", &bytes);
        let info = PreviewStore::default().open(&engine, id).unwrap();
        assert_eq!(info.kind, Kind::Text);
        assert!(info.truncated);
        assert!(info.token.is_none());
        let text = info.text.unwrap();
        assert_eq!(text.len(), TEXT_LIMIT);
        assert!(text.starts_with("<script>"));
        assert!(decode_text(b"binary\0payload", false).is_err());
        assert_eq!(
            decode_text(&[0xff, 0xfe, b'A', 0, 0xac, 0x20], false)
                .unwrap()
                .0,
            "A€"
        );
        assert_eq!(decode_text(&[b'a', 0xe2, 0x82], true).unwrap().0, "a");
    }
    #[test]
    fn pdf_and_unsupported_files_are_checked_before_creating_a_session() {
        let (_temp, engine, root, id) = fixture("generated.pdf", b"not a PDF");
        let store = PreviewStore::default();
        assert!(store.open(&engine, id).is_err());
        fs::write(root.join("generated.pdf"), b"%PDF-1.7\ngenerated fixture").unwrap();
        engine.scan_mode(false).unwrap();
        let info = store.open(&engine, id).unwrap();
        let token = info.token.unwrap();
        let data = store.read(&engine, &token, None, false).unwrap();
        assert_eq!(data.status, 200);
        assert!(data.bytes.starts_with(b"%PDF-"));
        fs::OpenOptions::new()
            .write(true)
            .open(root.join("generated.pdf"))
            .unwrap()
            .set_len(PDF_LIMIT + 1)
            .unwrap();
        engine.scan_mode(false).unwrap();
        assert!(store.open(&engine, id).unwrap_err().contains("40 MB"));
        assert!(format("docx", "generated.docx").is_none());
        assert!(byte_window(Some("bytes=0-1,5-6"), 10, CHUNK_LIMIT).is_err());
    }
}
