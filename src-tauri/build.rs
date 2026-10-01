fn main() {
    // A Cargo release profile alone still enables Tauri's devUrl. Never ship it.
    if std::env::var("PROFILE").as_deref() == Ok("release") && tauri_build::is_dev() {
        panic!("Release frontend is not embedded. Build with `tauri build` or `cargo build --release --features tauri/custom-protocol`.");
    }
    tauri_build::build()
}
