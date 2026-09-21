// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if app_lib::run_smoke_if_requested() {
        return;
    }
    if app_lib::run_helper_if_requested() {
        return;
    }
    app_lib::run();
}
