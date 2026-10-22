pub use digview_core::local_diagnostics::{background_failure, Diagnostics, Entry};
use std::sync::Arc;

#[tauri::command]
pub async fn write_diagnostics(
    state: tauri::State<'_, Arc<Diagnostics>>,
    entries: Vec<Entry>,
) -> Result<(), String> {
    let logger = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || logger.record(entries))
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn open_diagnostics_folder(
    state: tauri::State<'_, Arc<Diagnostics>>,
) -> Result<(), String> {
    let logger = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        open::that_detached(logger.directory()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn export_diagnostics(state: tauri::State<'_, Arc<Diagnostics>>) -> Result<bool, String> {
    let logger = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let Some(destination) = rfd::FileDialog::new()
            .set_title("Save local diagnostics")
            .set_file_name(format!("DigView-diagnostics-{time}.jsonl"))
            .add_filter("Diagnostic log", &["jsonl"])
            .save_file()
        else {
            return Ok(false);
        };
        logger.export(&destination)?;
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn diagnostics_status(state: tauri::State<'_, Arc<Diagnostics>>) -> bool {
    state.healthy()
}
