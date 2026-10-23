use digview_core::{db, engine, model, platform, preview};

#[cfg(feature = "desktop")]
mod background;

#[cfg(feature = "desktop")]
mod updates;
#[cfg(feature = "desktop")]
mod semantic;

#[cfg(feature = "desktop")]
mod content_preview;

#[cfg(feature = "desktop")]
mod diagnostics;

#[cfg(feature = "desktop")]
mod legacy_migration;

#[cfg(feature = "desktop")]
mod desktop {
    use super::*;
    use db::Result;
    use engine::Engine;
    use model::*;
    use std::sync::atomic::Ordering;
    use tauri::{Manager, State};

    fn suggested_folder(app: &tauri::AppHandle, key: &str) -> Result<std::path::PathBuf> {
        let resolver = app.path();
        match key {
            "documents" => resolver.document_dir(),
            "pictures" => resolver.picture_dir(),
            "videos" => resolver.video_dir(),
            "music" => resolver.audio_dir(),
            "desktop" => resolver.desktop_dir(),
            "downloads" => resolver.download_dir(),
            _ => return Err("Unknown suggested folder.".into()),
        }
        .map_err(|_| "This suggested folder is unavailable on this device.".into())
    }

    #[derive(serde::Serialize)]
    struct SetupSuggestion {
        key: &'static str,
        label: &'static str,
        path: String,
        available: bool,
        covered: bool,
        overlap: bool,
    }

    #[tauri::command]
    async fn setup_suggestions(
        app: tauri::AppHandle,
        state: State<'_, Engine>,
    ) -> Result<Vec<SetupSuggestion>> {
        let e = state.inner().clone();
        background(move || {
            let roots = db::roots(&db::connect(&e.0.db)?)?;
            Ok([
                ("documents", "Documents"),
                ("pictures", "Pictures"),
                ("videos", "Videos"),
                ("music", "Music"),
                ("desktop", "Desktop"),
                ("downloads", "Downloads"),
            ]
            .into_iter()
            .map(|(key, label)| {
                let resolved = suggested_folder(&app, key).ok();
                let canonical = resolved
                    .as_ref()
                    .and_then(|p| std::fs::canonicalize(p).ok())
                    .filter(|p| {
                        p.is_dir() && p.to_str().is_some() && !p.starts_with(&e.0.app_data)
                    });
                let covered = canonical
                    .as_ref()
                    .is_some_and(|p| roots.iter().any(|root| p.starts_with(&root.path)));
                let overlap = !covered
                    && canonical.as_ref().is_some_and(|p| {
                        roots
                            .iter()
                            .any(|root| std::path::Path::new(&root.path).starts_with(p))
                    });
                SetupSuggestion {
                    key,
                    label,
                    path: canonical
                        .as_ref()
                        .or(resolved.as_ref())
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    available: canonical.is_some(),
                    covered,
                    overlap,
                }
            })
            .collect())
        })
        .await
    }

    #[tauri::command]
    async fn choose_suggested_setup_folder(
        app: tauri::AppHandle,
        state: State<'_, Engine>,
        key: String,
    ) -> Result<digview_core::setup::Folder> {
        let e = state.inner().clone();
        // A checkbox explicitly selects one allowlisted OS folder, never a renderer path.
        background(move || e.grant_folder_choice(&suggested_folder(&app, &key)?)).await
    }

    #[tauri::command]
    async fn setup_status(state: State<'_, Engine>) -> Result<digview_core::setup::Status> {
        let e = state.inner().clone();
        background(move || e.setup_status()).await
    }
    #[tauri::command]
    async fn choose_setup_folder(
        state: State<'_, Engine>,
    ) -> Result<Option<digview_core::setup::Folder>> {
        let folder = rfd::AsyncFileDialog::new()
            .set_title("Choose a folder for DigView")
            .pick_folder()
            .await;
        let e = state.inner().clone();
        match folder {
            Some(folder) => background(move || e.grant_folder_choice(folder.path()))
                .await
                .map(Some),
            None => Ok(None),
        }
    }
    #[tauri::command]
    async fn discard_folder_choices(state: State<'_, Engine>, tokens: Vec<String>) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.discard_folder_choices(tokens)).await
    }
    #[tauri::command]
    async fn add_chosen_folders(
        state: State<'_, Engine>,
        tokens: Vec<String>,
    ) -> Result<Vec<Root>> {
        let e = state.inner().clone();
        background(move || e.add_folder_choices(tokens)).await
    }
    #[tauri::command]
    async fn finish_setup(
        state: State<'_, Engine>,
        request: digview_core::setup::Request,
    ) -> Result<digview_core::setup::Status> {
        let e = state.inner().clone();
        background(move || e.finish_setup(request)).await
    }
    #[tauri::command]
    async fn open_library(app: tauri::AppHandle, state: State<'_, Engine>) -> Result<()> {
        let e = state.inner().clone();
        let completed = background(move || e.setup_status()).await?.completed;
        if !completed {
            return Err("Finish setup before opening the library.".into());
        }
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_title("DigView");
            let _ = window.set_min_size(Some(tauri::LogicalSize::new(920.0, 640.0)));
            let _ = window.set_size(tauri::LogicalSize::new(1360.0, 900.0));
            let _ = window.center();
        }
        Ok(())
    }

    #[tauri::command]
    async fn background_settings(app: tauri::AppHandle) -> Result<crate::background::Settings> {
        background(move || app.state::<crate::background::Background>().get()).await
    }
    #[tauri::command]
    async fn set_background_settings(
        app: tauri::AppHandle,
        preferences: crate::background::Preferences,
        run_at_startup: bool,
    ) -> Result<crate::background::Settings> {
        background(move || {
            app.state::<crate::background::Background>()
                .save(preferences, run_at_startup)
        })
        .await
    }

    async fn background<T: Send + 'static>(
        f: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let result = tauri::async_runtime::spawn_blocking(f)
            .await
            .map_err(|e| e.to_string())?;
        if let Err(error) = &result {
            crate::diagnostics::background_failure(error);
        }
        result
    }
    #[tauri::command]
    async fn bootstrap(state: State<'_, Engine>) -> Result<Bootstrap> {
        let engine = state.inner().clone();
        background(move || {
            let conn = db::connect(&engine.0.db)?;
            Ok(Bootstrap {
                roots: db::roots(&conn)?,
                stats: db::stats(&conn)?,
                job: engine.job(),
                platform: std::env::consts::OS.into(),
                ocr: platform::ocr_capability(),
                version: env!("CARGO_PKG_VERSION").into(),
            })
        })
        .await
    }
    #[tauri::command]
    async fn document_index_status(
        state: State<'_, Engine>,
    ) -> Result<digview_core::document_index::Status> {
        let e = state.inner().clone();
        background(move || e.document_index_status()).await
    }
    #[tauri::command]
    async fn set_document_index_enabled(state: State<'_, Engine>, enabled: bool) -> Result<()> {
        let e = state.inner().clone();
        background(move || {
            e.set_document_index_enabled(enabled)?;
            if enabled && !(e.job().running && e.job().kind == "content") {
                e.start_document_index(false)?;
            }
            Ok(())
        })
        .await
    }
    #[tauri::command]
    fn start_document_index(state: State<'_, Engine>, retry: bool) -> Result<()> {
        state.start_document_index(retry)
    }
    #[tauri::command]
    async fn clear_document_index(state: State<'_, Engine>) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.clear_document_index()).await
    }
    #[tauri::command]
    async fn scan_status(state: State<'_, Engine>) -> Result<digview_core::scan_policy::Status> {
        let e = state.inner().clone();
        background(move || e.scan_status()).await
    }
    #[tauri::command]
    async fn set_scan_preferences(
        state: State<'_, Engine>,
        preferences: digview_core::scan_policy::Preferences,
    ) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.save_scan_preferences(preferences)).await
    }
    #[tauri::command]
    fn job_status(state: State<'_, Engine>) -> Job {
        state.job()
    }
    #[tauri::command]
    async fn hidden_items(
        state: State<'_, Engine>,
    ) -> Result<Vec<digview_core::visibility::HiddenItem>> {
        let e = state.inner().clone();
        background(move || e.hidden_items()).await
    }
    #[tauri::command]
    async fn hide_files(
        state: State<'_, Engine>,
        ids: Vec<i64>,
        containing_folders: bool,
    ) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.hide_files(ids, containing_folders)).await
    }
    #[tauri::command]
    async fn hide_root(state: State<'_, Engine>, id: i64) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.hide_root(id)).await
    }
    #[tauri::command]
    async fn choose_hidden_folder(state: State<'_, Engine>) -> Result<bool> {
        let folder = rfd::AsyncFileDialog::new()
            .set_title("Hide a folder inside your selected library folders")
            .pick_folder()
            .await;
        let e = state.inner().clone();
        match folder {
            Some(folder) => background(move || e.hide_folder(folder.path()))
                .await
                .map(|_| true),
            None => Ok(false),
        }
    }
    #[tauri::command]
    async fn unhide_item(state: State<'_, Engine>, id: i64) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.unhide(id)).await
    }
    #[tauri::command]
    async fn personal_library(
        state: State<'_, Engine>,
    ) -> Result<digview_core::discovery::PersonalLibrary> {
        let e = state.inner().clone();
        background(move || e.personal_library()).await
    }
    #[tauri::command]
    async fn set_favourite(state: State<'_, Engine>, id: i64, enabled: bool) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.favourite(id, enabled)).await
    }
    #[tauri::command]
    async fn save_search(
        state: State<'_, Engine>,
        name: String,
        filters: digview_core::discovery::SearchState,
    ) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.save_search(name, filters)).await
    }
    #[tauri::command]
    async fn remove_search(state: State<'_, Engine>, id: i64) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.remove_search(id)).await
    }
    #[tauri::command]
    async fn ocr_identity(
        state: State<'_, Engine>,
        id: i64,
    ) -> Result<digview_core::discovery::OcrIdentity> {
        let e = state.inner().clone();
        background(move || e.ocr_identity(id)).await
    }
    #[tauri::command]
    async fn save_image_text(
        state: State<'_, Engine>,
        identity: digview_core::discovery::OcrIdentity,
        text: String,
    ) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.save_image_text(identity, text)).await
    }
    #[tauri::command]
    async fn clear_image_text(state: State<'_, Engine>) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.clear_image_text()).await
    }
    #[tauri::command]
    async fn get_image_text(state: State<'_, Engine>, id: i64) -> Result<Option<String>> {
        let e = state.inner().clone();
        background(move || e.image_text(id)).await
    }
    #[tauri::command]
    async fn choose_folder(state: State<'_, Engine>) -> Result<Option<Root>> {
        let engine = state.inner().clone();
        let path = rfd::AsyncFileDialog::new()
            .set_title("Choose a folder for your library")
            .pick_folder()
            .await;
        match path {
            Some(folder) => background(move || engine.add_root(folder.path()))
                .await
                .map(Some),
            None => Ok(None),
        }
    }
    #[tauri::command]
    async fn remove_root(state: State<'_, Engine>, id: i64) -> Result<()> {
        let e = state.inner().clone();
        background(move || e.remove_root(id)).await
    }
    #[tauri::command]
    fn start_scan(state: State<'_, Engine>) -> Result<()> {
        state.start_scan()
    }
    #[tauri::command]
    fn start_duplicates(state: State<'_, Engine>) -> Result<()> {
        state.start_duplicates()
    }
    #[tauri::command]
    async fn cancel_job(state: State<'_, Engine>) -> Result<()> {
        if state.job().kind == "update" {
            return Ok(());
        }
        if state.job().kind == "content" {
            let e = state.inner().clone();
            return background(move || e.set_document_index_enabled(false)).await;
        }
        state.inner().0.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }
    #[tauri::command]
    async fn query_files(state: State<'_, Engine>, query: Query) -> Result<QueryResult> {
        let e = state.inner().clone();
        background(move || db::query(&db::connect(&e.0.db)?, query)).await
    }
    #[tauri::command]
    async fn duplicate_groups(state: State<'_, Engine>) -> Result<Vec<DuplicateGroup>> {
        let e = state.inner().clone();
        background(move || db::duplicate_groups(&db::connect(&e.0.db)?)).await
    }
    #[tauri::command]
    async fn get_thumbnail(state: State<'_, Engine>, id: i64, large: bool) -> Result<String> {
        let e = state.inner().clone();
        background(move || preview::thumbnail(&e, id, large)).await
    }
    #[tauri::command]
    async fn open_preview(
        window: tauri::WebviewWindow,
        state: State<'_, Engine>,
        store: State<'_, std::sync::Arc<digview_core::content_preview::PreviewStore>>,
        id: i64,
    ) -> Result<digview_core::content_preview::PreviewInfo> {
        if window.label() != "main" {
            return Err("Preview is only available in the library window.".into());
        }
        let engine = state.inner().clone();
        let store = store.inner().clone();
        background(move || store.open(&engine, id)).await
    }
    #[tauri::command]
    fn close_preview(
        store: State<'_, std::sync::Arc<digview_core::content_preview::PreviewStore>>,
        token: String,
    ) -> Result<()> {
        store.close(&token)
    }
    #[tauri::command]
    async fn open_file(state: State<'_, Engine>, id: i64) -> Result<()> {
        let e = state.inner().clone();
        background(move || {
            let (_, path) = e.checked_file(id)?;
            open::that_detached(path).map_err(|e| e.to_string())
        })
        .await
    }
    #[tauri::command]
    async fn reveal_file(state: State<'_, Engine>, id: i64) -> Result<()> {
        let e = state.inner().clone();
        background(move || {
            let (_, path) = e.checked_file(id)?;
            let parent = path.parent().ok_or("No containing folder")?;
            open::that_detached(parent).map_err(|e| e.to_string())
        })
        .await
    }
    #[tauri::command]
    async fn recognize_text(state: State<'_, Engine>, id: i64) -> Result<String> {
        let e = state.inner().clone();
        background(move || {
            let identity = e.ocr_identity(id)?;
            let text = platform::recognize(&e, id)?;
            e.save_image_text(identity, text.clone())?;
            Ok(text)
        })
        .await
    }
    #[tauri::command]
    async fn trash_files(
        state: State<'_, Engine>,
        files: Vec<TrashSelection>,
        duplicate_only: bool,
    ) -> Result<TrashResult> {
        let e = state.inner().clone();
        background(move || e.trash(files, duplicate_only)).await
    }

    pub fn run() {
        // Run before WebView creation so its profile can be imported while closed.
        let imported = std::thread::spawn(crate::legacy_migration::prepare)
            .join()
            .unwrap_or_else(|_| Err("DigView migration could not finish.".into()));
        if let Err(error) = imported {
            rfd::MessageDialog::new()
                .set_title("DigView")
                .set_description(format!("Could not import your previous library. {error}"))
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        }
        tauri::Builder::default()
            .register_asynchronous_uri_scheme_protocol(
                "digview-preview",
                crate::content_preview::serve,
            )
            .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
                if !args.iter().any(|arg| arg == "--startup") {
                    crate::background::show(app);
                }
            }))
            .plugin(tauri_plugin_clipboard_manager::init())
            .setup(|app| {
                let data = app.path().app_local_data_dir()?;
                app.manage(crate::diagnostics::Diagnostics::initialize(
                    &data,
                    env!("CARGO_PKG_VERSION"),
                ));
                app.manage(crate::updates::Updates::default());
                app.manage(crate::semantic::Semantic::default());
                app.manage(std::sync::Arc::new(
                    digview_core::content_preview::PreviewStore::default(),
                ));
                crate::updates::install_plugin(app.handle()).map_err(std::io::Error::other)?;
                let engine = Engine::new_with_private_data(&data, &crate::legacy_migration::private_data_dirs())
                    .map_err(std::io::Error::other)?;
                let setting_up = !engine
                    .setup_status()
                    .map_err(std::io::Error::other)?
                    .completed;
                let preferences = crate::background::Background::load(&data)?;
                let hidden = !setting_up
                    && preferences.launch_hidden(std::env::args().any(|arg| arg == "--startup"));
                app.manage(preferences);
                crate::background::install_tray(app)?;
                preview::prune_cache(&engine);
                crate::semantic::start_automation(engine.clone(), app.state::<crate::semantic::Semantic>().inner().clone());
                app.manage(engine.watch_selected());
                app.manage(engine);
                if setting_up {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.set_title("Set up DigView");
                        let _ = window.set_min_size(Some(tauri::LogicalSize::new(760.0, 640.0)));
                        let _ = window.set_size(tauri::LogicalSize::new(900.0, 760.0));
                        let _ = window.center();
                    }
                }
                if !hidden {
                    crate::background::show(app.handle());
                }
                Ok(())
            })
            .on_window_event(|window, event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if window.label() == "main"
                        && window
                            .state::<crate::background::Background>()
                            .keep_running()
                    {
                        // Only intercept after hiding succeeds; never leave an inaccessible process.
                        if window.hide().is_ok() {
                            api.prevent_close();
                        }
                    }
                }
            })
            .invoke_handler(tauri::generate_handler![
                crate::semantic::semantic_status,
                crate::semantic::semantic_action,
                crate::semantic::semantic_query,
                crate::semantic::matched_document,
                setup_status,
                setup_suggestions,
                choose_suggested_setup_folder,
                add_chosen_folders,
                discard_folder_choices,
                choose_setup_folder,
                finish_setup,
                open_library,
                crate::diagnostics::write_diagnostics,
                crate::diagnostics::diagnostics_status,
                crate::diagnostics::open_diagnostics_folder,
                crate::diagnostics::export_diagnostics,
                bootstrap,
                background_settings,
                set_background_settings,
                job_status,
                crate::updates::update_status,
                crate::updates::check_update,
                crate::updates::install_update,
                personal_library,
                set_favourite,
                save_search,
                remove_search,
                ocr_identity,
                save_image_text,
                clear_image_text,
                get_image_text,
                scan_status,
                document_index_status,
                set_document_index_enabled,
                start_document_index,
                clear_document_index,
                set_scan_preferences,
                choose_folder,
                remove_root,
                start_scan,
                start_duplicates,
                cancel_job,
                query_files,
                duplicate_groups,
                get_thumbnail,
                hidden_items,
                hide_files,
                hide_root,
                choose_hidden_folder,
                unhide_item,
                open_preview,
                close_preview,
                open_file,
                reveal_file,
                recognize_text,
                trash_files
            ])
            .build(tauri::generate_context!())
            .expect("Could not start DigView")
            .run(|app, event| {
                if let tauri::RunEvent::Exit = event {
                    crate::semantic::shutdown(app);
                    app.state::<std::sync::Arc<crate::diagnostics::Diagnostics>>()
                        .clean_shutdown();
                }
            });
    }
}
#[cfg(feature = "desktop")]
pub use desktop::run;
