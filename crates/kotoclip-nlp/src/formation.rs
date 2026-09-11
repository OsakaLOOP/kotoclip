//! L2 之前的构词证据层。
//! 该模块只组织 provider 已给出的 compound span，不根据词性推断新的语言规则。
use crate::{model::MorphemeToken, structure::{StructureArtifact, StructureStatus}};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SCHEMA: &str = "kotoclip.formation-artifact.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FormationStatus { Observed, Candidate, Pending }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormationEvidence {
    pub provider: String,
    pub source_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormationNode {
    pub id: String,
    pub kind: String,
    pub char_range: [usize; 2],
    pub morpheme_indices: Vec<usize>,
    pub status: FormationStatus,
    pub evidence: Vec<FormationEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormationConflict {
    pub id: String,
    pub node_ids: Vec<String>,
    pub char_range: [usize; 2],
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormationArtifact {
    pub schema: String,
    pub nodes: Vec<FormationNode>,
    pub conflicts: Vec<FormationConflict>,
}

fn status(value: &StructureStatus) -> FormationStatus {
    match value {
        StructureStatus::Observed => FormationStatus::Observed,
        StructureStatus::Candidate => FormationStatus::Candidate,
        StructureStatus::Pending => FormationStatus::Pending,
    }
}

/// 将结构层 compound 映射到 UniDic token，保留一对多和重叠关系。
pub fn collect_formations(
    text: &str,
    morphemes: &[MorphemeToken],
    structure: &StructureArtifact,
) -> Result<FormationArtifact, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut nodes = Vec::new();
    for span in &structure.compounds {
        let [start, end] = span.char_range;
        if start >= end || end > chars.len() {
            return Err(format!("构词跨度 {} 超出文本范围", span.id));
        }
        let observed: String = chars[start..end].iter().collect();
        if observed.trim().is_empty() {
            continue;
        }
        let token_indices: Vec<usize> = morphemes.iter().enumerate()
            .filter(|(_, token)| token.char_range[0] >= start && token.char_range[1] <= end)
            .map(|(index, _)| index)
            .collect();
        if token_indices.is_empty() {
            return Err(format!("构词跨度 {} 未覆盖 UniDic token", span.id));
        }
        nodes.push(FormationNode {
            id: format!("formation:{}", span.id),
            kind: span.kind.clone(),
            char_range: [start, end],
            morpheme_indices: token_indices,
            status: status(&span.status),
            evidence: vec![FormationEvidence {
                provider: span.provider.clone(),
                source_id: span.source_id.clone(),
                reason: "structure_compound".into(),
            }],
        });
    }
    let mut conflicts = Vec::new();
    for left in 0..nodes.len() {
        for right in (left + 1)..nodes.len() {
            let a = nodes[left].char_range;
            let b = nodes[right].char_range;
            if a[0] < b[1] && b[0] < a[1] && a != b {
                let mut ids = vec![nodes[left].id.clone(), nodes[right].id.clone()];
                ids.sort();
                let mut providers = BTreeSet::new();
                providers.extend(nodes[left].evidence.iter().map(|item| item.provider.as_str()));
                providers.extend(nodes[right].evidence.iter().map(|item| item.provider.as_str()));
                conflicts.push(FormationConflict {
                    id: format!("formation-conflict-{}-{}", left, right),
                    node_ids: ids,
                    char_range: [a[0].max(b[0]), a[1].min(b[1])],
                    reason: format!("overlap:{}", providers.into_iter().collect::<Vec<_>>().join(",")),
                });
            }
        }
    }
    Ok(FormationArtifact { schema: SCHEMA.into(), nodes, conflicts })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MorphemeToken;

    fn token(index: usize, range: [usize; 2], surface: &str) -> MorphemeToken {
        MorphemeToken { id: format!("m{index}"), source_index: index, surface: surface.into(), char_range: range, pos: [None, None, None, None], lemma: None, reading: None, query_forms: Vec::new() }
    }

    #[test]
    fn maps_compound_to_tokens_and_keeps_provider_evidence() {
        let structure = StructureArtifact {
            schema: crate::structure::SCHEMA.into(), provider: "local".into(), provider_version: None,
            paragraphs: Vec::new(), sentences: Vec::new(), clauses: Vec::new(), bunsetsu: Vec::new(),
            compounds: vec![crate::structure::StructureSpan {
                id: "ginza:c0".into(), kind: "compound".into(), char_range: [0, 4], status: StructureStatus::Observed,
                provider: "ginza".into(), source_id: Some("s".into()), head_char_range: None, labels: Vec::new(),
            }],
        };
        let result = collect_formations("情報処理", &[token(0, [0, 2], "情報"), token(1, [2, 4], "処理")], &structure).unwrap();
        assert_eq!(result.nodes[0].morpheme_indices, vec![0, 1]);
        assert_eq!(result.nodes[0].evidence[0].provider, "ginza");
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn records_overlapping_provider_candidates() {
        let span = |id: &str, range: [usize; 2], provider: &str| crate::structure::StructureSpan {
            id: id.into(), kind: "compound".into(), char_range: range, status: StructureStatus::Candidate,
            provider: provider.into(), source_id: None, head_char_range: None, labels: Vec::new(),
        };
        let structure = StructureArtifact {
            schema: crate::structure::SCHEMA.into(), provider: "local".into(), provider_version: None,
            paragraphs: Vec::new(), sentences: Vec::new(), clauses: Vec::new(), bunsetsu: Vec::new(),
            compounds: vec![span("g", [0, 4], "ginza"), span("k", [2, 6], "kwja")],
        };
        let result = collect_formations("情報処理技術", &[token(0, [0, 2], "情報"), token(1, [2, 4], "処理"), token(2, [4, 6], "技術")], &structure).unwrap();
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].char_range, [2, 4]);
    }
}
