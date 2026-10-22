fn main() {
    if !digview_core::document_extract::worker_main_if_requested() {
        eprintln!("Document parser protocol fixture helper");
        std::process::exit(2);
    }
}
