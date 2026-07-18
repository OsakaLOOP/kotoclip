// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Ok(duration) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        println!("[时间戳] Rust 程序已运行 (dev 转换为 running): {}", duration.as_millis());
    }
    tauri_app_lib::run()
}
