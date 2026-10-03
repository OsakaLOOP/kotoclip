//! L2 文节证据层。
//! 文节边界和主辞完全来自结构 provider，模块只完成 token 与构词引用。
use crate::{formation::FormationArtifact, model::MorphemeToken, structure::{StructureArtifact, StructureStatus}};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SCHEMA: &str = "kotoclip.bunsetsu-artifact.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BunsetsuStatus { Observed, Candidate, Pending }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BunsetsuNode {
    pub id: String,
    pub char_range: [usize; 2],
    pub morpheme_indices: Vec<usize>,
    pub formation_node_ids: Vec<String>,
    pub status: BunsetsuStatus,
    pub provider: String,
    pub source_id: Option<String>,
    pub head_char_range: Option<[usize; 2]>,
    pub head_morpheme_index: Option<usize>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BunsetsuConflict {
    pub id: String,
    pub node_ids: Vec<String>,
    pub char_range: [usize; 2],
    pub providers: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BunsetsuArtifact {
    pub schema: String,
    pub nodes: Vec<BunsetsuNode>,
    pub conflict_groups: Vec<BunsetsuConflict>,
}

fn status(value: &StructureStatus) -> BunsetsuStatus {
    match value {
        StructureStatus::Observed => BunsetsuStatus::Observed,
        StructureStatus::Candidate => BunsetsuStatus::Candidate,
        StructureStatus::Pending => BunsetsuStatus::Pending,
    }
}

/// 将结构层 bunsetsu 映射到 UniDic token 与已确认的构词节点。
pub fn collect_bunsetsu(
    text: &str,
    morphemes: &[MorphemeToken],
    structure: &StructureArtifact,
    formation: &FormationArtifact,
) -> Result<BunsetsuArtifact, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut nodes = Vec::new();
    for span in &structure.bunsetsu {
        let [start, end] = span.char_range;
        if start >= end || end > chars.len() {
            return Err(format!("文节跨度 {} 超出文本范围", span.id));
        }
        let coverage = crate::alignment::cover_range(span.char_range, morphemes, text);
        let morpheme_indices = coverage.morpheme_indices;
        let head_morpheme_index = span.head_char_range.filter(|head| {
            head[0] >= start && head[1] <= end && head[0] < head[1]
        }).and_then(|head| {
            morphemes.iter().enumerate()
                .find(|(index, token)| token.char_range == head && morpheme_indices.contains(index))
                .map(|(index, _)| index)
        });
        let formation_node_ids = formation.nodes.iter()
            .filter(|node| node.char_range[0] >= start && node.char_range[1] <= end)
            .map(|node| node.id.clone())
            .collect();
        nodes.push(BunsetsuNode {
            id: format!("bunsetsu:{}", span.id),
            char_range: [start, end],
            morpheme_indices,
            formation_node_ids,
            status: if coverage.complete { status(&span.status) } else { BunsetsuStatus::Pending },
            provider: span.provider.clone(),
            source_id: span.source_id.clone(),
            head_char_range: span.head_char_range,
            head_morpheme_index,
            labels: span.labels.iter().cloned().chain([format!("alignment:{}", coverage.reason)]).collect(),
        });
    }

    let mut conflict_groups = Vec::new();
    for left in 0..nodes.len() {
        for right in (left + 1)..nodes.len() {
            let a = nodes[left].char_range;
            let b = nodes[right].char_range;
            if a[0] < b[1] && b[0] < a[1] && a != b {
                let mut node_ids = vec![nodes[left].id.clone(), nodes[right].id.clone()];
                node_ids.sort();
                let mut providers = BTreeSet::new();
                providers.insert(nodes[left].provider.clone());
                providers.insert(nodes[right].provider.clone());
                conflict_groups.push(BunsetsuConflict {
                    id: format!("bunsetsu-conflict-{}-{}", left, right),
                    node_ids,
                    char_range: [a[0].max(b[0]), a[1].min(b[1])],
                    providers: providers.into_iter().collect(),
                    reason: "overlapping_boundaries".into(),
                });
            }
        }
    }
    Ok(BunsetsuArtifact { schema: SCHEMA.into(), nodes, conflict_groups })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MorphemeToken;

    fn token(index: usize, range: [usize; 2], surface: &str) -> MorphemeToken {
        MorphemeToken { id: format!("m{index}"), source_index: index, surface: surface.into(), char_range: range, pos: [None, None, None, None], lemma: None, reading: None, query_forms: Vec::new() }
    }

    fn span(id: &str, range: [usize; 2], provider: &str, head: Option<[usize; 2]>) -> crate::structure::StructureSpan {
        crate::structure::StructureSpan { id: id.into(), kind: "bunsetsu".into(), char_range: range, status: StructureStatus::Observed, provider: provider.into(), source_id: Some("sample".into()), head_char_range: head, labels: vec!["head".into()] }
    }

    #[test]
    fn maps_head_and_formation_references() {
        let structure = StructureArtifact { schema: crate::structure::SCHEMA.into(), provider: "local".into(), provider_version: None, paragraphs: Vec::new(), sentences: Vec::new(), clauses: Vec::new(), bunsetsu: vec![span("g", [0, 4], "ginza", Some([2, 4]))], compounds: Vec::new() };
        let formation = FormationArtifact { schema: crate::formation::SCHEMA.into(), nodes: vec![crate::formation::FormationNode { id: "formation:c".into(), kind: "compound".into(), char_range: [0, 4], morpheme_indices: vec![0, 1], status: crate::formation::FormationStatus::Observed, evidence: Vec::new(), word: None }], conflicts: Vec::new() };
        let result = collect_bunsetsu("情報処理", &[token(0, [0, 2], "情報"), token(1, [2, 4], "処理")], &structure, &formation).unwrap();
        assert_eq!(result.nodes[0].head_morpheme_index, Some(1));
        assert_eq!(result.nodes[0].formation_node_ids, vec!["formation:c"]);
    }

    #[test]
    fn records_provider_boundary_conflicts() {
        let structure = StructureArtifact { schema: crate::structure::SCHEMA.into(), provider: "local".into(), provider_version: None, paragraphs: Vec::new(), sentences: Vec::new(), clauses: Vec::new(), bunsetsu: vec![span("g", [0, 4], "ginza", None), span("k", [2, 6], "kwja", None)], compounds: Vec::new() };
        let result = collect_bunsetsu("情報処理技術", &[token(0, [0, 2], "情報"), token(1, [2, 4], "処理"), token(2, [4, 6], "技術")], &structure, &FormationArtifact { schema: crate::formation::SCHEMA.into(), nodes: Vec::new(), conflicts: Vec::new() }).unwrap();
        assert_eq!(result.conflict_groups[0].char_range, [2, 4]);
        assert_eq!(result.conflict_groups[0].providers, vec!["ginza", "kwja"]);
    }

    #[test]
    fn preserves_provider_boundary_inside_unidic_token() {
        let mut structure = crate::structure::local_candidates("時折雨");
        structure.bunsetsu.push(span("kwja:b0", [0, 1], "kwja", Some([0, 1])));
        let formation = FormationArtifact { schema: crate::formation::SCHEMA.into(), nodes: Vec::new(), conflicts: Vec::new() };
        let result = collect_bunsetsu("時折雨", &[token(0, [0, 2], "時折"), token(1, [2, 3], "雨")], &structure, &formation).unwrap();
        assert_eq!(result.nodes[0].status, BunsetsuStatus::Pending);
        assert_eq!(result.nodes[0].morpheme_indices, vec![0]);
        assert_eq!(result.nodes[0].head_morpheme_index, None);
    }
}
