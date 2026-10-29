// The webview never calls this plugin: the Rust core does, through
// `run_mobile_plugin_async`, so no command is exposed to JavaScript.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
