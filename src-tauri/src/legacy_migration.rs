//! Import an earlier installation once, before creating the new WebView.
//! Legacy identifiers belong only in this migration bridge.
pub fn private_data_dirs() -> Vec<std::path::PathBuf> {
    #[cfg(windows)]
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        let old = std::path::Path::new(&base).join("app.filolocal.desktop");
        if old.is_dir() {
            return vec![old];
        }
    }
    Vec::new()
}

#[cfg(windows)]
pub fn prepare() -> Result<(), String> {
    use std::{
        fs,
        os::windows::{fs::MetadataExt, process::CommandExt},
        path::Path,
        process::Command,
    };
    use winreg::{enums::*, RegKey};

    fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
        let metadata = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
        // Never follow a symlink or Windows junction out of owned application data.
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(
                "Previous app data contains a linked folder. Migration requires review.".into(),
            );
        }
        if metadata.is_dir() {
            fs::create_dir(target).map_err(|e| e.to_string())?;
            for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if matches!(
                    entry.file_name().to_str(),
                    Some(
                        "library.sqlite3"
                            | "library.sqlite3-wal"
                            | "library.sqlite3-shm"
                            | "library.sqlite3-journal"
                    )
                ) {
                    continue;
                }
                copy_tree(&entry.path(), &target.join(entry.file_name()))?;
            }
        } else if metadata.is_file() {
            // Destination is a newly created staging tree; never overwrite an existing library.
            fs::copy(source, target).map_err(|e| e.to_string())?;
        } else {
            return Err("Previous app data contains an unsupported entry.".into());
        }
        Ok(())
    }

    let base =
        std::env::var_os("LOCALAPPDATA").ok_or("Windows application data is unavailable.")?;
    let old = Path::new(&base).join("app.filolocal.desktop");
    let target = Path::new(&base).join("app.digview.desktop");
    if !old.join("library.sqlite3").is_file() {
        return Ok(());
    }
    if !target.join("library.sqlite3").is_file() {
        if target.exists() {
            return Err("DigView data already exists without a library. Migration stopped to avoid overwriting data.".into());
        }
        let processes = Command::new("tasklist.exe")
            .args(["/FO", "CSV", "/NH"])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| e.to_string())?;
        if !processes.status.success() {
            return Err(
                "Could not check whether the previous app is closed. Retry DigView.".into(),
            );
        }
        for line in String::from_utf8_lossy(&processes.stdout).lines() {
            let mut fields = line.trim_matches('"').split("\",\"");
            let name = fields.next().unwrap_or("").to_ascii_lowercase();
            let pid = fields.next().and_then(|p| p.parse::<u32>().ok());
            if (name == "filo-local.exe"
                || name == "digview.exe"
                || name.starts_with("zee-plus-files"))
                && pid != Some(std::process::id())
            {
                return Err("Quit the previous app from its tray before opening DigView. Your library is preserved.".into());
            }
        }
        let stage = Path::new(&base).join(format!("app.digview.import-{}", std::process::id()));
        // A failed copy remains for diagnosis. It is never adopted on the next attempt.
        if stage.exists() {
            return Err(
                "A previous migration staging folder exists. Migration requires review.".into(),
            );
        }
        // SQLite owns its file locks; never raw-copy a database under an exclusive lock.
        let library = digview_core::db::connect(&old.join("library.sqlite3"))?;
        copy_tree(&old, &stage)?;
        library
            .execute(
                "VACUUM INTO ?",
                [stage.join("library.sqlite3").to_string_lossy().as_ref()],
            )
            .map_err(|e| e.to_string())?;
        fs::rename(&stage, &target).map_err(|e| e.to_string())?;
        // Old data is retained; there is no deletion and no scan of selected user folders.
    }

    // Transfer only this application's known startup command, preserving opt-in state.
    let key = match RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        KEY_READ | KEY_WRITE,
    ) {
        Ok(key) => key,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    let command = match key.get_value::<String, _>("FiloLocal") {
        Ok(command) => command,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    if let Some(exe) = command
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix("\" --startup"))
    {
        let name = Path::new(exe)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        if name == "filo-local.exe" || name == "digview.exe" || name.starts_with("zee-plus-files") {
            let current = std::env::current_exe().map_err(|e| e.to_string())?;
            let next = format!("\"{}\" --startup", current.display());
            match key.get_value::<String, _>("DigView") {
                Ok(existing) if existing != next => return Ok(()),
                Ok(_) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    key.set_value("DigView", &next).map_err(|e| e.to_string())?;
                }
                Err(e) => return Err(e.to_string()),
            }
            key.delete_value("FiloLocal").map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn prepare() -> Result<(), String> {
    Ok(())
}
