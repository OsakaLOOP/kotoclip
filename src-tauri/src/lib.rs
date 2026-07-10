pub mod state;
pub mod commands;

use std::fs;
use std::path::Path;
use std::sync::Mutex;
use state::AppState;
use kotoclip_core::Engine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 1. 初始化本地数据存储目录与字典放置路径
    let base_dir = Path::new("D:\\PROJ\\GIT\\kotoclip");
    let data_dir = base_dir.join("data");
    let dicts_dir = data_dir.join("dicts");
    let user_db_path = data_dir.join("user_profile.db");
    
    // IPADIC 字典文件所在路径 (由用户已提前下载放置)
    let ipadic_dict_path = base_dir.join("ipadic").join("system.dic");

    // 自动创建本地 data 和 data/dicts 文件夹
    if !dicts_dir.exists() {
        fs::create_dir_all(&dicts_dir).expect("无法创建字典放置目录");
    }

    // 2. 初始化核心分词及查词 Engine 实例
    let engine = Engine::new(
        &ipadic_dict_path,
        &dicts_dir,
        &user_db_path,
    ).expect("初始化 Kotoclip 核心引擎失败");

    // 3. 构建并启动 Tauri 桌面应用
    tauri::Builder::default()
        // 注册全局并发安全状态供 Command 使用
        .manage(AppState {
            engine: Mutex::new(engine),
        })
        .plugin(tauri_plugin_opener::init())
        // 注册所有和前端 IPC 交互的 Command 处理器
        .invoke_handler(tauri::generate_handler![
            commands::analyze_text,
            commands::lookup_word,
            commands::mark_known,
            commands::mark_unknown,
            commands::add_merge_rule,
            commands::export_selected,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 桌面应用运行出错");
}
