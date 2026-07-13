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
}
