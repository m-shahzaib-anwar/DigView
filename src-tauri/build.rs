#[cfg(feature = "desktop")]
fn main() {
    println!("cargo:rerun-if-env-changed=DIGVIEW_UPDATER_ENDPOINT");
    println!("cargo:rerun-if-env-changed=DIGVIEW_UPDATER_PUBLIC_KEY");
    tauri_build::build()
}
#[cfg(not(feature = "desktop"))]
fn main() {}
