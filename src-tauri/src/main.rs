#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if digview_core::document_extract::worker_main_if_requested() {
        return;
    }
    digview_lib::run()
}
