//! L2 句子与小句证据层。
//! 该层只投影 StructureArtifact 的范围与来源，不推导新的边界。
use crate::{model::MorphemeToken, structure::{StructureArtifact, StructureStatus}};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SCHEMA: &str = "kotoclip.clause-artifact.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClauseStatus { Observed, Candidate, Pending }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SentenceNode {
    pub id: String,
    pub char_range: [usize; 2],
    pub status: ClauseStatus,
    pub provider: String,
    pub source_id: Option<String>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClauseNode {
    pub id: String,
    pub char_range: [usize; 2],
    pub sentence_ids: Vec<String>,
    pub morpheme_indices: Vec<usize>,
    pub status: ClauseStatus,
    pub provider: String,
    pub source_id: Option<String>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoundaryConflict {
    pub id: String,
    pub node_ids: Vec<String>,
    pub char_range: [usize; 2],
    pub providers: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClauseArtifact {
    pub schema: String,
    pub sentences: Vec<SentenceNode>,
    pub clauses: Vec<ClauseNode>,
    pub conflicts: Vec<BoundaryConflict>,
}

fn status(value: &StructureStatus) -> ClauseStatus {
    match value {
        StructureStatus::Observed => ClauseStatus::Observed,
        StructureStatus::Candidate => ClauseStatus::Candidate,
        StructureStatus::Pending => ClauseStatus::Pending,
    }
}

fn conflicts(nodes: &[(String, [usize; 2], String)], kind: &str) -> Vec<BoundaryConflict> {
    let mut result = Vec::new();
    for left in 0..nodes.len() {
        for right in (left + 1)..nodes.len() {
            let a = nodes[left].1;
            let b = nodes[right].1;
            if a[0] < b[1] && b[0] < a[1] && a != b {
                let mut ids = vec![nodes[left].0.clone(), nodes[right].0.clone()];
                ids.sort();
                let mut providers = BTreeSet::new();
                providers.insert(nodes[left].2.clone());
                providers.insert(nodes[right].2.clone());
                result.push(BoundaryConflict {
                    id: format!("{kind}-conflict-{}-{}", left, right),
                    node_ids: ids,
                    char_range: [a[0].max(b[0]), a[1].min(b[1])],
                    providers: providers.into_iter().collect(),
                    reason: format!("overlapping_{kind}_boundaries"),
                });
            }
        }
    }
    result
}

/// 将句子和小句结构映射到统一 token，保留 provider 边界冲突。
pub fn collect_clauses(text: &str, morphemes: &[MorphemeToken], structure: &StructureArtifact) -> Result<ClauseArtifact, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut sentences = Vec::new();
    for span in &structure.sentences {
        let [start, end] = span.char_range;
        if start >= end || end > chars.len() { return Err(format!("句子跨度 {} 超出文本范围", span.id)); }
        sentences.push(SentenceNode { id: format!("sentence:{}", span.id), char_range: [start, end], status: status(&span.status), provider: span.provider.clone(), source_id: span.source_id.clone(), labels: span.labels.clone() });
    }
    let mut clauses = Vec::new();
    for span in &structure.clauses {
        let [start, end] = span.char_range;
        if start >= end || end > chars.len() { return Err(format!("小句跨度 {} 超出文本范围", span.id)); }
        let surface: String = chars[start..end].iter().collect();
        if surface.trim().is_empty() { continue; }
        let coverage = crate::alignment::cover_range(span.char_range, morphemes, text);
        let morpheme_indices = coverage.morpheme_indices;
        let sentence_ids = sentences.iter().filter(|sentence| sentence.char_range[0] <= start && end <= sentence.char_range[1]).map(|sentence| sentence.id.clone()).collect();
        clauses.push(ClauseNode { id: format!("clause:{}", span.id), char_range: [start, end], sentence_ids, morpheme_indices, status: if coverage.complete { status(&span.status) } else { ClauseStatus::Pending }, provider: span.provider.clone(), source_id: span.source_id.clone(), labels: span.labels.iter().cloned().chain([format!("alignment:{}", coverage.reason)]).collect() });
    }
    let sentence_refs: Vec<_> = sentences.iter().map(|node| (node.id.clone(), node.char_range, node.provider.clone())).collect();
    let clause_refs: Vec<_> = clauses.iter().map(|node| (node.id.clone(), node.char_range, node.provider.clone())).collect();
    let mut boundary_conflicts = conflicts(&sentence_refs, "sentence");
    boundary_conflicts.extend(conflicts(&clause_refs, "clause"));
    Ok(ClauseArtifact { schema: SCHEMA.into(), sentences, clauses, conflicts: boundary_conflicts })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn token(index: usize, range: [usize; 2]) -> MorphemeToken { MorphemeToken { id: format!("m{index}"), source_index: index, surface: "甲".into(), char_range: range, pos: [None, None, None, None], lemma: None, reading: None, query_forms: Vec::new() } }
    fn span(id: &str, kind: &str, range: [usize; 2], provider: &str) -> crate::structure::StructureSpan { crate::structure::StructureSpan { id: id.into(), kind: kind.into(), char_range: range, status: StructureStatus::Observed, provider: provider.into(), source_id: None, head_char_range: None, labels: Vec::new() } }

    #[test]
    fn maps_clause_to_sentence_and_tokens() {
        let structure = StructureArtifact { schema: crate::structure::SCHEMA.into(), provider: "local".into(), provider_version: None, paragraphs: Vec::new(), sentences: vec![span("s", "sentence", [0, 4], "ginza")], clauses: vec![span("c", "clause", [0, 2], "ginza")], bunsetsu: Vec::new(), compounds: Vec::new() };
        let result = collect_clauses("甲乙甲乙", &[token(0, [0, 2]), token(1, [2, 4])], &structure).unwrap();
        assert_eq!(result.clauses[0].sentence_ids, vec!["sentence:s"]);
        assert_eq!(result.clauses[0].morpheme_indices, vec![0]);
    }

    #[test]
    fn records_sentence_boundary_conflict() {
        let structure = StructureArtifact { schema: crate::structure::SCHEMA.into(), provider: "local".into(), provider_version: None, paragraphs: Vec::new(), sentences: vec![span("g", "sentence", [0, 4], "ginza"), span("k", "sentence", [2, 6], "kwja")], clauses: Vec::new(), bunsetsu: Vec::new(), compounds: Vec::new() };
        let result = collect_clauses("甲乙甲乙甲乙", &[token(0, [0, 2]), token(1, [2, 4]), token(2, [4, 6])], &structure).unwrap();
        assert_eq!(result.conflicts[0].char_range, [2, 4]);
        assert_eq!(result.conflicts[0].providers, vec!["ginza", "kwja"]);
    }
}
