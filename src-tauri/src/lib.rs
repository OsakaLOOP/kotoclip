mod reader_engine;

use kotoclip_core::analysis::{Request, ResourcePaths, Response};
use kotoclip_core::reader_state::{SavedSelection, WordState};
use reader_engine::ReaderEngine;
use std::{
    path::PathBuf,
    sync::Arc,
};
use tauri::{Manager, State};

struct AppState {
    reader: Arc<ReaderEngine>,
    cancellation: Arc<std::sync::atomic::AtomicU64>,
}

#[tauri::command]
fn cancel_external(state: State<'_, AppState>) {
    state.cancellation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
fn search_grammar_catalog(
    query: Option<String>, family: Option<String>, jlpt_level: Option<u8>,
    audit_status: Option<String>, source_ref: Option<String>,
) -> Result<Vec<kotoclip_core::grammar_catalog::GrammarConcept>, String> {
    kotoclip_core::grammar_catalog::search(query.as_deref(), family.as_deref(), jlpt_level, audit_status.as_deref(), source_ref.as_deref())
}

#[tauri::command]
fn get_grammar_concept(concept_id: String) -> Result<kotoclip_core::grammar_catalog::GrammarConceptBundle, String> {
    kotoclip_core::grammar_catalog::get(&concept_id)
}

#[tauri::command]
async fn nlp_request(state: State<'_, AppState>, request: Request) -> Result<Response, String> {
    if matches!(request, Request::CancelExternal) {
        cancel_external(state);
        return Ok(Response { result: Some(serde_json::json!({"cancelled": true})), error: None });
    }
    let service = state.reader.analysis.clone();
    let generation = state.cancellation.load(std::sync::atomic::Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        Ok(service.dispatch_at(request, generation))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn reader_library(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"books": state.reader.books()?, "path": state.reader.library_path()}))
}

#[tauri::command]
async fn reader_import(state: State<'_, AppState>, path: String) -> Result<kotoclip_core::library::LibraryBook, String> {
    let reader = state.reader.clone();
    tauri::async_runtime::spawn_blocking(move || reader.import(&path)).await.map_err(|error| error.to_string())?
}

#[tauri::command]
async fn reader_open_book(state: State<'_, AppState>, id: String) -> Result<kotoclip_core::library::LibraryBook, String> {
    let reader = state.reader.clone();
    tauri::async_runtime::spawn_blocking(move || reader.open_book(&id)).await.map_err(|error| error.to_string())?
}

#[tauri::command]
async fn reader_open_text(state: State<'_, AppState>, text: String) -> Result<serde_json::Value, String> {
    let reader = state.reader.clone();
    tauri::async_runtime::spawn_blocking(move || reader.open_text(text)).await.map_err(|error| error.to_string())?
}

#[tauri::command]
fn reader_close(state: State<'_, AppState>) { state.reader.close(); }

#[tauri::command]
fn reader_progress(state: State<'_, AppState>, id: String, offset: usize, total: usize, chapter: Option<String>, seconds: u64) -> Result<kotoclip_core::library::LibraryBookSummary, String> {
    state.reader.progress(&id, offset, total, chapter.as_deref(), seconds)
}

#[tauri::command]
fn reader_organize(state: State<'_, AppState>, id: String, color: Option<String>, tags: Vec<String>) -> Result<kotoclip_core::library::LibraryBookSummary, String> {
    state.reader.organize(&id, color.as_deref(), &tags)
}

#[tauri::command]
fn reader_reset(state: State<'_, AppState>, id: String) -> Result<kotoclip_core::library::LibraryBookSummary, String> { state.reader.reset(&id) }

#[tauri::command]
fn reader_remove(state: State<'_, AppState>, id: String) -> Result<bool, String> { state.reader.remove(&id) }

#[tauri::command]
fn reader_word(state: State<'_, AppState>, base: String, reading: String) -> Result<WordState, String> { state.reader.word(&base, &reading) }

#[tauri::command]
fn reader_mark(state: State<'_, AppState>, base: String, reading: String, known: bool) -> Result<WordState, String> { state.reader.mark(&base, &reading, known) }

#[tauri::command]
fn reader_expose(state: State<'_, AppState>, base: String, reading: String) -> Result<(), String> { state.reader.expose(&base, &reading) }

#[tauri::command]
fn reader_selections(state: State<'_, AppState>, book: String, version: String) -> Result<Vec<SavedSelection>, String> { state.reader.selections(&book, &version) }

#[tauri::command]
fn reader_save_selection(state: State<'_, AppState>, selection: SavedSelection) -> Result<(), String> { state.reader.save_selection(&selection) }

#[tauri::command]
fn reader_delete_selection(state: State<'_, AppState>, book: String, version: String, start: usize, end: usize) -> Result<(), String> {
    state.reader.delete_selection(&book, &version, start, end)
}

#[tauri::command]
fn reader_clear_selections(state: State<'_, AppState>, book: String, version: String) -> Result<(), String> { state.reader.clear_selections(&book, &version) }

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
            let paths = if cfg!(debug_assertions) {
                ResourcePaths::development(&root)
            } else {
                let resources = app.path().resource_dir()?;
                let portable = std::env::current_exe()?
                    .parent()
                    .ok_or("程序目录无效")?
                    .to_path_buf();
                let data = std::env::var_os("KOTOCLIP_DATA_DIR")
                    .map(PathBuf::from)
                    .unwrap_or(app.path().app_data_dir()?);
                let dictionary = |name: &str| {
                    let override_name = format!("KOTOCLIP_UNIDIC_{}", name.to_uppercase());
                    std::env::var_os(override_name)
                        .map(PathBuf::from)
                        .unwrap_or_else(|| {
                            let local = portable.join("nlp").join(format!("{name}.dic"));
                            if local.is_file() {
                                local
                            } else {
                                resources
                                    .join("_up_/experiments/unidic-source")
                                    .join(format!("unidic-{name}-202512.vibrato.dic"))
                            }
                        })
                };
                let sources = [
                    data.join("dict-sources"),
                    portable.join("dict-sources"),
                    resources.join("_up_/data/dict-sources"),
                ]
                .into_iter()
                .find(|p| p.is_dir())
                .unwrap_or_else(|| data.join("dict-sources"));
                ResourcePaths {
                    cwj: dictionary("cwj"),
                    csj: dictionary("csj"),
                    dictionary_sources: sources,
                    dictionaries: data.join("dicts"),
                    provider_config: data.join("providers.local.json"),
                    provider_script: resources.join("_up_/scripts/nlp_provider.py"),
                    provider_defaults: kotoclip_core::providers::ProviderSettings::development(&portable),
                }
            };
            let data = std::env::var_os("KOTOCLIP_DATA_DIR").map(PathBuf::from).unwrap_or(app.path().app_data_dir()?);
            let library = app.path().document_dir()?.join("Kotoclip Library");
            let reader = Arc::new(ReaderEngine::new(paths, library, data).map_err(std::io::Error::other)?);
            let background_reader = Arc::clone(&reader);
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = background_reader.backfill_resource_dimensions() {
                    eprintln!("资源尺寸回填失败：{error}");
                }
            });
            let cancellation = reader.analysis.cancellation();
            app.manage(AppState { reader, cancellation });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![nlp_request, search_grammar_catalog, get_grammar_concept, cancel_external,
            reader_library, reader_import, reader_open_book, reader_open_text, reader_close, reader_progress,
            reader_organize, reader_reset, reader_remove, reader_word, reader_mark, reader_expose,
            reader_selections, reader_save_selection, reader_delete_selection, reader_clear_selections])
        .run(tauri::generate_context!())
        .expect("桌面应用启动失败");
}
