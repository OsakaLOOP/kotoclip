use super::client::StructuredCompletionRequest;
use crate::models::{DictEntry, DictionaryLookup};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

pub const DICTIONARY_DISAMBIGUATION_SCHEMA_VERSION: &str = "kotoclip.dictionary-disambiguation.v1";
pub const DICTIONARY_DECISION_SCHEMA_JSON: &str =
    include_str!("../../resources/llm_dictionary_decision.schema.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationBudget {
    pub max_candidates: usize,
    pub max_context_chars: usize,
    pub max_definition_chars_per_candidate: usize,
    pub max_total_definition_chars: usize,
    pub max_linked_candidates_per_target: usize,
    pub max_linked_content_chars_per_candidate: usize,
    pub max_total_linked_content_chars: usize,
}

impl Default for DisambiguationBudget {
    fn default() -> Self {
        Self {
            max_candidates: 32,
            max_context_chars: 800,
            max_definition_chars_per_candidate: 1_600,
            max_total_definition_chars: 24_000,
            max_linked_candidates_per_target: 4,
            max_linked_content_chars_per_candidate: 1_000,
            max_total_linked_content_chars: 16_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryContextEvidence {
    pub sentence: String,
    pub before: String,
    pub target: String,
    pub after: String,
    pub document_title: Option<String>,
    pub paragraph_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryAnalysisEvidence {
    pub surface: String,
    pub base_form: String,
    pub reading: Option<String>,
    pub pos: Vec<String>,
    pub author_ruby_reading: Option<String>,
    pub nbest_readings: Vec<String>,
    pub deterministic_preferred_entry_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryDisambiguationCandidate {
    pub candidate_id: String,
    pub ordinal: usize,
    pub dict_name: String,
    pub headword: String,
    pub reading: Option<String>,
    pub match_type: String,
    pub deterministic_preferred: bool,
    pub content_markdown: String,
    pub content_truncated: bool,
    pub navigation_targets: Vec<DictionaryNavigationTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryNavigationTarget {
    pub relation: String,
    pub label: String,
    pub target: String,
    pub resolution: String,
    pub resolved_candidates: Vec<DictionaryLinkedCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryLinkedCandidate {
    pub candidate_id: String,
    pub dict_name: String,
    pub headword: String,
    pub reading: Option<String>,
    pub content_markdown: String,
    pub content_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryDisambiguationRequest {
    pub schema_version: String,
    pub request_id: String,
    pub task: String,
    pub context: DictionaryContextEvidence,
    pub analysis: DictionaryAnalysisEvidence,
    pub candidates: Vec<DictionaryDisambiguationCandidate>,
    pub candidate_set_truncated: bool,
    pub omitted_candidate_count: usize,
    pub instructions: DisambiguationInstructions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationInstructions {
    pub use_only_supplied_evidence: bool,
    pub allow_abstention: bool,
    pub require_verbatim_quotes: bool,
    pub selection_is_advisory: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryDecisionStatus {
    Selected,
    Ambiguous,
    InsufficientEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryDisambiguationDecision {
    pub schema_version: String,
    pub request_id: String,
    pub status: DictionaryDecisionStatus,
    pub selected_candidate_id: Option<String>,
    pub ranked_candidate_ids: Vec<String>,
    pub confidence: f32,
    pub needs_user_review: bool,
    pub summary: String,
    pub ambiguity_reason: Option<String>,
    pub evidence: Vec<DictionaryDecisionEvidence>,
    pub navigation_recommendation: Option<DictionaryNavigationRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryDecisionEvidence {
    pub candidate_id: String,
    pub context_quote: String,
    pub definition_quote: String,
    pub supports: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryNavigationRecommendation {
    pub from_candidate_id: String,
    pub relation: String,
    pub target: String,
    pub target_candidate_id: String,
    pub context_quote: String,
    pub target_content_quote: String,
    pub reason: String,
}

pub fn build_disambiguation_request(
    request_id: impl Into<String>,
    lookup: &DictionaryLookup,
    resolved_navigation: &HashMap<String, Vec<DictEntry>>,
    mut context: DictionaryContextEvidence,
    analysis: DictionaryAnalysisEvidence,
    budget: &DisambiguationBudget,
) -> DictionaryDisambiguationRequest {
    context.sentence = truncate_chars(&context.sentence, budget.max_context_chars).0;
    context.before = truncate_chars(&context.before, budget.max_context_chars / 2).0;
    context.after = truncate_chars(&context.after, budget.max_context_chars / 2).0;
    let candidate_count = lookup.entries.len();
    let mut remaining_definition_chars = budget.max_total_definition_chars;
    let mut remaining_linked_chars = budget.max_total_linked_content_chars;
    let candidates = lookup
        .entries
        .iter()
        .take(budget.max_candidates)
        .enumerate()
        .map(|(index, entry)| {
            let per_candidate_limit = budget
                .max_definition_chars_per_candidate
                .min(remaining_definition_chars);
            let markdown = entry_markdown(entry);
            let (content_markdown, content_truncated) =
                truncate_chars(&markdown, per_candidate_limit);
            remaining_definition_chars =
                remaining_definition_chars.saturating_sub(content_markdown.chars().count());
            DictionaryDisambiguationCandidate {
                candidate_id: entry.entry_key.clone(),
                ordinal: index + 1,
                dict_name: entry.dict_name.clone(),
                headword: entry.headword.clone(),
                reading: entry.reading.clone(),
                match_type: entry.match_type.clone(),
                deterministic_preferred: entry.is_preferred,
                content_markdown,
                content_truncated,
                navigation_targets: build_navigation_targets(
                    entry,
                    resolved_navigation,
                    budget,
                    &mut remaining_linked_chars,
                ),
            }
        })
        .collect::<Vec<_>>();
    let material_truncated = candidates.iter().any(|candidate| {
        candidate.content_truncated
            || candidate.navigation_targets.iter().any(|target| {
                target
                    .resolved_candidates
                    .iter()
                    .any(|resolved| resolved.content_truncated)
            })
    });

    DictionaryDisambiguationRequest {
        schema_version: DICTIONARY_DISAMBIGUATION_SCHEMA_VERSION.to_string(),
        request_id: request_id.into(),
        task: "select_dictionary_definition".to_string(),
        context,
        analysis,
        candidates,
        candidate_set_truncated: candidate_count > budget.max_candidates || material_truncated,
        omitted_candidate_count: candidate_count.saturating_sub(budget.max_candidates),
        instructions: DisambiguationInstructions {
            use_only_supplied_evidence: true,
            allow_abstention: true,
            require_verbatim_quotes: true,
            selection_is_advisory: true,
        },
    }
}

pub fn build_disambiguation_prompt(
    request: &DictionaryDisambiguationRequest,
) -> Result<StructuredCompletionRequest, serde_json::Error> {
    let response_schema: Value = serde_json::from_str(DICTIONARY_DECISION_SCHEMA_JSON)?;
    let user_payload = serde_json::to_string_pretty(request)?;
    Ok(StructuredCompletionRequest {
        task_id: request.request_id.clone(),
        system_prompt: [
            "你是日语词典消歧器，不是百科问答助手。",
            "只能使用请求中的 context、analysis、candidates 及已解析 navigation_targets 的 Markdown 内容。",
            "candidate_id 是不透明标识；只能选择输入中存在的 ID，禁止创建词条、读音、释义或链接。",
            "每条证据必须逐字引用 context 与对应 candidate 的 content_markdown；本地程序会验证子串。",
            "不得根据连接名称猜测目标含义。resolution 不是 resolved 或没有 resolved_candidates 时，禁止推荐该跳转。",
            "证据不足、多个词义同样合理、候选被截断或需要输入外知识时，返回 ambiguous 或 insufficient_evidence。",
            "deterministic_preferred 只是本地规则证据，不是必须服从的答案。",
            "输出必须严格符合 JSON Schema，不得包含 Markdown 或额外文本。",
        ]
        .join("\n"),
        user_prompt: format!(
            "请对以下只读候选集进行消歧。结果仅供 UI 标记，不会自动保存或跳转。\n{user_payload}"
        ),
        response_schema_name: "kotoclip_dictionary_disambiguation".to_string(),
        response_schema,
        max_output_tokens: 1_500,
    })
}

pub fn validate_disambiguation_decision(
    request: &DictionaryDisambiguationRequest,
    decision: &DictionaryDisambiguationDecision,
) -> Result<(), DisambiguationValidationError> {
    if decision.schema_version != DICTIONARY_DISAMBIGUATION_SCHEMA_VERSION {
        return Err(DisambiguationValidationError::SchemaVersion);
    }
    if decision.request_id != request.request_id {
        return Err(DisambiguationValidationError::RequestId);
    }
    if !(0.0..=1.0).contains(&decision.confidence) {
        return Err(DisambiguationValidationError::Confidence);
    }

    let candidates: HashMap<_, _> = request
        .candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.as_str(), candidate))
        .collect();
    let selected = decision.selected_candidate_id.as_deref();
    if decision.status == DictionaryDecisionStatus::Selected && selected.is_none() {
        return Err(DisambiguationValidationError::MissingSelection);
    }
    if decision.status != DictionaryDecisionStatus::Selected && selected.is_some() {
        return Err(DisambiguationValidationError::UnexpectedSelection);
    }
    if selected.is_some_and(|candidate_id| !candidates.contains_key(candidate_id)) {
        return Err(DisambiguationValidationError::UnknownCandidate);
    }

    let mut ranked = HashSet::new();
    for candidate_id in &decision.ranked_candidate_ids {
        if !candidates.contains_key(candidate_id.as_str()) {
            return Err(DisambiguationValidationError::UnknownCandidate);
        }
        if !ranked.insert(candidate_id) {
            return Err(DisambiguationValidationError::DuplicateRanking);
        }
    }

    let context_text = format!(
        "{}\n{}\n{}\n{}",
        request.context.sentence,
        request.context.before,
        request.context.target,
        request.context.after
    );
    let mut selected_has_evidence = false;
    for evidence in &decision.evidence {
        let candidate = candidates
            .get(evidence.candidate_id.as_str())
            .ok_or(DisambiguationValidationError::UnknownCandidate)?;
        if evidence.context_quote.is_empty() || !context_text.contains(&evidence.context_quote) {
            return Err(DisambiguationValidationError::UngroundedContextQuote);
        }
        if evidence.definition_quote.is_empty()
            || !candidate
                .content_markdown
                .contains(&evidence.definition_quote)
        {
            return Err(DisambiguationValidationError::UngroundedDefinitionQuote);
        }
        if selected == Some(evidence.candidate_id.as_str()) {
            selected_has_evidence = true;
        }
    }
    if selected.is_some() && !selected_has_evidence {
        return Err(DisambiguationValidationError::MissingSelectedEvidence);
    }

    if let Some(navigation) = &decision.navigation_recommendation {
        let candidate = candidates
            .get(navigation.from_candidate_id.as_str())
            .ok_or(DisambiguationValidationError::UnknownCandidate)?;
        let target = candidate
            .navigation_targets
            .iter()
            .find(|target| {
                target.target == navigation.target && target.relation == navigation.relation
            })
            .ok_or(DisambiguationValidationError::UnknownNavigationTarget)?;
        let resolved = target
            .resolved_candidates
            .iter()
            .find(|resolved| resolved.candidate_id == navigation.target_candidate_id)
            .ok_or(DisambiguationValidationError::UnresolvedNavigationTarget)?;
        if navigation.context_quote.is_empty() || !context_text.contains(&navigation.context_quote)
        {
            return Err(DisambiguationValidationError::UngroundedContextQuote);
        }
        if navigation.target_content_quote.is_empty()
            || !resolved
                .content_markdown
                .contains(&navigation.target_content_quote)
        {
            return Err(DisambiguationValidationError::UngroundedNavigationQuote);
        }
    }

    if request.candidate_set_truncated
        && decision.status == DictionaryDecisionStatus::Selected
        && !decision.needs_user_review
    {
        return Err(DisambiguationValidationError::TruncatedWithoutReview);
    }
    Ok(())
}

fn build_navigation_targets(
    entry: &DictEntry,
    resolved_navigation: &HashMap<String, Vec<DictEntry>>,
    budget: &DisambiguationBudget,
    remaining_chars: &mut usize,
) -> Vec<DictionaryNavigationTarget> {
    entry
        .links
        .iter()
        .map(|link| {
            let resolved_entries = resolved_navigation.get(&link.target);
            let resolved_candidates = resolved_entries
                .into_iter()
                .flatten()
                .take(budget.max_linked_candidates_per_target)
                .map(|resolved| {
                    let limit = budget
                        .max_linked_content_chars_per_candidate
                        .min(*remaining_chars);
                    let (content_markdown, content_truncated) =
                        truncate_chars(&entry_markdown(resolved), limit);
                    *remaining_chars =
                        (*remaining_chars).saturating_sub(content_markdown.chars().count());
                    DictionaryLinkedCandidate {
                        candidate_id: resolved.entry_key.clone(),
                        dict_name: resolved.dict_name.clone(),
                        headword: resolved.headword.clone(),
                        reading: resolved.reading.clone(),
                        content_markdown,
                        content_truncated,
                    }
                })
                .collect::<Vec<_>>();
            DictionaryNavigationTarget {
                relation: link.relation.clone(),
                label: link.label.clone(),
                target: link.target.clone(),
                resolution: if resolved_candidates.is_empty() {
                    "unresolved".to_string()
                } else {
                    "resolved".to_string()
                },
                resolved_candidates,
            }
        })
        .collect()
}

fn entry_markdown(entry: &DictEntry) -> String {
    let html = entry
        .content_blocks
        .iter()
        .map(|block| block.html.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let breaks = Regex::new(r"(?i)<br\s*/?>|</p>|</div>|</li>").expect("固定换行正则必须有效");
    let list_items = Regex::new(r"(?i)<li[^>]*>").expect("固定列表正则必须有效");
    let tags = Regex::new(r"<[^>]+>").expect("固定 HTML 标签正则必须有效");
    let whitespace = Regex::new(r"\s+").expect("固定空白正则必须有效");
    let lines = breaks.replace_all(&html, "\n");
    let lists = list_items.replace_all(&lines, "\n- ");
    let plain = tags.replace_all(&lists, "");
    let decoded = plain
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    decoded
        .lines()
        .map(|line| whitespace.replace_all(line, " ").trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn truncate_chars(value: &str, limit: usize) -> (String, bool) {
    let count = value.chars().count();
    if count <= limit {
        return (value.to_string(), false);
    }
    if limit == 0 {
        return (String::new(), true);
    }
    let mut truncated: String = value.chars().take(limit.saturating_sub(1)).collect();
    truncated.push('…');
    (truncated, true)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DisambiguationValidationError {
    #[error("schema_version 不匹配")]
    SchemaVersion,
    #[error("request_id 不匹配")]
    RequestId,
    #[error("confidence 必须在 0..=1")]
    Confidence,
    #[error("selected 状态缺少 selected_candidate_id")]
    MissingSelection,
    #[error("非 selected 状态不得包含 selected_candidate_id")]
    UnexpectedSelection,
    #[error("响应引用了不存在的 candidate_id")]
    UnknownCandidate,
    #[error("ranked_candidate_ids 包含重复项")]
    DuplicateRanking,
    #[error("context_quote 不是输入上下文的逐字子串")]
    UngroundedContextQuote,
    #[error("definition_quote 不是候选释义的逐字子串")]
    UngroundedDefinitionQuote,
    #[error("选中候选缺少可核验的上下文与释义证据")]
    MissingSelectedEvidence,
    #[error("导航建议不在候选已知 navigation_targets 中")]
    UnknownNavigationTarget,
    #[error("导航建议引用的目标没有提供已解析候选内容")]
    UnresolvedNavigationTarget,
    #[error("target_content_quote 不是已解析目标 Markdown 的逐字子串")]
    UngroundedNavigationQuote,
    #[error("候选集被截断时，选中结果必须要求人工复核")]
    TruncatedWithoutReview,
}
