pub mod commands;
pub mod paths;
pub mod state;

use kotoclip_core::library::ReaderLibrary;
use kotoclip_core::{cache::AnalysisCache, DictionaryService, Engine};
use state::AppState;
use std::any::Any;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use tauri::{Emitter, Manager};

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendReadyEvent {
    ready: bool,
    error: Option<String>,
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "未知 panic".to_string()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let run_started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    println!("[时间戳] Rust tauri_app_lib::run 开始: {}", run_started);

    // 构建并启动 Tauri 桌面应用
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let setup_entered = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            println!(
                "[时间戳] Rust setup 阶段进入: {}, 距离 run 开始: {}ms",
                setup_entered,
                setup_entered - run_started
            );

            let paths =
                paths::AppPaths::resolve(app.handle()).map_err(|error| error.to_string())?;
            let library =
                ReaderLibrary::open(&paths.library_dir).map_err(|error| error.to_string())?;

            // 先注册可等待的资源，让 Tauri 能立即进入事件循环并绘制前端。
            let engine = state::LazyResource::pending();
            let dictionary = state::LazyResource::pending();
            let dictionary_background = state::LazyResource::pending();
            let analysis_cache = state::LazyResource::pending();
            app.manage(AppState {
                engine: engine.clone(),
                dictionary: dictionary.clone(),
                dictionary_background: dictionary_background.clone(),
                sessions: state::RecoveringMutex::new(HashMap::new(), "document sessions"),
                analysis_cancellations: state::AnalysisCancellationRegistry::new(),
                next_session_id: AtomicU64::new(1),
                analysis_cache: analysis_cache.clone(),
                library,
            });

            // 词典和 SQLite 初始化可能持续数百毫秒到数秒，不能阻塞 WebView 首次绘制。
            let app_handle = app.handle().clone();
            std::thread::Builder::new()
                .name("kotoclip-backend-init".to_string())
                .spawn(move || {
                    // 主动避让 1.5 秒，让出 CPU 调度资源优先保障 WebView2 的建立与首屏渲染
                    std::thread::sleep(std::time::Duration::from_millis(1500));

                    let start_time = std::time::SystemTime::now();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let paths = paths::AppPaths::resolve(&app_handle)
                            .map_err(|error| error.to_string())?;

                        let dictionary_value = DictionaryService::new_from_dictionary_sources(
                            &paths.dictionary_source_dir,
                            &paths.dictionary_dir,
                            &paths.profile_db,
                        )
                        .map_err(|error| error.to_string())?;
                        dictionary.initialize(Ok(dictionary_value));

                        let background_dictionary_value = DictionaryService::open_existing(
                            &paths.dictionary_dir,
                            &paths.profile_db,
                        )
                        .map_err(|error| error.to_string())?;
                        dictionary_background.initialize(Ok(background_dictionary_value));

                        let engine_value = Engine::new(
                            &paths.system_dictionary,
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

                        if let Ok(elapsed) = start_time.elapsed() {
                            println!(
                                "[开发日志] 后台分析引擎与词典就绪，耗时: {}ms",
                                elapsed.as_millis()
                            );
                        }
                        Ok::<(), String>(())
                    }))
                    .unwrap_or_else(|payload| {
                        Err(format!(
                            "后台初始化任务异常结束：{}",
                            panic_payload_message(payload.as_ref())
                        ))
                    });

                    if let Err(error) = result {
                        engine.initialize(Err(error.clone()));
                        dictionary.initialize(Err(error.clone()));
                        dictionary_background.initialize(Err(error.clone()));
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
        .invoke_handler(tauri::generate_handler![
            commands::log_ui_timestamps,
            commands::import_epub_document,
            commands::list_library_books,
            commands::open_library_book,
            commands::update_library_progress,
            commands::update_library_book_organization,
            commands::reset_library_book_progress,
            commands::remove_library_book,
            commands::get_library_location,
            commands::open_document,
            commands::backend_status,
            commands::cancel_document_analysis,
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
            commands::get_dictionary_settings,
            commands::set_dictionary_order,
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
