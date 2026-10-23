//! Shared local engine. No webview, GUI, network client or cloud service dependency.
pub mod content_preview;
pub mod db;
pub mod engine;
pub mod local_diagnostics;
pub mod model;
pub mod platform;
pub mod preview;
pub mod setup;
pub mod watcher;

pub mod scan_policy;

pub mod discovery;
pub mod document_extract;
pub mod document_index;
pub mod update_policy;
pub mod visibility;

pub mod semantic;

pub mod query_language;
