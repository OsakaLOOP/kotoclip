use crate::state::AppState;
use kotoclip_core::analysis_progress::AnalysisProgress;
use kotoclip_core::document::{
    propagate_stage_invalidation, AnalysisPatch, AnalysisStage, DocumentSession,
};
use kotoclip_core::models::{
    AnnotatedToken, DictionaryLookup, ExportEntry, ExpressionRule, SegmentationCandidate,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
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
pub struct DocumentResponse {
    pub patch: AnalysisPatch,
    pub backend_duration_ms: u64,
    pub cache_hit: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendStatus {
    pub ready: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrammarConceptBundle {
    pub concept: kotoclip_core::pipeline::grammar::catalog::GrammarConcept,
    pub senses: Vec<kotoclip_core::pipeline::grammar::catalog::GrammarSense>,
    pub explanation: kotoclip_core::pipeline::grammar::catalog::GrammarExplanationDocument,
    pub explanations: Vec<kotoclip_core::pipeline::grammar::catalog::GrammarExplanationDocument>,
}

#[tauri::command]
pub fn search_grammar_catalog(
    query: Option<String>,
    family: Option<String>,
    jlpt_level: Option<u8>,
    audit_status: Option<String>,
    source_ref: Option<String>,
) -> Result<Vec<kotoclip_core::pipeline::grammar::catalog::GrammarConcept>, String> {
    let catalog =
        kotoclip_core::pipeline::grammar::catalog::GrammarCatalog::load_embedded()
            .map_err(|error| error.to_string())?;
    Ok(catalog
        .search(
            query.as_deref(),
            family.as_deref(),
            jlpt_level,
            audit_status.as_deref(),
            source_ref.as_deref(),
        )
        .into_iter()
        .cloned()
        .collect())
}

#[tauri::command]
pub fn get_grammar_concept(concept_id: String) -> Result<GrammarConceptBundle, String> {
    let catalog =
        kotoclip_core::pipeline::grammar::catalog::GrammarCatalog::load_embedded()
            .map_err(|error| error.to_string())?;
    let concept = catalog
        .concept(&concept_id)
        .cloned()
        .ok_or_else(|| format!("未知语法 concept：{concept_id}"))?;
    let explanation = catalog
        .explanation(&concept.default_explanation_id)
        .cloned()
        .ok_or_else(|| format!("语法 concept 缺少讲解：{}", concept.concept_id))?;
    let senses = catalog
        .senses_for(&concept.concept_id)
        .into_iter()
        .cloned()
        .collect();
    let explanations = catalog
        .explanations_for(&concept.concept_id)
        .into_iter()
        .cloned()
        .collect();
    Ok(GrammarConceptBundle {
        concept,
        senses,
        explanation,
        explanations,
    })
}

#[tauri::command]
pub async fn backend_status(state: State<'_, AppState>) -> Result<BackendStatus, String> {
    Ok(BackendStatus {
        ready: state.engine.status()? && state.analysis_cache.status()?,
    })
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentMutation {
    ReplaceText {
        text: String,
        #[serde(default)]
        record_exposure: bool,
    },
}

#[tauri::command]
pub async fn open_document(
    window: Window,
    state: State<'_, AppState>,
    text: String,
    record_exposure: Option<bool>,
    request_id: Option<String>,
) -> Result<DocumentResponse, String> {
    let request_id = request_id.unwrap_or_else(|| "open-document".to_string());
    let started = std::time::Instant::now();
    let sequence = state.next_session_id.fetch_add(1, Ordering::Relaxed);
    let session_id = format!("document-{sequence}");
    let cached = state
        .analysis_cache
        .lock()
        .map_err(|error| error.to_string())?
        .load(&text);
    if let Some(stable_tokens) = cached {
        let engine = state.engine.lock().map_err(|error| error.to_string())?;
        let mut session = DocumentSession::new_progressive(
            session_id.clone(),
            text,
            record_exposure.unwrap_or(true),
        );
        session.set_cached_stable_tokens(stable_tokens);
        let batch = session
            .next_batch(2_000)
            .ok_or_else(|| "缓存文档没有可恢复内容".to_string())?;
        let stable_batch = session
            .take_cached_stable_tokens(&batch)
            .ok_or_else(|| "缓存缺少首批稳定 Token".to_string())?;
        let tokens = engine
            .hydrate_stable_tokens_for_document_batch(stable_batch)
            .map_err(|error| error.to_string())?;
        let patch = session
            .append_analyzed_batch(0, &batch, tokens)
            .map_err(|error| error.to_string())?;
        state
            .sessions
            .lock()
            .map_err(|error| error.to_string())?
            .insert(session_id, session);
        return Ok(DocumentResponse {
            patch,
            backend_duration_ms: started.elapsed().as_millis() as u64,
            cache_hit: true,
        });
    }
    let mut session =
        DocumentSession::new_progressive(session_id.clone(), text, record_exposure.unwrap_or(true));
    let batch = session
        .next_batch(2_000)
        .ok_or_else(|| "文档没有可分析内容".to_string())?;
    let engine = state.engine.lock().map_err(|error| error.to_string())?;
    let tokens = engine
        .analyze_document_batch_with_progress(
            &batch.source,
            session.document_readings(),
            |progress| {
                let _ = window.emit(
                    "analysis-progress",
                    AnalysisProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            },
        )
        .map_err(|error| error.to_string())?;
    let patch = session
        .append_analyzed_batch(0, &batch, tokens)
        .map_err(|error| error.to_string())?;
    if session.should_record_exposure() {
        engine
            .record_document_exposures(&session.tokens)
            .map_err(|error| error.to_string())?;
        session.mark_exposure_recorded();
    }
    drop(engine);
    state
        .sessions
        .lock()
        .map_err(|error| error.to_string())?
        .insert(session_id, session);
    Ok(DocumentResponse {
        patch,
        backend_duration_ms: started.elapsed().as_millis() as u64,
        cache_hit: false,
    })
}

#[tauri::command]
pub async fn continue_document_analysis(
    state: State<'_, AppState>,
    session_id: String,
    base_revision: u64,
    target_characters: Option<usize>,
) -> Result<Option<AnalysisPatch>, String> {
    let engine = state.engine.lock().map_err(|error| error.to_string())?;
    let mut sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("文档会话不存在：{session_id}"))?;
    session
        .require_revision(base_revision)
        .map_err(|error| error.to_string())?;
    let Some(batch) = session.next_batch(target_characters.unwrap_or(8_000)) else {
        return Ok(None);
    };
    let tokens = if let Some(stable) = session.take_cached_stable_tokens(&batch) {
        engine
            .hydrate_stable_tokens_for_document_batch(stable)
            .map_err(|error| error.to_string())?
    } else {
        engine
            .analyze_document_batch(&batch.source, session.document_readings())
            .map_err(|error| error.to_string())?
    };
    let patch = session
        .append_analyzed_batch(base_revision, &batch, tokens)
        .map_err(|error| error.to_string())?;
    if session.should_record_exposure() {
        engine
            .record_document_exposures(&session.tokens)
            .map_err(|error| error.to_string())?;
        session.mark_exposure_recorded();
    }
    Ok(Some(patch))
}

#[tauri::command]
pub async fn request_document_range(
    state: State<'_, AppState>,
    session_id: String,
    base_revision: u64,
    char_range: (usize, usize),
) -> Result<AnalysisPatch, String> {
    let engine = state.engine.lock().map_err(|error| error.to_string())?;
    let mut sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("文档会话不存在：{session_id}"))?;
    if let Some(batch) = session.batch_for_range(char_range, 4_000) {
        let tokens = if let Some(stable) = session.take_cached_stable_tokens(&batch) {
            engine
                .hydrate_stable_tokens_for_document_batch(stable)
                .map_err(|error| error.to_string())?
        } else {
            engine
                .analyze_document_batch(&batch.source, session.document_readings())
                .map_err(|error| error.to_string())?
        };
        let patch = session
            .append_analyzed_batch(base_revision, &batch, tokens)
            .map_err(|error| error.to_string())?;
        if session.should_record_exposure() {
            engine
                .record_document_exposures(&session.tokens)
                .map_err(|error| error.to_string())?;
            session.mark_exposure_recorded();
        }
        Ok(patch)
    } else {
        session
            .range_patch(base_revision, char_range)
            .map_err(|error| error.to_string())
    }
}

#[tauri::command]
pub async fn apply_document_mutation(
    window: Window,
    state: State<'_, AppState>,
    session_id: String,
    base_revision: u64,
    mutation: DocumentMutation,
    request_id: Option<String>,
) -> Result<DocumentResponse, String> {
    {
        let sessions = state.sessions.lock().map_err(|error| error.to_string())?;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| format!("文档会话不存在：{session_id}"))?;
        session
            .require_revision(base_revision)
            .map_err(|error| error.to_string())?;
    }
    let request_id = request_id.unwrap_or_else(|| "document-mutation".to_string());
    let started = std::time::Instant::now();
    let (text, record_exposure) = match mutation {
        DocumentMutation::ReplaceText {
            text,
            record_exposure,
        } => (text, record_exposure),
    };
    let tokens = {
        let engine = state.engine.lock().map_err(|error| error.to_string())?;
        engine
            .analyze_text_with_progress(&text, record_exposure, |progress| {
                let _ = window.emit(
                    "analysis-progress",
                    AnalysisProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            })
            .map_err(|error| error.to_string())?
    };
    let patch = state
        .sessions
        .lock()
        .map_err(|error| error.to_string())?
        .get_mut(&session_id)
        .ok_or_else(|| format!("文档会话不存在：{session_id}"))?
        .replace_all(base_revision, text, tokens)
        .map_err(|error| error.to_string())?;
    Ok(DocumentResponse {
        patch,
        backend_duration_ms: started.elapsed().as_millis() as u64,
        cache_hit: false,
    })
}

#[tauri::command]
pub async fn finalize_document(
    state: State<'_, AppState>,
    session_id: String,
    base_revision: u64,
) -> Result<bool, String> {
    let engine = state.engine.lock().map_err(|error| error.to_string())?;
    let mut sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("文档会话不存在：{session_id}"))?;
    session
        .require_revision(base_revision)
        .map_err(|error| error.to_string())?;
    if session.should_record_exposure() {
        engine
            .record_document_exposures(&session.tokens)
            .map_err(|error| error.to_string())?;
        session.mark_exposure_recorded();
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
pub async fn persist_document_cache(
    state: State<'_, AppState>,
    session_id: String,
    base_revision: u64,
) -> Result<bool, String> {
    let engine = state.engine.lock().map_err(|error| error.to_string())?;
    let sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| format!("文档会话不存在：{session_id}"))?;
    session
        .require_revision(base_revision)
        .map_err(|error| error.to_string())?;
    if !session.is_complete() {
        return Ok(false);
    }
    let stable_tokens = engine.analyze_stable_text(&session.source);
    state
        .analysis_cache
        .lock()
        .map_err(|error| error.to_string())?
        .store(&session.source, &stable_tokens)
        .map_err(|error| error.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn close_document(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<bool, String> {
    Ok(state
        .sessions
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&session_id)
        .is_some())
}

#[tauri::command]
pub async fn refresh_document_expressions(
    state: State<'_, AppState>,
    session_id: String,
    base_revision: u64,
) -> Result<AnalysisPatch, String> {
    let engine = state.engine.lock().map_err(|error| error.to_string())?;
    let mut sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("文档会话不存在：{session_id}"))?;
    session
        .require_revision(base_revision)
        .map_err(|error| error.to_string())?;
    let document_range = session.char_range();
    let changed_indices = engine
        .refresh_expression_annotations_changed(&mut session.tokens)
        .map_err(|error| error.to_string())?;
    session
        .apply_token_mutation(
            base_revision,
            "expression_rules_changed",
            propagate_stage_invalidation(AnalysisStage::Expression, vec![document_range]),
            |_| changed_indices,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn mark_document_known(
    state: State<'_, AppState>,
    session_id: String,
    base_revision: u64,
    base_form: String,
    reading: String,
    known: bool,
) -> Result<AnalysisPatch, String> {
    let engine = state.engine.lock().map_err(|error| error.to_string())?;
    let mut sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("文档会话不存在：{session_id}"))?;
    session
        .require_revision(base_revision)
        .map_err(|error| error.to_string())?;
    if known {
        engine
            .mark_known(&base_form, &reading)
            .map_err(|error| error.to_string())?;
    } else {
        engine
            .mark_unknown(&base_form, &reading)
            .map_err(|error| error.to_string())?;
    }
    let ranges: Vec<_> = session
        .tokens
        .iter()
        .filter(|token| {
            token.display_class == "content"
                && token.bunsetsu.head_word.base_form == base_form
                && token.bunsetsu.head_word.reading == reading
        })
        .map(|token| token.bunsetsu.char_range)
        .collect();
    let changed_indices = engine
        .refresh_profile_annotations_for_key(&mut session.tokens, &base_form, &reading)
        .map_err(|error| error.to_string())?;
    session
        .apply_token_mutation(
            base_revision,
            if known { "mark_known" } else { "mark_unknown" },
            propagate_stage_invalidation(AnalysisStage::Profile, ranges),
            |_| changed_indices,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn choose_document_segmentation(
    state: State<'_, AppState>,
    session_id: String,
    base_revision: u64,
    source: AnnotatedToken,
    candidate: SegmentationCandidate,
) -> Result<AnalysisPatch, String> {
    let engine = state.engine.lock().map_err(|error| error.to_string())?;
    let mut sessions = state.sessions.lock().map_err(|error| error.to_string())?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("文档会话不存在：{session_id}"))?;
    session
        .require_revision(base_revision)
        .map_err(|error| error.to_string())?;
    engine
        .choose_segmentation(&source, &candidate)
        .map_err(|error| error.to_string())?;
    let local_range = source.bunsetsu.char_range;
    let document_range = session.char_range();
    let changed_indices = engine
        .refresh_segmentation_for_range(&mut session.tokens, local_range)
        .map_err(|error| error.to_string())?;
    let mut invalidation =
        propagate_stage_invalidation(AnalysisStage::WordFormation, vec![local_range]);
    for item in &mut invalidation {
        if matches!(
            item.stage,
            AnalysisStage::Expression | AnalysisStage::Presentation
        ) {
            item.char_ranges = vec![document_range];
        }
    }
    session
        .apply_token_mutation(base_revision, "segmentation_choice", invalidation, |_| {
            changed_indices
        })
        .map_err(|error| error.to_string())
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
pub async fn preview_expression_rule(
    state: State<'_, AppState>,
    tokens: Vec<AnnotatedToken>,
    bunsetsu_states: Vec<String>,
    morpheme_masks: Vec<Vec<bool>>,
    gap_after: Option<usize>,
    expression_type: String,
    boundary_effect: String,
) -> Result<kotoclip_core::models::ExpressionRulePreview, String> {
    let engine = state.engine.lock().map_err(|error| error.to_string())?;
    Ok(engine.preview_configured_expression_rule(
        &tokens,
        &bunsetsu_states,
        &morpheme_masks,
        gap_after,
        &expression_type,
        &boundary_effect,
    ))
}

#[tauri::command]
pub async fn get_expression_rules(
    state: State<'_, AppState>,
) -> Result<Vec<ExpressionRule>, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.get_expression_rules().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_expression_rule(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.delete_expression_rule(id).map_err(|e| e.to_string())
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
