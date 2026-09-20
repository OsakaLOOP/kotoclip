//! 可解析的来源实体图与多来源默认选择。引用的有效范围由文档身份限定。
use crate::{alignment::{cover_range, SpanCoverage}, external::{self, Endpoint, NodeKind, Range, RelationKind, SourceArtifact}, model::MorphemeToken};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const SCHEMA: &str = "kotoclip.structure-graph.v1";
pub const SELECTION_VERSION: &str = "source-preference.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedEntity {
    pub id: String,
    pub provider: String,
    pub source_id: String,
    pub kind: NodeKind,
    pub text_ranges: Vec<Range>,
    pub source_ranges: Vec<Range>,
    pub surface: String,
    pub coverage: Vec<SpanCoverage>,
    pub alignment_group_ids: Vec<String>,
    pub morpheme_ids: Vec<String>,
    pub members: Vec<String>,
    pub head: Option<String>,
    pub head_morpheme_ids: Vec<String>,
    pub complete: bool,
    pub diagnostics: Vec<String>,
    pub features: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MappedEndpoint { Entity { id: String }, Root, Exophora { label: String } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedRelation {
    pub id: String,
    pub provider: String,
    pub source_id: String,
    pub kind: RelationKind,
    pub source: MappedEndpoint,
    pub target: MappedEndpoint,
    pub label: String,
    pub complete: bool,
    pub diagnostics: Vec<String>,
    pub features: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureCandidate {
    pub id: String,
    pub kind: NodeKind,
    pub text_ranges: Vec<Range>,
    pub evidence: Vec<String>,
    pub preferred_entity: Option<String>,
    pub selected: bool,
    pub reason: String,
    pub competing_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureGraph {
    pub schema: String,
    pub document_id: String,
    pub selection_version: String,
    pub entities: Vec<MappedEntity>,
    pub relations: Vec<MappedRelation>,
    pub candidates: Vec<StructureCandidate>,
}

fn stable_id(prefix: &str, value: impl Serialize) -> String {
    format!("{prefix}:{}", &external::text_digest(&serde_json::to_string(&value).unwrap())[..24])
}

fn preference(kind: &NodeKind, provider: &str) -> usize {
    let preferred = match kind {
        NodeKind::BasicPhrase | NodeKind::Clause | NodeKind::Predicate => "kwja",
        _ => "ginza",
    };
    if provider == preferred { 0 } else { 1 }
}

fn overlap(a: &[Range], b: &[Range]) -> bool {
    a.iter().any(|x| b.iter().any(|y| x[0] < y[1] && y[0] < x[1]))
}

fn contains(a: &[Range], b: &[Range]) -> bool {
    b.iter().all(|y| a.iter().any(|x| x[0] <= y[0] && y[1] <= x[1]))
}

pub fn build(document_id: &str, text: &str, morphemes: &[MorphemeToken], sources: &[SourceArtifact]) -> Result<StructureGraph, String> {
    let mut graph = StructureGraph { schema: SCHEMA.into(), document_id: document_id.into(), selection_version: SELECTION_VERSION.into(), entities: Vec::new(), relations: Vec::new(), candidates: Vec::new() };
    for source in sources {
        source.validate(text)?;
        let groups = crate::alignment_group::from_source(source, morphemes);
        let source_nodes: HashMap<_, _> = source.nodes.iter().map(|node| (node.id.as_str(), node)).collect();
        let identities: HashMap<_, _> = source.nodes.iter().map(|node| (node.id.as_str(), stable_id("entity", (&source.provider.id, &node.id, &node.kind, &node.text_ranges, &node.source_ranges)))).collect();
        for node in &source.nodes {
            let coverage: Vec<_> = node.text_ranges.iter().map(|&range| cover_range(range, morphemes, text)).collect();
            let indices: BTreeSet<_> = coverage.iter().flat_map(|c| c.morpheme_indices.iter().copied()).collect();
            let mut diagnostics: Vec<_> = coverage.iter().filter(|c| !c.complete).map(|c| c.reason.clone()).collect();
            let mut head = node;
            let mut visited = BTreeSet::new();
            while let Some(id) = &head.head {
                if !visited.insert(id) { return Err(format!("来源 {} 的主辞引用存在环", source.provider.id)); }
                head = source_nodes[id.as_str()];
            }
            let head_coverage: Vec<_> = if node.head.is_some() { head.text_ranges.iter().map(|&range| cover_range(range, morphemes, text)).collect() } else { Vec::new() };
            if head_coverage.iter().any(|c| !c.complete) { diagnostics.push("partial_head".into()); }
            if !contains(&node.text_ranges, &head.text_ranges) { diagnostics.push("head_outside_entity".into()); }
            if node.members.iter().any(|id| !contains(&node.text_ranges, &source_nodes[id.as_str()].text_ranges)) { diagnostics.push("member_outside_entity".into()); }
            if node.kind == NodeKind::Compound && coverage.iter().any(|c| !c.gaps.is_empty()) { diagnostics.push("compound_contains_gap".into()); }
            let head_indices: BTreeSet<_> = head_coverage.iter().flat_map(|c| c.morpheme_indices.iter().copied()).collect();
            if matches!(node.kind, NodeKind::Compound | NodeKind::Bunsetsu | NodeKind::BasicPhrase | NodeKind::Clause | NodeKind::Predicate)
                && !source.nodes.iter().filter(|n| n.kind == NodeKind::Sentence).any(|sentence| contains(&sentence.text_ranges, &node.text_ranges)) {
                diagnostics.push("outside_source_sentence".into());
            }
            graph.entities.push(MappedEntity {
                id: identities[node.id.as_str()].clone(), provider: source.provider.id.clone(), source_id: node.id.clone(),
                kind: node.kind.clone(), text_ranges: node.text_ranges.clone(), source_ranges: node.source_ranges.clone(), surface: node.surface.clone(),
                alignment_group_ids: groups.iter().filter(|group| overlap(&node.text_ranges, &[group.char_range])).map(|g| g.id.clone()).collect(),
                morpheme_ids: indices.iter().map(|&i| morphemes[i].id.clone()).collect(), coverage,
                members: node.members.iter().map(|id| identities[id.as_str()].clone()).collect(),
                head: node.head.as_ref().map(|id| identities[id.as_str()].clone()),
                head_morpheme_ids: head_indices.iter().map(|&i| morphemes[i].id.clone()).collect(),
                complete: diagnostics.is_empty(), diagnostics, features: node.features.clone(),
            });
        }
        let mapped: HashMap<_, _> = graph.entities.iter().map(|e| (e.id.as_str(), e)).collect();
        let endpoint = |value: &Endpoint| match value {
            Endpoint::Node { id } => MappedEndpoint::Entity { id: identities[id.as_str()].clone() },
            Endpoint::Root => MappedEndpoint::Root,
            Endpoint::Exophora { label } => MappedEndpoint::Exophora { label: label.clone() },
        };
        for relation in &source.relations {
            let from = endpoint(&relation.source);
            let target = endpoint(&relation.target);
            let mut diagnostics = Vec::new();
            if ![&from, &target].iter().all(|endpoint| match endpoint {
                MappedEndpoint::Entity { id } => mapped[id.as_str()].complete, _ => true,
            }) { diagnostics.push("pending_endpoint_alignment".into()); }
            if matches!(relation.kind, RelationKind::Dependency) {
                if let (MappedEndpoint::Entity { id: left }, MappedEndpoint::Entity { id: right }) = (&from, &target) {
                    if !source.nodes.iter().filter(|node| node.kind == NodeKind::Sentence).any(|sentence|
                        contains(&sentence.text_ranges, &mapped[left.as_str()].text_ranges) && contains(&sentence.text_ranges, &mapped[right.as_str()].text_ranges)) {
                        diagnostics.push("cross_sentence_dependency".into());
                    }
                }
            }
            graph.relations.push(MappedRelation {
                id: stable_id("relation", (&source.provider.id, &relation.id, &relation.kind, &from, &target, &relation.label)),
                provider: source.provider.id.clone(), source_id: relation.id.clone(), kind: relation.kind.clone(),
                source: from, target, label: relation.label.clone(), complete: diagnostics.is_empty(), diagnostics, features: relation.features.clone(),
            });
        }
    }
    let mut equivalent: BTreeMap<(NodeKind, Vec<Range>), Vec<&MappedEntity>> = BTreeMap::new();
    for entity in &graph.entities {
        if entity.kind != NodeKind::Token { equivalent.entry((entity.kind.clone(), entity.text_ranges.clone())).or_default().push(entity); }
    }
    for ((kind, text_ranges), mut entities) in equivalent {
        entities.sort_by_key(|e| (!e.complete, preference(&kind, &e.provider), &e.id));
        let preferred = entities.iter().find(|e| e.complete);
        graph.candidates.push(StructureCandidate {
            id: stable_id("candidate", (&kind, &text_ranges)), kind, text_ranges,
            evidence: entities.iter().map(|e| e.id.clone()).collect(), preferred_entity: preferred.map(|e| e.id.clone()), selected: false,
            reason: preferred.map(|e| format!("source_preference:{}", e.provider)).unwrap_or_else(|| "pending_alignment".into()),
            competing_ids: Vec::new(),
        });
    }
    let entities: HashMap<_, _> = graph.entities.iter().map(|e| (e.id.as_str(), e)).collect();
    graph.candidates.sort_by_key(|candidate| {
        let rank = candidate.preferred_entity.as_ref().map(|id| preference(&candidate.kind, &entities[id.as_str()].provider)).unwrap_or(2);
        (rank, candidate.text_ranges.clone(), candidate.id.clone())
    });
    for index in 0..graph.candidates.len() {
        if graph.candidates[index].preferred_entity.is_none() { continue; }
        let candidate = &graph.candidates[index];
        let competing: Vec<_> = graph.candidates[..index].iter().filter(|other| {
            if !other.selected || other.kind != candidate.kind || !overlap(&other.text_ranges, &candidate.text_ranges) { return false; }
            let nested = matches!(candidate.kind, NodeKind::Clause | NodeKind::Entity)
                && (contains(&other.text_ranges, &candidate.text_ranges) || contains(&candidate.text_ranges, &other.text_ranges));
            !nested
        }).map(|other| other.id.clone()).collect();
        graph.candidates[index].selected = competing.is_empty();
        if !competing.is_empty() { graph.candidates[index].reason = "competing_source_boundary".into(); }
        graph.candidates[index].competing_ids = competing;
    }
    Ok(graph)
}

impl StructureGraph {
    /// 消费层依据选择决定使用默认结构，所有来源及竞争候选继续保留。
    pub fn apply_selection(&self, structure: &mut crate::structure::StructureArtifact) {
        use crate::structure::StructureStatus;
        let selected: BTreeSet<_> = self.candidates.iter().filter(|c| c.selected).filter_map(|c| c.preferred_entity.as_deref()).collect();
        let entities: HashMap<_, _> = self.entities.iter().map(|e| (format!("{}:{}", e.provider, e.source_id), e)).collect();
        for span in structure.sentences.iter_mut().chain(&mut structure.clauses).chain(&mut structure.bunsetsu).chain(&mut structure.compounds) {
            if let Some(entity) = entities.get(&span.id) {
                span.status = if !entity.complete { StructureStatus::Pending } else if selected.contains(entity.id.as_str()) { StructureStatus::Observed } else { StructureStatus::Candidate };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEXT: &str = "　警察署へ\t行った。\n彼は２８歳。 ｶﾞ、か\u{3099}、㍿、𠮷。  \n";

    fn fixture() -> (Vec<SourceArtifact>, Vec<MorphemeToken>) {
        let sources = serde_json::from_str(include_str!("../../../data/validation/behavior/integration-boundaries.json")).unwrap();
        let data: Value = serde_json::from_str(include_str!("../../../data/validation/behavior/unidic.json")).unwrap();
        let segment = data["segments"].as_array().unwrap().iter().find(|s| s["id"] == "boundaries" && s["requested_register"] == "cwj").unwrap();
        let morphemes = serde_json::from_value(segment["document"]["morphemes"].clone()).unwrap();
        (sources, morphemes)
    }

    #[test]
    fn real_graph_keeps_basic_phrases_clauses_heads_and_resolvable_relations() {
        let (sources, morphemes) = fixture();
        let graph = build("document", TEXT, &morphemes, &sources).unwrap();
        assert!(graph.entities.iter().any(|e| e.kind == NodeKind::BasicPhrase));
        assert!(graph.entities.iter().any(|e| e.kind == NodeKind::Clause));
        let head = graph.entities.iter().find(|e| e.provider == "ginza" && e.source_id == "b1").unwrap();
        assert_eq!(head.head_morpheme_ids.len(), 2);
        let ids: BTreeSet<_> = graph.entities.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids.len(), graph.entities.len());
        for entity in &graph.entities {
            assert!(entity.members.iter().chain(&entity.head).all(|id| ids.contains(id.as_str())));
        }
        for relation in &graph.relations {
            for endpoint in [&relation.source, &relation.target] {
                if let MappedEndpoint::Entity { id } = endpoint { assert!(ids.contains(id.as_str())); }
            }
        }
        assert!(graph.relations.iter().any(|r| matches!(r.target, MappedEndpoint::Exophora { .. })));
        assert!(graph.relations.iter().any(|r| matches!(r.kind, RelationKind::Coreference)));
        let groups: BTreeSet<_> = sources.iter().flat_map(|s| crate::alignment_group::from_source(s, &morphemes)).map(|g| g.id).collect();
        assert!(graph.entities.iter().all(|e| e.alignment_group_ids.iter().all(|id| groups.contains(id))));
    }

    #[test]
    fn same_ranges_merge_evidence_with_deterministic_source_preference() {
        let (sources, morphemes) = fixture();
        let ginza = sources[0].clone();
        let mut kwja = ginza.clone();
        kwja.provider.id = "kwja".into();
        let graph = build("document", TEXT, &morphemes, &[kwja.clone(), ginza.clone()]).unwrap();
        let candidate = graph.candidates.iter().find(|c| c.kind == NodeKind::Bunsetsu && c.text_ranges == vec![[1, 5]]).unwrap();
        assert_eq!(candidate.evidence.len(), 2);
        assert!(candidate.selected);
        assert_eq!(candidate.reason, "source_preference:ginza");
        kwja.nodes.reverse();
        let reordered = build("document", TEXT, &morphemes, &[ginza, kwja]).unwrap();
        let actual = reordered.candidates.iter().find(|c| c.id == candidate.id).unwrap();
        assert_eq!(actual.preferred_entity, candidate.preferred_entity);
    }

    #[test]
    fn crossing_boundaries_keep_both_candidates_and_select_one() {
        let (mut sources, morphemes) = fixture();
        let mut kwja = sources[0].clone();
        kwja.provider.id = "kwja".into();
        let span = kwja.nodes.iter_mut().find(|n| n.id == "b1").unwrap();
        span.text_ranges = vec![[1, 6]];
        span.source_ranges = vec![[1, 6]];
        span.surface = "警察署へ\t".into();
        span.source_surface = span.surface.clone();
        sources[1] = kwja;
        let graph = build("document", TEXT, &morphemes, &sources).unwrap();
        let candidate = graph.candidates.iter().find(|c| c.kind == NodeKind::Bunsetsu && c.text_ranges == vec![[1, 6]]).unwrap();
        assert!(!candidate.selected);
        assert_eq!(candidate.reason, "competing_source_boundary");
        assert_eq!(candidate.competing_ids.len(), 1);
    }

    #[test]
    fn dependency_across_source_sentences_is_pending() {
        let (mut sources, morphemes) = fixture();
        let relation = sources[0].relations.iter_mut().find(|r| matches!(&r.source, Endpoint::Node { id } if id == "t1")).unwrap();
        relation.target = Endpoint::Node { id: "t8".into() };
        let relation_id = relation.id.clone();
        let graph = build("document", TEXT, &morphemes, &sources).unwrap();
        let actual = graph.relations.iter().find(|r| r.provider == "ginza" && r.source_id == relation_id).unwrap();
        assert!(!actual.complete);
        assert!(actual.diagnostics.iter().any(|d| d == "cross_sentence_dependency"));
    }
}
