//! Signed updates through the official Tauri verifier. No renderer-supplied URLs.
use digview_core::{db::Result, engine::Engine, update_policy};
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const ENDPOINT: &str = match option_env!("DIGVIEW_UPDATER_ENDPOINT") {
    Some(s) => s,
    None => "",
};
const PUBLIC_KEY: &str = match option_env!("DIGVIEW_UPDATER_PUBLIC_KEY") {
    Some(s) => s,
    None => "",
};
#[derive(Default)]
pub struct Updates {
    pending: Mutex<Option<(Instant, Update)>>,
    busy: AtomicBool,
}
#[derive(Serialize)]
pub struct Status {
    pub configured: bool,
    pub current: String,
    pub available: Option<String>,
    pub notes: Option<String>,
}
struct Busy<'a>(&'a AtomicBool);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
fn enabled() -> bool {
    update_policy::configured(ENDPOINT, PUBLIC_KEY, cfg!(debug_assertions))
        && update_policy::valid_public_key(PUBLIC_KEY)
}
pub fn install_plugin(app: &tauri::AppHandle) -> Result<()> {
    if enabled() {
        app.plugin(
            tauri_plugin_updater::Builder::new()
                .pubkey(PUBLIC_KEY)
                .build(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub fn update_status(state: State<'_, Updates>) -> Result<Status> {
    let pending = state.pending.lock().map_err(|e| e.to_string())?;
    let update = pending
        .as_ref()
        .filter(|(t, _)| t.elapsed() < Duration::from_secs(3600));
    Ok(Status {
        configured: enabled(),
        current: env!("CARGO_PKG_VERSION").into(),
        available: update.map(|(_, u)| u.version.clone()),
        notes: update.and_then(|(_, u)| u.body.as_ref().map(|b| b.chars().take(4000).collect())),
    })
}
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle, state: State<'_, Updates>) -> Result<Status> {
    if !enabled() {
        return Err("Signed updates are not configured for this build".into());
    }
    if state.busy.swap(true, Ordering::AcqRel) {
        return Err("An update request is already in progress".into());
    }
    let _busy = Busy(&state.busy);
    *state.pending.lock().map_err(|e| e.to_string())? = None;
    let endpoint = update_policy::public_https(ENDPOINT)?;
    let update = app
        .updater_builder()
        .pubkey(PUBLIC_KEY)
        .endpoints(vec![endpoint])
        .map_err(|e| e.to_string())?
        .timeout(Duration::from_secs(120))
        .configure_client(|c| {
            c.redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5
                    || update_policy::public_artifact_url(attempt.url().as_str()).is_err()
                {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
        })
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|_| "Could not check for updates. Check your connection and retry.")?;
    if let Some(ref u) = update {
        update_policy::public_artifact_url(u.download_url.as_str())?;
    }
    *state.pending.lock().map_err(|e| e.to_string())? = update.map(|u| (Instant::now(), u));
    drop(_busy);
    update_status(state)
}
#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    state: State<'_, Updates>,
    confirmed_version: String,
) -> Result<()> {
    if !enabled() {
        return Err("Signed updates are not configured".into());
    }
    if app.state::<Engine>().job().running {
        return Err("Finish or cancel the current library task before updating".into());
    }
    if state.busy.swap(true, Ordering::AcqRel) {
        return Err("An update request is already in progress".into());
    }
    let _busy = Busy(&state.busy);
    let update = {
        let mut pending = state.pending.lock().map_err(|e| e.to_string())?;
        let (at, u) = pending.as_ref().ok_or("Check for updates first")?;
        if at.elapsed() > Duration::from_secs(3600) || u.version != confirmed_version {
            return Err("Update changed or expired. Check again before installing.".into());
        }
        pending.take().unwrap().1
    };
    update_policy::public_artifact_url(update.download_url.as_str())?;
    // Official download verifies the embedded signature. Never install on error.
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|_| "Update download or signature verification failed. Nothing was installed.")?;
    update_policy::verify_artifact(&bytes, &update.signature, PUBLIC_KEY, &update.version)?;
    let engine = app.state::<Engine>().inner().clone();
    let worker = engine.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Reserve the normal job slot through backup and installation so watcher,
        // scans, root changes and cleanup cannot race the handover.
        worker.begin("update")?;
        if let Err(error) = worker.update_snapshot() {
            worker.finish_update(Err(error.clone()));
            return Err(error);
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;
    let worker = engine.clone();
    let result = match tauri::async_runtime::spawn_blocking(move || {
        update.install(bytes).map_err(|e| e.to_string())
    })
    .await
    {
        Ok(result) => result,
        Err(e) => Err(e.to_string()),
    };
    worker.finish_update(result.clone());
    result
}
