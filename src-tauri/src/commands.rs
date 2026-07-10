use tauri::State;
use crate::state::AppState;
use kotoclip_core::models::{AnnotatedToken, DictEntry, ExportEntry, SegmentationCandidate};

/// IPC 命令：分析日语文本并进行生词等级判定
#[tauri::command]
pub async fn analyze_text(
    state: State<'_, AppState>,
    text: String,
    record_exposure: Option<bool>,
) -> Result<Vec<AnnotatedToken>, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine
        .analyze_text_with_exposure(&text, record_exposure.unwrap_or(true))
        .map_err(|e| e.to_string())
}

/// IPC 命令：查词，并按照多词词典优先级重排序
#[tauri::command]
pub async fn lookup_word(
    state: State<'_, AppState>,
    word: String,
    reading: Option<String>,
    priority_list: Vec<String>,
) -> Result<Vec<DictEntry>, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    Ok(engine.lookup_word(&word, reading.as_deref(), &priority_list))
}

/// IPC 命令：主动标记单词为“已知”
#[tauri::command]
pub async fn mark_known(
    state: State<'_, AppState>,
    base_form: String,
    reading: String,
) -> Result<(), String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.mark_known(&base_form, &reading).map_err(|e| e.to_string())
}

/// IPC 命令：主动标记单词为“未知”
#[tauri::command]
pub async fn mark_unknown(
    state: State<'_, AppState>,
    base_form: String,
    reading: String,
) -> Result<(), String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.mark_unknown(&base_form, &reading).map_err(|e| e.to_string())
}

/// IPC 命令：手动合并相邻胶囊分词并注册至本地自定义数据库中
#[tauri::command]
pub async fn add_merge_rule(
    state: State<'_, AppState>,
    parts: Vec<String>,
) -> Result<(), String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.add_merge_rule(&parts).map_err(|e| e.to_string())
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

/// IPC 命令：打包所选生词生成 Anki 格式的导出 JSON 字符串
#[tauri::command]
pub async fn export_selected(
    source_text: String,
    selected_entries: Vec<ExportEntry>,
) -> Result<String, String> {
    kotoclip_core::export::json::export_to_json(&source_text, selected_entries).map_err(|e| e.to_string())
}
