use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub keep_running: bool,
    pub start_minimized: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            keep_running: true,
            start_minimized: true,
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(flatten)]
    preferences: Preferences,
    run_at_startup: bool,
    startup_supported: bool,
}
pub struct Background {
    path: PathBuf,
    preferences: Mutex<Preferences>,
}
impl Background {
    pub fn load(data: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let path = data.join("background.json");
        let preferences = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                eprintln!("Invalid background preferences; using defaults: {error}");
                Preferences::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            path,
            preferences: Mutex::new(preferences),
        })
    }
    pub fn keep_running(&self) -> bool {
        self.preferences
            .lock()
            .map(|p| p.keep_running)
            .unwrap_or(false)
    }
    pub fn launch_hidden(&self, startup: bool) -> bool {
        self.preferences
            .lock()
            .map(|p| startup && p.keep_running && p.start_minimized)
            .unwrap_or(false)
    }
    pub fn get(&self) -> Result<Settings, String> {
        Ok(Settings {
            preferences: self.preferences.lock().map_err(|e| e.to_string())?.clone(),
            run_at_startup: startup_enabled()?,
            startup_supported: cfg!(windows),
        })
    }
    pub fn save(&self, preferences: Preferences, run_at_startup: bool) -> Result<Settings, String> {
        let mut current = self.preferences.lock().map_err(|e| e.to_string())?;
        let old_startup = startup_enabled()?;
        // Do not publish the in-memory policy until both OS registration and disk save succeed.
        if old_startup != run_at_startup {
            set_startup(run_at_startup)?;
        }
        let write = serde_json::to_vec_pretty(&preferences)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                let temporary = self.path.with_extension("tmp");
                std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
                std::fs::rename(&temporary, &self.path).map_err(|e| e.to_string())
            });
        if let Err(error) = write {
            if old_startup != run_at_startup {
                if let Err(rollback) = set_startup(old_startup) {
                    return Err(format!("Settings save failed: {error}. Startup restore failed: {rollback}. Reopen settings to check."));
                }
            }
            return Err(error);
        }
        *current = preferences.clone();
        Ok(Settings {
            preferences,
            run_at_startup,
            startup_supported: cfg!(windows),
        })
    }
}

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const RUN_NAME: &str = "DigView";
#[cfg(windows)]
fn startup_command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(format!("\"{}\" --startup", exe.display()))
}
#[cfg(windows)]
fn startup_enabled() -> Result<bool, String> {
    use winreg::{enums::*, RegKey};
    let key = match RegKey::predef(HKEY_CURRENT_USER).open_subkey(RUN_KEY) {
        Ok(key) => key,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.to_string()),
    };
    match key.get_value::<String, _>(RUN_NAME) {
        Ok(command) => Ok(command == startup_command()?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(windows)]
fn set_startup(enabled: bool) -> Result<(), String> {
    use winreg::{enums::*, RegKey};
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(RUN_KEY)
        .map_err(|e| e.to_string())?;
    if enabled {
        key.set_value(RUN_NAME, &startup_command()?)
            .map_err(|e| e.to_string())
    } else {
        match key.delete_value(RUN_NAME) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
#[cfg(not(windows))]
fn startup_enabled() -> Result<bool, String> {
    Ok(false)
}
#[cfg(not(windows))]
fn set_startup(enabled: bool) -> Result<(), String> {
    if enabled {
        Err("Run at startup is currently supported on Windows only".into())
    } else {
        Ok(())
    }
}

pub fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
pub fn install_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open DigView", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit DigView", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let icon = app
        .default_window_icon()
        .expect("Bundled application icon")
        .clone();
    TrayIconBuilder::with_id("digview-background")
        .icon(icon)
        .tooltip("DigView — selected-folder indexing")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(app),
            "quit" => app.exit(0),
            _ => (),
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_launch_always_opens_and_login_requires_both_preferences() {
        for keep_running in [false, true] {
            for start_minimized in [false, true] {
                let state = Background {
                    path: PathBuf::new(),
                    preferences: Mutex::new(Preferences {
                        keep_running,
                        start_minimized,
                    }),
                };
                assert!(!state.launch_hidden(false));
                assert_eq!(state.launch_hidden(true), keep_running && start_minimized);
                assert_eq!(state.keep_running(), keep_running);
            }
        }
    }

    #[test]
    fn persisted_close_policy_survives_restart_and_invalid_json_recovers() {
        let dir = std::env::temp_dir().join(format!(
            "digview-background-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        assert!(Background::load(&dir).unwrap().keep_running());
        let path = dir.join("background.json");
        std::fs::write(&path, br#"{"keepRunning":false,"startMinimized":true}"#).unwrap();
        let loaded = Background::load(&dir).unwrap();
        assert!(!loaded.keep_running());
        assert!(!loaded.launch_hidden(true));
        std::fs::write(&path, b"interrupted JSON").unwrap();
        assert!(Background::load(&dir).unwrap().keep_running());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn startup_path_is_quoted_and_only_fixed_argument_is_added() {
        let exe = std::env::current_exe().unwrap();
        assert_eq!(
            startup_command().unwrap(),
            format!("\"{}\" --startup", exe.display())
        );
    }
}
