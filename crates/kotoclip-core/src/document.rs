use crate::models::AnnotatedToken;
use crate::pipeline::ruby;
use crate::transport::{CompactAnalysisPatch, CompactEncoder};
use serde::Serialize;
use std::collections::HashSet;

pub const DOCUMENT_SESSION_SCHEMA_VERSION: u32 = 1;
pub const PIPELINE_ARTIFACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineFingerprint {
    pub session_schema_version: u32,
    pub pipeline_artifact_version: u32,
}

impl Default for PipelineFingerprint {
    fn default() -> Self {
        Self {
            session_schema_version: DOCUMENT_SESSION_SCHEMA_VERSION,
            pipeline_artifact_version: PIPELINE_ARTIFACT_VERSION,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PatchKind {
    FullReplace,
    RangeReplace,
    TokenUpdate,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStage {
    Morpheme,
    WordFormation,
    DictionaryLexical,
    Bunsetsu,
    Grammar,
    Profile,
    Expression,
    Presentation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidationReport {
    pub reason: String,
    pub stages: Vec<AnalysisStage>,
    pub stage_ranges: Vec<StageInvalidation>,
    pub char_ranges: Vec<(usize, usize)>,
    pub recomputed_characters: usize,
    pub total_characters: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageInvalidation {
    pub stage: AnalysisStage,
    pub char_ranges: Vec<(usize, usize)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisPatch {
    pub session_id: String,
    pub base_revision: u64,
    pub revision: u64,
    pub kind: PatchKind,
    pub char_range: (usize, usize),
    pub removed_token_ids: Vec<String>,
    pub token_ids: Vec<String>,
    pub ordered_token_ids: Vec<String>,
    pub analysis: CompactAnalysisPatch,
    pub fingerprint: PipelineFingerprint,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalidation: Option<InvalidationReport>,
    pub document_char_range: (usize, usize),
    pub available_ranges: Vec<(usize, usize)>,
    pub complete: bool,
}

#[derive(Debug, Clone)]
pub struct DocumentChunk {
    pub source: String,
    pub char_range: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct DocumentBatch {
    pub source: String,
    pub char_range: (usize, usize),
    pub chunk_start: usize,
    pub chunk_end: usize,
}

pub struct DocumentSession {
    pub session_id: String,
    pub revision: u64,
    pub source: String,
    pub tokens: Vec<AnnotatedToken>,
    token_ids: Vec<String>,
    fingerprint: PipelineFingerprint,
    encoder: CompactEncoder,
    chunks: Vec<DocumentChunk>,
    analyzed_chunks: Vec<bool>,
    document_char_range: (usize, usize),
    record_exposure_on_complete: bool,
    exposure_recorded: bool,
    cached_stable_tokens: Vec<AnnotatedToken>,
}

impl DocumentSession {
    pub fn new(session_id: String, source: String, mut tokens: Vec<AnnotatedToken>) -> Self {
        stabilize_expression_ids(&mut tokens);
        let token_ids = stable_token_ids(&tokens);
        let document_char_range = document_char_range(&tokens);
        Self {
            session_id,
            revision: 1,
            source,
            tokens,
            token_ids,
            fingerprint: PipelineFingerprint::default(),
            encoder: CompactEncoder::default(),
            chunks: Vec::new(),
            analyzed_chunks: Vec::new(),
            document_char_range,
            record_exposure_on_complete: false,
            exposure_recorded: false,
            cached_stable_tokens: Vec::new(),
        }
    }

    pub fn new_progressive(
        session_id: String,
        source: String,
        record_exposure_on_complete: bool,
    ) -> Self {
        let chunks = split_document_chunks(&source);
        let chunk_count = chunks.len();
        let document_end = chunks.last().map_or(0, |chunk| chunk.char_range.1);
        Self {
            session_id,
            revision: 0,
            source,
            tokens: Vec::new(),
            token_ids: Vec::new(),
            fingerprint: PipelineFingerprint::default(),
            encoder: CompactEncoder::default(),
            chunks,
            analyzed_chunks: vec![false; chunk_count],
            document_char_range: (0, document_end),
            record_exposure_on_complete,
            exposure_recorded: false,
            cached_stable_tokens: Vec::new(),
        }
    }

    pub fn next_batch(&self, target_characters: usize) -> Option<DocumentBatch> {
        let chunk_start = self.analyzed_chunks.iter().position(|analyzed| !analyzed)?;
        self.batch_from_chunk(chunk_start, target_characters)
    }

    pub fn set_cached_stable_tokens(&mut self, tokens: Vec<AnnotatedToken>) {
        self.cached_stable_tokens = tokens;
    }

    pub fn take_cached_stable_tokens(
        &mut self,
        batch: &DocumentBatch,
    ) -> Option<Vec<AnnotatedToken>> {
        if self.cached_stable_tokens.is_empty() {
            return None;
        }
        let mut selected = Vec::new();
        self.cached_stable_tokens.retain(|token| {
            if ranges_intersect(token.bunsetsu.char_range, batch.char_range) {
                selected.push(token.clone());
                false
            } else {
                true
            }
        });
        if selected.is_empty() {
            return None;
        }
        localize_tokens(&mut selected, batch.char_range.0);
        Some(selected)
    }

    pub fn batch_for_range(
        &self,
        char_range: (usize, usize),
        target_characters: usize,
    ) -> Option<DocumentBatch> {
        let chunk_start = self
            .chunks
            .iter()
            .enumerate()
            .find(|(index, chunk)| {
                !self.analyzed_chunks[*index] && ranges_intersect(chunk.char_range, char_range)
            })?
            .0;
        self.batch_from_chunk(chunk_start, target_characters)
    }

    fn batch_from_chunk(
        &self,
        chunk_start: usize,
        target_characters: usize,
    ) -> Option<DocumentBatch> {
        let first = self.chunks.get(chunk_start)?;
        let mut source = String::new();
        let mut chunk_end = chunk_start;
        let mut end = first.char_range.0;
        while let Some(chunk) = self.chunks.get(chunk_end) {
            if self.analyzed_chunks[chunk_end] {
                break;
            }
            source.push_str(&chunk.source);
            end = chunk.char_range.1;
            chunk_end += 1;
            if end.saturating_sub(first.char_range.0) >= target_characters.max(1) {
                break;
            }
        }
        Some(DocumentBatch {
            source,
            char_range: (first.char_range.0, end),
            chunk_start,
            chunk_end,
        })
    }

    pub fn append_analyzed_batch(
        &mut self,
        base_revision: u64,
        batch: &DocumentBatch,
        mut tokens: Vec<AnnotatedToken>,
    ) -> Result<AnalysisPatch, SessionRevisionError> {
        self.require_revision(base_revision)?;
        let token_offset = self
            .tokens
            .iter()
            .filter(|token| token.bunsetsu.char_range.0 < batch.char_range.0)
            .count();
        let out_of_order = self
            .tokens
            .iter()
            .any(|token| token.bunsetsu.char_range.0 > batch.char_range.0);
        offset_tokens(&mut tokens, batch.char_range.0, token_offset);
        stabilize_expression_ids(&mut tokens);
        let token_ids = stable_token_ids(&tokens);
        self.tokens.extend(tokens.iter().cloned());
        self.tokens.sort_by_key(|token| token.bunsetsu.char_range.0);
        reindex_expression_token_ranges(&mut self.tokens);
        self.token_ids = stable_token_ids(&self.tokens);
        self.analyzed_chunks[batch.chunk_start..batch.chunk_end].fill(true);
        let (patch_tokens, patch_token_ids) = if out_of_order {
            (self.tokens.clone(), self.token_ids.clone())
        } else {
            (tokens, token_ids)
        };
        self.revision += 1;
        Ok(AnalysisPatch {
            session_id: self.session_id.clone(),
            base_revision,
            revision: self.revision,
            kind: if base_revision == 0 {
                PatchKind::FullReplace
            } else {
                PatchKind::RangeReplace
            },
            char_range: batch.char_range,
            removed_token_ids: Vec::new(),
            token_ids: patch_token_ids,
            ordered_token_ids: self.token_ids.clone(),
            analysis: self.encoder.encode_patch(&patch_tokens),
            fingerprint: self.fingerprint.clone(),
            invalidation: None,
            document_char_range: self.document_char_range,
            available_ranges: self.available_ranges(),
            complete: self.is_complete(),
        })
    }

    pub fn full_patch(&mut self, base_revision: u64) -> AnalysisPatch {
        AnalysisPatch {
            session_id: self.session_id.clone(),
            base_revision,
            revision: self.revision,
            kind: PatchKind::FullReplace,
            char_range: document_char_range(&self.tokens),
            removed_token_ids: Vec::new(),
            token_ids: self.token_ids.clone(),
            ordered_token_ids: self.token_ids.clone(),
            analysis: self.encoder.encode_patch(&self.tokens),
            fingerprint: self.fingerprint.clone(),
            invalidation: None,
            document_char_range: self.document_char_range,
            available_ranges: self.available_ranges(),
            complete: self.is_complete(),
        }
    }

    pub fn range_patch(
        &mut self,
        base_revision: u64,
        char_range: (usize, usize),
    ) -> Result<AnalysisPatch, SessionRevisionError> {
        self.require_revision(base_revision)?;
        let mut token_ids = Vec::new();
        let mut tokens = Vec::new();
        for (token_id, token) in self.token_ids.iter().zip(&self.tokens) {
            if ranges_intersect(token.bunsetsu.char_range, char_range) {
                token_ids.push(token_id.clone());
                tokens.push(token.clone());
            }
        }
        Ok(AnalysisPatch {
            session_id: self.session_id.clone(),
            base_revision,
            revision: self.revision,
            kind: PatchKind::RangeReplace,
            char_range,
            removed_token_ids: Vec::new(),
            token_ids,
            ordered_token_ids: self.token_ids.clone(),
            analysis: self.encoder.encode_patch(&tokens),
            fingerprint: self.fingerprint.clone(),
            invalidation: None,
            document_char_range: self.document_char_range,
            available_ranges: self.available_ranges(),
            complete: self.is_complete(),
        })
    }

    pub fn replace_all(
        &mut self,
        base_revision: u64,
        source: String,
        mut tokens: Vec<AnnotatedToken>,
    ) -> Result<AnalysisPatch, SessionRevisionError> {
        self.require_revision(base_revision)?;
        stabilize_expression_ids(&mut tokens);
        let previous_ids: HashSet<_> = self.token_ids.iter().cloned().collect();
        let next_ids = stable_token_ids(&tokens);
        let next_id_set: HashSet<_> = next_ids.iter().cloned().collect();
        let removed_token_ids = previous_ids.difference(&next_id_set).cloned().collect();
        self.source = source;
        self.tokens = tokens;
        self.token_ids = next_ids;
        self.chunks.clear();
        self.analyzed_chunks.clear();
        self.document_char_range = document_char_range(&self.tokens);
        self.revision += 1;
        Ok(AnalysisPatch {
            session_id: self.session_id.clone(),
            base_revision,
            revision: self.revision,
            kind: PatchKind::FullReplace,
            char_range: document_char_range(&self.tokens),
            removed_token_ids,
            token_ids: self.token_ids.clone(),
            ordered_token_ids: self.token_ids.clone(),
            analysis: self.encoder.encode_patch(&self.tokens),
            fingerprint: self.fingerprint.clone(),
            invalidation: None,
            document_char_range: self.document_char_range,
            available_ranges: self.available_ranges(),
            complete: self.is_complete(),
        })
    }

    pub fn apply_token_mutation<F>(
        &mut self,
        base_revision: u64,
        reason: impl Into<String>,
        stage_ranges: Vec<StageInvalidation>,
        mutate: F,
    ) -> Result<AnalysisPatch, SessionRevisionError>
    where
        F: FnOnce(&mut [AnnotatedToken]) -> Vec<usize>,
    {
        self.require_revision(base_revision)?;
        let mut changed_indices = mutate(&mut self.tokens);
        stabilize_expression_ids(&mut self.tokens);
        changed_indices.sort_unstable();
        changed_indices.dedup();
        changed_indices.retain(|index| *index < self.tokens.len());
        let changed_tokens: Vec<_> = changed_indices
            .iter()
            .map(|index| self.tokens[*index].clone())
            .collect();
        let changed_ids: Vec<_> = changed_indices
            .iter()
            .map(|index| self.token_ids[*index].clone())
            .collect();
        let char_ranges = merge_char_ranges(
            stage_ranges
                .iter()
                .flat_map(|item| item.char_ranges.iter().copied()),
        );
        let recomputed_characters = char_ranges.iter().map(|range| range.1 - range.0).sum();
        self.revision += 1;
        Ok(AnalysisPatch {
            session_id: self.session_id.clone(),
            base_revision,
            revision: self.revision,
            kind: PatchKind::TokenUpdate,
            char_range: enclosing_range(&char_ranges),
            removed_token_ids: Vec::new(),
            token_ids: changed_ids,
            // TokenUpdate 不改变结构和稳定 ID 顺序，避免每次领域 mutation 重发全文 ID。
            ordered_token_ids: Vec::new(),
            analysis: self.encoder.encode_patch(&changed_tokens),
            fingerprint: self.fingerprint.clone(),
            invalidation: Some(InvalidationReport {
                reason: reason.into(),
                stages: stage_ranges.iter().map(|item| item.stage).collect(),
                stage_ranges,
                char_ranges,
                recomputed_characters,
                total_characters: document_char_range(&self.tokens).1
                    - document_char_range(&self.tokens).0,
            }),
            document_char_range: self.document_char_range,
            available_ranges: self.available_ranges(),
            complete: self.is_complete(),
        })
    }

    pub fn require_revision(&self, base_revision: u64) -> Result<(), SessionRevisionError> {
        if self.revision == base_revision {
            Ok(())
        } else {
            Err(SessionRevisionError {
                expected: self.revision,
                received: base_revision,
            })
        }
    }

    pub fn char_range(&self) -> (usize, usize) {
        self.document_char_range
    }

    pub fn is_complete(&self) -> bool {
        self.analyzed_chunks.iter().all(|analyzed| *analyzed)
    }

    pub fn should_record_exposure(&self) -> bool {
        self.is_complete() && self.record_exposure_on_complete && !self.exposure_recorded
    }

    pub fn mark_exposure_recorded(&mut self) {
        self.exposure_recorded = true;
    }

    fn available_ranges(&self) -> Vec<(usize, usize)> {
        merge_char_ranges(
            self.chunks
                .iter()
                .zip(&self.analyzed_chunks)
                .filter(|(_, analyzed)| **analyzed)
                .map(|(chunk, _)| chunk.char_range),
        )
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("文档 revision 已过期：当前 {expected}，收到 {received}")]
pub struct SessionRevisionError {
    pub expected: u64,
    pub received: u64,
}

fn stable_token_ids(tokens: &[AnnotatedToken]) -> Vec<String> {
    let mut occurrence = std::collections::HashMap::new();
    tokens
        .iter()
        .map(|token| {
            let key = (
                token.bunsetsu.char_range,
                token.display_class.as_str(),
                token.bunsetsu.surface.as_str(),
            );
            let ordinal = occurrence.entry(key).or_insert(0_usize);
            let id = format!(
                "t:{}:{}:{}:{}",
                token.bunsetsu.char_range.0,
                token.bunsetsu.char_range.1,
                token.display_class,
                *ordinal
            );
            *ordinal += 1;
            id
        })
        .collect()
}

fn document_char_range(tokens: &[AnnotatedToken]) -> (usize, usize) {
    (
        tokens
            .first()
            .map_or(0, |token| token.bunsetsu.char_range.0),
        tokens.last().map_or(0, |token| token.bunsetsu.char_range.1),
    )
}

fn ranges_intersect(left: (usize, usize), right: (usize, usize)) -> bool {
    left.0 < right.1 && right.0 < left.1
}

fn split_document_chunks(source: &str) -> Vec<DocumentChunk> {
    let mut chunks = Vec::new();
    let mut char_offset = 0;
    for line in source.split_inclusive('\n') {
        let length = ruby::prepare_text(line).text.chars().count();
        chunks.push(DocumentChunk {
            source: line.to_string(),
            char_range: (char_offset, char_offset + length),
        });
        char_offset += length;
    }
    if source.is_empty() {
        return chunks;
    }
    if !source.ends_with('\n') && chunks.is_empty() {
        let length = ruby::prepare_text(source).text.chars().count();
        chunks.push(DocumentChunk {
            source: source.to_string(),
            char_range: (0, length),
        });
    }
    chunks
}

fn offset_tokens(tokens: &mut [AnnotatedToken], char_offset: usize, token_offset: usize) {
    let offset_range = |range: &mut (usize, usize)| {
        range.0 += char_offset;
        range.1 += char_offset;
    };
    for token in tokens {
        offset_range(&mut token.bunsetsu.char_range);
        for morpheme in &mut token.bunsetsu.morphemes {
            offset_range(&mut morpheme.char_range);
        }
        for tag in &mut token.bunsetsu.grammar_tags {
            offset_range(&mut tag.char_range);
        }
        for formation in &mut token.bunsetsu.word_formations {
            offset_range(&mut formation.char_range);
            for capture in &mut formation.captures {
                offset_range(&mut capture.char_range);
            }
        }
        for lexical in &mut token.bunsetsu.lexical_units {
            offset_range(&mut lexical.char_range);
        }
        for expression in &mut token.expressions {
            offset_range(&mut expression.char_range);
            for range in &mut expression.matched_ranges {
                offset_range(range);
            }
            expression.token_range.0 += token_offset;
            expression.token_range.1 += token_offset;
        }
    }
}

fn localize_tokens(tokens: &mut [AnnotatedToken], char_offset: usize) {
    let localize_range = |range: &mut (usize, usize)| {
        range.0 -= char_offset;
        range.1 -= char_offset;
    };
    for token in tokens {
        localize_range(&mut token.bunsetsu.char_range);
        for morpheme in &mut token.bunsetsu.morphemes {
            localize_range(&mut morpheme.char_range);
        }
        for tag in &mut token.bunsetsu.grammar_tags {
            localize_range(&mut tag.char_range);
        }
        for formation in &mut token.bunsetsu.word_formations {
            localize_range(&mut formation.char_range);
            for capture in &mut formation.captures {
                localize_range(&mut capture.char_range);
            }
        }
        for lexical in &mut token.bunsetsu.lexical_units {
            localize_range(&mut lexical.char_range);
        }
    }
}

fn stabilize_expression_ids(tokens: &mut [AnnotatedToken]) {
    crate::pipeline::expressions::stabilize_expression_ids(tokens);
}

fn reindex_expression_token_ranges(tokens: &mut [AnnotatedToken]) {
    let ranges: Vec<_> = tokens
        .iter()
        .map(|token| token.bunsetsu.char_range)
        .collect();
    for token in tokens {
        for expression in &mut token.expressions {
            let first = ranges
                .iter()
                .position(|range| ranges_intersect(*range, expression.char_range));
            let last = ranges
                .iter()
                .rposition(|range| ranges_intersect(*range, expression.char_range));
            if let (Some(first), Some(last)) = (first, last) {
                expression.token_range = (first, last + 1);
            }
        }
    }
}

fn merge_char_ranges(ranges: impl IntoIterator<Item = (usize, usize)>) -> Vec<(usize, usize)> {
    let mut ranges: Vec<_> = ranges.into_iter().collect();
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for range in ranges {
        if let Some(previous) = merged.last_mut().filter(|previous| range.0 <= previous.1) {
            previous.1 = previous.1.max(range.1);
        } else {
            merged.push(range);
        }
    }
    merged
}

fn enclosing_range(ranges: &[(usize, usize)]) -> (usize, usize) {
    (
        ranges.first().map_or(0, |range| range.0),
        ranges.last().map_or(0, |range| range.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Bunsetsu, HeadWord, Morpheme, PosTag};

    fn token(surface: &str, range: (usize, usize)) -> AnnotatedToken {
        let pos = PosTag {
            major: "名詞".to_string(),
            sub1: "一般".to_string(),
            sub2: "*".to_string(),
            sub3: "*".to_string(),
        };
        AnnotatedToken {
            bunsetsu: Bunsetsu {
                morphemes: vec![Morpheme {
                    surface: surface.to_string(),
                    pos: pos.clone(),
                    base_form: surface.to_string(),
                    reading: String::new(),
                    conjugation_type: "*".to_string(),
                    conjugation_form: "*".to_string(),
                    char_range: range,
                }],
                surface: surface.to_string(),
                head_word: HeadWord {
                    surface: surface.to_string(),
                    base_form: surface.to_string(),
                    reading: String::new(),
                    pos,
                },
                grammar_tags: Vec::new(),
                word_formations: Vec::new(),
                lexical_units: Vec::new(),
                function: None,
                char_range: range,
            },
            novelty_score: 1.0,
            is_selected: false,
            is_known: false,
            inference_reason: None,
            expressions: Vec::new(),
            display_class: "content".to_string(),
        }
    }

    #[test]
    fn rejects_stale_revision_and_keeps_ids_stable_for_annotation_changes() {
        let mut session = DocumentSession::new(
            "session-1".to_string(),
            "日本語".to_string(),
            vec![token("日本", (0, 2)), token("語", (2, 3))],
        );
        let original_ids = session.token_ids.clone();
        let mut updated = session.tokens.clone();
        updated[0].is_known = true;
        let patch = session
            .replace_all(1, "日本語".to_string(), updated)
            .expect("当前 revision 应接受更新");
        assert_eq!(patch.token_ids, original_ids);
        assert_eq!(patch.revision, 2);
        let error = session
            .replace_all(1, "旧结果".to_string(), Vec::new())
            .err()
            .expect("旧 revision 必须被拒绝");
        assert_eq!(
            error,
            SessionRevisionError {
                expected: 2,
                received: 1,
            }
        );
    }

    #[test]
    fn range_patch_only_contains_intersecting_tokens() {
        let mut session = DocumentSession::new(
            "session-1".to_string(),
            "日本語".to_string(),
            vec![token("日本", (0, 2)), token("語", (2, 3))],
        );
        let opened = session.full_patch(0);
        let string_count = opened.analysis.s.len();
        let patch = session.range_patch(1, (1, 2)).expect("revision 应匹配");
        assert_eq!(patch.kind, PatchKind::RangeReplace);
        assert_eq!(patch.token_ids.len(), 1);
        assert_eq!(patch.ordered_token_ids.len(), 2);
        assert_eq!(patch.analysis.b as usize, string_count);
        assert!(patch.analysis.s.is_empty(), "重复范围不应重发已有字符串");
    }

    #[test]
    fn token_mutation_reports_exact_invalidated_stages() {
        let mut session = DocumentSession::new(
            "session-1".to_string(),
            "日本語".to_string(),
            vec![token("日本", (0, 2)), token("語", (2, 3))],
        );
        session.full_patch(0);
        let patch = session
            .apply_token_mutation(
                1,
                "mark_known",
                vec![
                    StageInvalidation {
                        stage: AnalysisStage::Profile,
                        char_ranges: vec![(0, 2)],
                    },
                    StageInvalidation {
                        stage: AnalysisStage::Presentation,
                        char_ranges: vec![(0, 2)],
                    },
                ],
                |tokens| {
                    tokens[0].is_known = true;
                    vec![0]
                },
            )
            .expect("画像 mutation 应成功");
        assert_eq!(patch.kind, PatchKind::TokenUpdate);
        assert_eq!(patch.token_ids.len(), 1);
        assert!(
            patch.ordered_token_ids.is_empty(),
            "非结构 mutation 不应重发全文 Token 顺序"
        );
        let invalidation = patch.invalidation.expect("必须返回失效报告");
        assert_eq!(
            invalidation.stages,
            vec![AnalysisStage::Profile, AnalysisStage::Presentation]
        );
        assert_eq!(invalidation.recomputed_characters, 2);
        assert_eq!(invalidation.total_characters, 3);
    }

    #[test]
    fn exposure_is_deferred_until_progressive_document_is_complete() {
        let mut session = DocumentSession::new_progressive(
            "session-1".to_string(),
            "一行目。\n二行目。".to_string(),
            true,
        );
        assert!(!session.should_record_exposure());
        while let Some(batch) = session.next_batch(1) {
            session
                .append_analyzed_batch(session.revision, &batch, Vec::new())
                .expect("批次提交应成功");
        }
        assert!(session.should_record_exposure());
        session.mark_exposure_recorded();
        assert!(!session.should_record_exposure());
    }

    #[test]
    fn missing_range_can_be_analyzed_before_earlier_chunks() {
        let mut session = DocumentSession::new_progressive(
            "session-1".to_string(),
            "一\n二\n三".to_string(),
            false,
        );
        let last = session
            .batch_for_range((4, 5), 1)
            .expect("末尾范围应生成批次");
        session
            .append_analyzed_batch(0, &last, vec![token("三", (0, 1))])
            .expect("末尾批次应可先提交");
        assert_eq!(session.tokens[0].bunsetsu.char_range, (4, 5));
        let first = session.next_batch(1).expect("仍应存在前部批次");
        let patch = session
            .append_analyzed_batch(1, &first, vec![token("一", (0, 1))])
            .expect("前部批次应可后提交");
        assert_eq!(session.tokens[0].bunsetsu.char_range, (0, 1));
        assert_eq!(patch.token_ids.len(), session.tokens.len());
        assert_eq!(patch.available_ranges, vec![(0, 2), (4, 5)]);
    }
}
