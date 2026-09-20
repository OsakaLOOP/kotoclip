//! 将外部 tokenizer 的范围映射到 UniDic token，保留词典来源差异。
use crate::model::MorphemeToken;
use crate::syntax::{SyntaxArtifact, SyntaxSpan};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.provider-token-alignment.v2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentStatus { Exact, Compound, Partial, Unmatched }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpanCoverage {
    pub morpheme_indices: Vec<usize>,
    pub intersections: Vec<[usize; 2]>,
    pub gaps: Vec<[usize; 2]>,
    pub complete: bool,
    pub reason: String,
}

/// 核对整个范围的词元覆盖；空白保留为 gap，词元内部切点保留为部分映射。
pub fn cover_range(range: [usize; 2], morphemes: &[MorphemeToken], text: &str) -> SpanCoverage {
    let [start, end] = range;
    let chars: Vec<char> = text.chars().collect();
    let indices: Vec<usize> = morphemes.iter().enumerate().filter(|(_, t)| t.char_range[0] < end && start < t.char_range[1]).map(|(i, _)| i).collect();
    let intersections: Vec<_> = indices.iter().map(|&i| [start.max(morphemes[i].char_range[0]), end.min(morphemes[i].char_range[1])]).collect();
    let mut cursor = start;
    let mut gaps = Vec::new();
    for &[a, b] in &intersections {
        if cursor < a { gaps.push([cursor, a]); }
        cursor = cursor.max(b);
    }
    if cursor < end { gaps.push([cursor, end]); }
    let boundaries = indices.iter().all(|&i| start <= morphemes[i].char_range[0] && morphemes[i].char_range[1] <= end);
    let whitespace = end <= chars.len() && gaps.iter().all(|&[a, b]| chars[a..b].iter().all(|c| c.is_whitespace()));
    let complete = !indices.is_empty() && boundaries && whitespace;
    let reason = if indices.is_empty() { "no_morpheme" } else if !boundaries { "partial_morpheme" } else if !whitespace { "uncovered_text" } else if !gaps.is_empty() { "covered_with_whitespace" } else { "complete_morphemes" };
    SpanCoverage { morpheme_indices: indices, intersections, gaps, complete, reason: reason.into() }
}

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
    pub groups: Vec<crate::alignment_group::AlignmentGroup>,
}

pub fn align_artifact(artifact: &SyntaxArtifact, morphemes: &[MorphemeToken]) -> TokenAlignmentArtifact {
    let alignments = artifact.spans.iter()
        .filter(|span| span.kind == "token")
        .map(|span| align_span(&artifact.provider.id, span, morphemes))
        .collect();
    TokenAlignmentArtifact {
        schema: SCHEMA.into(), source_id: artifact.segment_id.clone(),
        unidic_provider: "unidic".into(), external_provider: artifact.provider.id.clone(), alignments,
        groups: crate::alignment_group::from_syntax(artifact, morphemes),
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
    } else if !indices.is_empty() && covered_start && covered_end && indices.windows(2).all(|pair| morphemes[pair[0]].char_range[1] == morphemes[pair[1]].char_range[0]) {
        (AlignmentStatus::Compound, "contiguous_unidic_tokens")
    } else if !overlaps.is_empty() {
        (AlignmentStatus::Partial, if covered_start && covered_end { "unidic_gap" } else { "external_range_splits_unidic_token" })
    } else {
        (AlignmentStatus::Unmatched, "no_unidic_token_in_range")
    };
    TokenAlignment {
        provider: provider.into(), provider_token_id: span.id.clone(), provider_char_range: span.char_range,
        provider_surface: span.surface.clone(), status, morpheme_indices: overlaps.clone(),
        morpheme_ids: overlaps.iter().map(|index| morphemes[*index].id.clone()).collect(), reason: reason.into(),
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
            text_characters: 4, text_sha256: String::new(), spans: vec![SyntaxSpan { id: "t0".into(), kind: "token".into(), char_range: range,
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
        assert_eq!(result.alignments[0].morpheme_ids, vec!["m0", "m1"]);
    }

    #[test]
    fn distinguishes_whitespace_coverage_from_token_continuity() {
        let tokens = [token("m0", [0, 1]), token("m1", [2, 3])];
        let result = align_artifact(&artifact([0, 3]), &tokens);
        assert_eq!(result.alignments[0].status, AlignmentStatus::Partial);
        assert_eq!(result.alignments[0].reason, "unidic_gap");
        let coverage = cover_range([0, 3], &tokens, "甲 乙");
        assert!(coverage.complete);
        assert_eq!(coverage.gaps, vec![[1, 2]]);
        assert!(!cover_range([0, 3], &tokens, "甲丙乙").complete);
    }

    #[test]
    fn preserves_internal_cut_and_intersection() {
        let coverage = cover_range([0, 1], &[token("m0", [0, 2])], "時折");
        assert!(!coverage.complete);
        assert_eq!(coverage.morpheme_indices, vec![0]);
        assert_eq!(coverage.intersections, vec![[0, 1]]);
        assert_eq!(coverage.reason, "partial_morpheme");
    }
}
