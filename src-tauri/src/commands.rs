use crate::state::AppState;
use kotoclip_core::analysis_progress::AnalysisProgress;
use kotoclip_core::models::{
    AnnotatedToken, DictionaryLookup, ExportEntry, ExpressionRule, SegmentationCandidate,
};
use serde::Serialize;
use tauri::{Emitter, State, Window};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisProgressEvent {
    request_id: String,
    #[serde(flatten)]
    progress: AnalysisProgress,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResponse {
    pub tokens: Vec<AnnotatedToken>,
    pub backend_duration_ms: u64,
}

/// IPC 命令：分析日语文本并进行生词等级判定
#[tauri::command]
pub async fn analyze_text(
    window: Window,
    state: State<'_, AppState>,
    text: String,
    record_exposure: Option<bool>,
    request_id: Option<String>,
) -> Result<AnalysisResponse, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    let request_id = request_id.unwrap_or_else(|| "legacy".to_string());
    let started = std::time::Instant::now();
    let tokens = engine
        .analyze_text_with_progress(&text, record_exposure.unwrap_or(true), |progress| {
            let _ = window.emit(
                "analysis-progress",
                AnalysisProgressEvent {
                    request_id: request_id.clone(),
                    progress,
                },
            );
        })
        .map_err(|e| e.to_string())?;
    let backend_duration_ms = started.elapsed().as_millis() as u64;
    Ok(AnalysisResponse {
        tokens,
        backend_duration_ms,
    })
}

/// IPC 命令：查词，并按照多词词典优先级重排序
#[tauri::command]
pub async fn lookup_word(
    state: State<'_, AppState>,
    word: String,
    reading: Option<String>,
    priority_list: Vec<String>,
) -> Result<DictionaryLookup, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    Ok(engine.lookup_word(&word, reading.as_deref(), &priority_list))
}

#[tauri::command]
pub async fn choose_dictionary_target(
    state: State<'_, AppState>,
    query: String,
    reading: Option<String>,
    target: String,
) -> Result<(), String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine
        .choose_dictionary_target(&query, reading.as_deref(), &target)
        .map_err(|e| e.to_string())
}

/// IPC 命令：主动标记单词为“已知”
#[tauri::command]
pub async fn mark_known(
    state: State<'_, AppState>,
    base_form: String,
    reading: String,
) -> Result<(), String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine
        .mark_known(&base_form, &reading)
        .map_err(|e| e.to_string())
}

/// IPC 命令：主动标记单词为“未知”
#[tauri::command]
pub async fn mark_unknown(
    state: State<'_, AppState>,
    base_form: String,
    reading: String,
) -> Result<(), String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine
        .mark_unknown(&base_form, &reading)
        .map_err(|e| e.to_string())
}

/// IPC 命令：手动合并相邻胶囊分词并注册至本地自定义数据库中
#[tauri::command]
pub async fn add_merge_rule(state: State<'_, AppState>, parts: Vec<String>) -> Result<(), String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.add_merge_rule(&parts).map_err(|e| e.to_string())
}

/// 保存、列出和删除跨文节表达。表达只作为注解应用，不合并底层 token。
#[tauri::command]
pub async fn add_expression_rule(
    state: State<'_, AppState>,
    tokens: Vec<AnnotatedToken>,
    label: Option<String>,
    description: Option<String>,
    bunsetsu_states: Vec<String>,
    morpheme_masks: Vec<Vec<bool>>,
    gap_after: Option<usize>,
    expression_type: String,
    priority: i32,
    boundary_effect: String,
) -> Result<ExpressionRule, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine
        .add_configured_expression_rule(
            &tokens,
            label.as_deref(),
            description.as_deref(),
            &bunsetsu_states,
            &morpheme_masks,
            gap_after,
            &expression_type,
            priority,
            &boundary_effect,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_expression_rules(
    state: State<'_, AppState>,
) -> Result<Vec<ExpressionRule>, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.get_expression_rules().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_expression_annotations(
    state: State<'_, AppState>,
    tokens: Vec<AnnotatedToken>,
) -> Result<Vec<AnnotatedToken>, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine
        .refresh_expression_annotations(tokens)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_expression_rule(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.delete_expression_rule(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn split_token(
    state: State<'_, AppState>,
    token: AnnotatedToken,
) -> Result<Vec<AnnotatedToken>, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    Ok(engine.split_token(&token))
}

#[tauri::command]
pub async fn get_candidates(
    state: State<'_, AppState>,
    token: AnnotatedToken,
    top_n: usize,
) -> Result<Vec<SegmentationCandidate>, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    Ok(engine.get_candidates(&token, top_n))
}

#[tauri::command]
pub async fn choose_segmentation(
    state: State<'_, AppState>,
    source: AnnotatedToken,
    candidate: SegmentationCandidate,
) -> Result<(), String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine
        .choose_segmentation(&source, &candidate)
        .map_err(|e| e.to_string())
}

/// IPC 命令：打包所选生词生成 Anki 格式的导出 JSON 字符串
#[tauri::command]
pub async fn export_selected(
    source_text: String,
    selected_entries: Vec<ExportEntry>,
) -> Result<String, String> {
    kotoclip_core::export::json::export_to_json(&source_text, selected_entries)
        .map_err(|e| e.to_string())
}
