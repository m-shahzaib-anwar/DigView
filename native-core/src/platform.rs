use crate::{db::Result, engine::Engine};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn source_host(value: &str) -> Option<String> {
    let url = url::Url::parse(value.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.host_str()
        .map(|h| h.trim_start_matches("www.").to_lowercase())
}
#[cfg(target_os = "windows")]
pub fn origin(path: &Path) -> Option<String> {
    let mut stream = String::new();
    std::fs::File::open(format!("{}:Zone.Identifier", path.display()))
        .ok()?
        .take(65536)
        .read_to_string(&mut stream)
        .ok()?;
    for prefix in ["HostUrl=", "ReferrerUrl="] {
        for line in stream.lines() {
            if let Some(value) = line.strip_prefix(prefix) {
                if let Some(host) = source_host(value) {
                    return Some(host);
                }
            }
        }
    }
    None
}
#[cfg(target_os = "macos")]
pub fn origin(path: &Path) -> Option<String> {
    let data = xattr::get(path, "com.apple.metadata:kMDItemWhereFroms").ok()??;
    if data.len() > 65536 {
        return None;
    }
    let value = plist::Value::from_reader(std::io::Cursor::new(data)).ok()?;
    value
        .as_array()?
        .iter()
        .filter_map(|v| v.as_string())
        .find_map(source_host)
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn origin(path: &Path) -> Option<String> {
    let data = xattr::get(path, "user.xdg.origin.url").ok()??;
    if data.len() > 65536 {
        return None;
    }
    source_host(std::str::from_utf8(&data).ok()?)
}
pub fn ocr_capability() -> String {
    if cfg!(target_os = "windows") {
        "Windows OCR (local language packs)".into()
    } else {
        "Bundled offline Tesseract · English".into()
    }
}
pub fn recognize(engine: &Engine, id: i64) -> Result<String> {
    let (record, path) = engine.checked_file(id)?;
    if record.category != "images" {
        return Err("Text recognition is available for images.".into());
    }
    if record.size > 40 * 1024 * 1024 {
        return Err("For text recognition, choose an image under 40 MB.".into());
    }
    #[cfg(target_os = "windows")]
    {
        use base64::Engine as _;
        let script = include_str!("windows-ocr.ps1");
        let encoded = base64::engine::general_purpose::STANDARD.encode(
            script
                .encode_utf16()
                .flat_map(|x| x.to_le_bytes())
                .collect::<Vec<_>>(),
        );
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-EncodedCommand",
                &encoded,
            ])
            .env("DIGVIEW_OCR_PATH", &path);
        return capture(&mut command, Duration::from_secs(90));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut command = Command::new("tesseract");
        command.arg(&path).args(["stdout", "--psm", "3"]);
        capture(&mut command, Duration::from_secs(90))
    }
}
fn quiet(command: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}
fn capture(command: &mut Command, timeout: Duration) -> Result<String> {
    let mut child = quiet(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Local OCR is unavailable: {e}"))?;
    let out = child.stdout.take().ok_or("Could not read OCR output")?;
    let err = child.stderr.take().ok_or("Could not read OCR errors")?;
    let stdout = thread::spawn(move || {
        let mut bytes = Vec::new();
        out.take(4 * 1024 * 1024).read_to_end(&mut bytes).ok();
        bytes
    });
    let stderr = thread::spawn(move || {
        let mut bytes = Vec::new();
        err.take(65536).read_to_end(&mut bytes).ok();
        bytes
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Text recognition timed out. Try a smaller image.".into());
        }
        thread::sleep(Duration::from_millis(80));
    };
    let out = stdout
        .join()
        .map_err(|_| "Could not read recognized text")?;
    let err = stderr
        .join()
        .map_err(|_| "Could not read recognition status")?;
    if !status.success() {
        return Err(format!(
            "Local OCR failed. Check installed OCR language packs. {}",
            String::from_utf8_lossy(&err)
                .chars()
                .take(400)
                .collect::<String>()
        ));
    }
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origin_only_displays_a_safe_host() {
        assert_eq!(
            source_host("https://www.github.com/user/repo?token=secret"),
            Some("github.com".into())
        );
        assert_eq!(source_host("file:///etc/passwd"), None);
        assert_eq!(source_host("javascript:alert(1)"), None);
    }
}
