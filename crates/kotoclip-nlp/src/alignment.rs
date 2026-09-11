//! 将外部 tokenizer 的范围映射到 UniDic token，保留词典来源差异。
use crate::model::MorphemeToken;
use crate::syntax::{SyntaxArtifact, SyntaxSpan};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.provider-token-alignment.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentStatus { Exact, Compound, Partial, Unmatched }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenAlignment {
    pub provider: String,
    pub provider_token_id: String,
    pub provider_char_range: [usize; 2],
    pub provider_surface: Option<String>,
    pub status: AlignmentStatus,
    pub morpheme_indices: Vec<usize>,
    pub morpheme_ids: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenAlignmentArtifact {
    pub schema: String,
    pub source_id: Option<String>,
    pub unidic_provider: String,
    pub external_provider: String,
    pub alignments: Vec<TokenAlignment>,
}

pub fn align_artifact(artifact: &SyntaxArtifact, morphemes: &[MorphemeToken]) -> TokenAlignmentArtifact {
    let alignments = artifact.spans.iter()
        .filter(|span| span.kind == "token")
        .map(|span| align_span(&artifact.provider.id, span, morphemes))
        .collect();
    TokenAlignmentArtifact {
        schema: SCHEMA.into(), source_id: artifact.segment_id.clone(),
        unidic_provider: "unidic".into(), external_provider: artifact.provider.id.clone(), alignments,
    }
}

fn align_span(provider: &str, span: &SyntaxSpan, morphemes: &[MorphemeToken]) -> TokenAlignment {
    let [start, end] = span.char_range;
    let indices: Vec<usize> = morphemes.iter().enumerate()
        .filter(|(_, token)| token.char_range[0] >= start && token.char_range[1] <= end)
        .map(|(index, _)| index).collect();
    let overlaps: Vec<usize> = morphemes.iter().enumerate()
        .filter(|(_, token)| token.char_range[0] < end && start < token.char_range[1])
        .map(|(index, _)| index).collect();
    let covered_exact = indices.iter().any(|index| morphemes[*index].char_range == span.char_range);
    let covered_start = indices.first().map(|index| morphemes[*index].char_range[0]) == Some(start);
    let covered_end = indices.last().map(|index| morphemes[*index].char_range[1]) == Some(end);
    let (status, reason) = if covered_exact {
        (AlignmentStatus::Exact, "one_unidic_token")
    } else if !indices.is_empty() && covered_start && covered_end {
        (AlignmentStatus::Compound, "contiguous_unidic_tokens")
    } else if !overlaps.is_empty() {
        (AlignmentStatus::Partial, "external_range_splits_unidic_token")
    } else {
        (AlignmentStatus::Unmatched, "no_unidic_token_in_range")
    };
    TokenAlignment {
        provider: provider.into(), provider_token_id: span.id.clone(), provider_char_range: span.char_range,
        provider_surface: span.surface.clone(), status, morpheme_indices: indices.clone(),
        morpheme_ids: indices.iter().map(|index| morphemes[*index].id.clone()).collect(), reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{SyntaxProviderDescriptor, SyntaxSpan};
    fn token(id: &str, range: [usize; 2]) -> MorphemeToken {
        MorphemeToken { id: id.into(), source_index: 0, surface: "甲".into(), char_range: range,
            pos: [None, None, None, None], lemma: None, reading: None, query_forms: Vec::new() }
    }
    fn artifact(range: [usize; 2]) -> SyntaxArtifact {
        SyntaxArtifact { schema: crate::syntax::SCHEMA.into(), segment_id: Some("s".into()),
            provider: SyntaxProviderDescriptor { id: "ginza".into(), version: None, capabilities: vec!["token".into()], license: None },
            text_characters: 4, spans: vec![SyntaxSpan { id: "t0".into(), kind: "token".into(), char_range: range,
                head_char_range: None, source_id: "s".into(), surface: None, labels: Vec::new() }] }
    }
    #[test]
    fn classifies_exact_and_compound_ranges() {
        let exact = align_artifact(&artifact([0, 2]), &[token("m0", [0, 2])]);
        assert_eq!(exact.alignments[0].status, AlignmentStatus::Exact);
        let compound = align_artifact(&artifact([0, 4]), &[token("m0", [0, 2]), token("m1", [2, 4])]);
        assert_eq!(compound.alignments[0].status, AlignmentStatus::Compound);
        assert_eq!(compound.alignments[0].morpheme_ids, vec!["m0", "m1"]);
    }
    #[test]
    fn rejects_partial_external_token() {
        let result = align_artifact(&artifact([1, 3]), &[token("m0", [0, 2]), token("m1", [2, 4])]);
        assert_eq!(result.alignments[0].status, AlignmentStatus::Partial);
    }
}
