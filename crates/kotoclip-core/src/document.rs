use crate::models::AnnotatedToken;
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
}

pub struct DocumentSession {
    pub session_id: String,
    pub revision: u64,
    pub source: String,
    pub tokens: Vec<AnnotatedToken>,
    token_ids: Vec<String>,
    fingerprint: PipelineFingerprint,
    encoder: CompactEncoder,
}

impl DocumentSession {
    pub fn new(session_id: String, source: String, tokens: Vec<AnnotatedToken>) -> Self {
        let token_ids = stable_token_ids(&tokens);
        Self {
            session_id,
            revision: 1,
            source,
            tokens,
            token_ids,
            fingerprint: PipelineFingerprint::default(),
            encoder: CompactEncoder::default(),
        }
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
        })
    }

    pub fn replace_all(
        &mut self,
        base_revision: u64,
        source: String,
        tokens: Vec<AnnotatedToken>,
    ) -> Result<AnalysisPatch, SessionRevisionError> {
        self.require_revision(base_revision)?;
        let previous_ids: HashSet<_> = self.token_ids.iter().cloned().collect();
        let next_ids = stable_token_ids(&tokens);
        let next_id_set: HashSet<_> = next_ids.iter().cloned().collect();
        let removed_token_ids = previous_ids.difference(&next_id_set).cloned().collect();
        self.source = source;
        self.tokens = tokens;
        self.token_ids = next_ids;
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
            ordered_token_ids: self.token_ids.clone(),
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
        document_char_range(&self.tokens)
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
        let invalidation = patch.invalidation.expect("必须返回失效报告");
        assert_eq!(
            invalidation.stages,
            vec![AnalysisStage::Profile, AnalysisStage::Presentation]
        );
        assert_eq!(invalidation.recomputed_characters, 2);
        assert_eq!(invalidation.total_characters, 3);
    }
}
