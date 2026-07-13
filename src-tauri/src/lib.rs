pub mod commands;
pub mod paths;
pub mod state;

use kotoclip_core::Engine;
use state::AppState;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 构建并启动 Tauri 桌面应用
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 1. 初始化本地数据存储目录与字典放置路径
            let paths = paths::AppPaths::resolve(app.handle())?;
            if let Some(patterns) = &paths.grammar_patterns {
                std::env::set_var("KOTOCLIP_GRAMMAR_PATTERNS", patterns);
            }

            // 自动创建本地 data 和 data/dicts 文件夹
            // 2. 初始化核心分词及查词 Engine 实例
            let engine = Engine::new(
                &paths.system_dictionary,
                &paths.dictionary_dir,
                &paths.profile_db,
            )?;

            // 注册全局并发安全状态供 Command 使用
            app.manage(AppState {
                engine: Mutex::new(engine),
            });

            Ok(())
        })
        // 注册所有和前端 IPC 交互的 Command 处理器
        .invoke_handler(tauri::generate_handler![
            commands::analyze_text,
            commands::lookup_word,
            commands::choose_dictionary_target,
            commands::mark_known,
            commands::mark_unknown,
            commands::add_merge_rule,
            commands::add_expression_rule,
            commands::preview_expression_rule,
            commands::get_expression_rules,
            commands::refresh_expression_annotations,
            commands::delete_expression_rule,
            commands::get_candidates,
            commands::choose_segmentation,
            commands::export_selected,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 桌面应用运行出错");
}
