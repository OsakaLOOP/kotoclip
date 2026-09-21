use kotoclip_core::analysis::{AnalysisService, Request, ResourcePaths, Response};
use std::{
    path::PathBuf,
    sync::Arc,
};
use tauri::{Manager, State};

struct AppState {
    service: Arc<AnalysisService>,
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
    let service = state.service.clone();
    let generation = state.cancellation.load(std::sync::atomic::Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        Ok(service.dispatch_at(request, generation))
    })
    .await
    .map_err(|e| e.to_string())?
}

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
            let service = AnalysisService::new(paths);
            let cancellation = service.cancellation();
            app.manage(AppState { service: Arc::new(service), cancellation });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![nlp_request, search_grammar_catalog, get_grammar_concept, cancel_external])
        .run(tauri::generate_context!())
        .expect("桌面应用启动失败");
}
