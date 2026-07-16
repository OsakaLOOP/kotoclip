pub mod commands;
pub mod paths;
pub mod state;

use kotoclip_core::{cache::AnalysisCache, Engine};
use state::AppState;
use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Mutex};
use tauri::{Emitter, Manager};

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendReadyEvent {
    ready: bool,
    error: Option<String>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 构建并启动 Tauri 桌面应用
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 先注册可等待的资源，让 Tauri 能立即进入事件循环并绘制前端。
            let engine = state::LazyResource::pending();
            let analysis_cache = state::LazyResource::pending();
            app.manage(AppState {
                engine: engine.clone(),
                sessions: Mutex::new(HashMap::new()),
                next_session_id: AtomicU64::new(1),
                analysis_cache: analysis_cache.clone(),
            });

            // 词典和 SQLite 初始化可能持续数百毫秒到数秒，不能阻塞 WebView 首次绘制。
            let app_handle = app.handle().clone();
            std::thread::Builder::new()
                .name("kotoclip-backend-init".to_string())
                .spawn(move || {
                    let result = (|| -> Result<(), String> {
                        let paths = paths::AppPaths::resolve(&app_handle)
                            .map_err(|error| error.to_string())?;

                        let engine_value = Engine::new_from_dictionary_sources(
                            &paths.system_dictionary,
                            &paths.dictionary_source_dir,
                            &paths.dictionary_dir,
                            &paths.profile_db,
                        )
                        .map_err(|error| error.to_string())?;
                        engine.initialize(Ok(engine_value));

                        let cache_value = AnalysisCache::new(
                            paths.data_dir.join("cache").join("analysis"),
                            &paths.system_dictionary,
                            &paths.dictionary_dir,
                        )
                        .map_err(|error| error.to_string())?;
                        analysis_cache.initialize(Ok(cache_value));
                        Ok(())
                    })();

                    if let Err(error) = result {
                        engine.initialize(Err(error.clone()));
                        analysis_cache.initialize(Err(error.clone()));
                        let _ = app_handle.emit(
                            "backend-ready",
                            BackendReadyEvent {
                                ready: false,
                                error: Some(error),
                            },
                        );
                        return;
                    }

                    let _ = app_handle.emit(
                        "backend-ready",
                        BackendReadyEvent {
                            ready: true,
                            error: None,
                        },
                    );
                })
                .map_err(|error| format!("无法启动后台初始化线程：{error}"))?;

            Ok(())
        })
        // 注册所有和前端 IPC 交互的 Command 处理器
        .invoke_handler(tauri::generate_handler![
            commands::open_document,
            commands::backend_status,
            commands::continue_document_analysis,
            commands::finalize_document,
            commands::persist_document_cache,
            commands::request_document_range,
            commands::apply_document_mutation,
            commands::close_document,
            commands::refresh_document_expressions,
            commands::mark_document_known,
            commands::choose_document_segmentation,
            commands::lookup_word,
            commands::choose_dictionary_target,
            commands::search_grammar_catalog,
            commands::get_grammar_concept,
            commands::mark_known,
            commands::mark_unknown,
            commands::add_merge_rule,
            commands::add_expression_rule,
            commands::preview_expression_rule,
            commands::get_expression_rules,
            commands::delete_expression_rule,
            commands::get_candidates,
            commands::choose_segmentation,
            commands::export_selected,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 桌面应用运行出错");
}
