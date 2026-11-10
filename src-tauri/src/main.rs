// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().skip(1).any(|a| a == "--cleanup") {
        zastava_lib::sysproxy::cleanup_all_persistent_settings();
        return;
    }
    zastava_lib::run()
}
