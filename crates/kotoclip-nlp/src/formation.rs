//! GiNZA 正式词界、依存构词与 UniDic 查询核心。
use crate::{model::{MorphemeToken, QueryForm}, structure::{StructureArtifact, StructureStatus}};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SCHEMA: &str = "kotoclip.formation-artifact.v2";

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
    #[serde(default)]
    pub word: Option<FormationWord>,
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

/// 来源整体范围及其查询核心；词典验证由查询服务执行。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormationWord {
    pub surface: String,
    pub head_morpheme: Option<usize>,
    pub output_pos: Vec<Option<String>>,
    pub core_morpheme_indices: Vec<usize>,
    pub chain_ids: Vec<String>,
    pub source_token_ids: Vec<String>,
    pub source_relation_ids: Vec<String>,
    pub component_candidate_ids: Vec<String>,
    pub query_forms: Vec<QueryForm>,
    pub dictionary_status: String,
    pub reason: String,
}

fn status(value: &StructureStatus) -> FormationStatus {
    match value {
        StructureStatus::Observed => FormationStatus::Observed,
        StructureStatus::Candidate => FormationStatus::Candidate,
        StructureStatus::Pending => FormationStatus::Pending,
    }
}

fn add_ginza_candidates(text: &str, morphemes: &[MorphemeToken], sources: &[crate::external::SourceArtifact], nodes: &mut Vec<FormationNode>) -> Result<(), String> {
    use crate::external::{Endpoint, NodeKind, RelationKind};
    for source in sources.iter().filter(|s| s.provider.id == "ginza") {
        let tokens: std::collections::BTreeMap<_, _> = source.nodes.iter().filter(|n| n.kind == NodeKind::Token).map(|n| (n.id.as_str(), n)).collect();
        let mut proposals = Vec::new();
        for compound in source.nodes.iter().filter(|n| n.kind==NodeKind::Compound && n.text_ranges.len()==1) {
            let members=compound.members.iter().filter(|id| tokens.contains_key(id.as_str())).cloned().collect();
            let head=compound.head.as_ref().and_then(|id| tokens.get(id.as_str())).and_then(|n| n.text_ranges.first()).copied();
            proposals.push((compound.text_ranges[0],members,Vec::new(),"source_compound",head));
        }
        for token in tokens.values().filter(|n| n.text_ranges.len() == 1) {
            let range = token.text_ranges[0];
            let covered = crate::alignment::cover_range(range, morphemes, text);
            if covered.morpheme_indices.len() > 1 && !matches!(token.features["pos"].as_str(), Some("PUNCT" | "SPACE" | "ADP" | "AUX")) {
                proposals.push((range, vec![token.id.clone()], Vec::new(), "formal_token", Some(range)));
            }
            let mut member_ids = BTreeSet::from([token.id.clone()]);
            let mut relation_ids = BTreeSet::new();
            loop {
                let before = member_ids.len();
                for relation in &source.relations {
                    if !matches!(relation.kind, RelationKind::Dependency) || relation.label != "compound" { continue; }
                    let (Endpoint::Node { id: child }, Endpoint::Node { id: head }) = (&relation.source, &relation.target) else { continue; };
                    if member_ids.contains(head) && tokens.contains_key(child.as_str()) {
                        member_ids.insert(child.clone()); relation_ids.insert(relation.id.clone());
                    }
                }
                if before == member_ids.len() { break; }
            }
            if member_ids.len() < 2 { continue; }
            let ranges = crate::external::merge_ranges(member_ids.iter().flat_map(|id| tokens[id.as_str()].text_ranges.iter().copied()).collect());
            if let (Some(first), Some(last)) = (ranges.first(), ranges.last()) {
                proposals.push(([first[0],last[1]], member_ids.into_iter().collect(), relation_ids.into_iter().collect(),
                    if ranges.len()==1 { "dependency_compound" } else { "discontinuous_dependency" }, Some(range)));
            }
        }
        for (range, token_ids, relation_ids, kind, head_range) in proposals {
            if range[1] > text.chars().count() { return Err("GiNZA 构词范围超出正文".into()); }
            let coverage = crate::alignment::cover_range(range, morphemes, text);
            let lexical_members = coverage.morpheme_indices.iter().all(|&i| matches!(morphemes[i].pos[0].as_deref(), Some("名詞" | "動詞" | "形容詞" | "形状詞" | "接頭辞" | "接尾辞")));
            let complete = coverage.complete && coverage.gaps.is_empty() && lexical_members && kind != "discontinuous_dependency";
            let reason = if kind == "discontinuous_dependency" { "discontinuous_dependency" }
                else if !coverage.complete || !coverage.gaps.is_empty() { "incomplete_coverage" }
                else if !lexical_members { "nonlexical_members" }
                else { "complete_lexical_members" };
            let evidence = FormationEvidence { provider: "ginza".into(), source_id: Some(token_ids.join(",")), reason: kind.into() };
            let index = if let Some(i) = nodes.iter().position(|n| n.char_range == range) { i } else {
                nodes.push(FormationNode { id: format!("formation:ginza:{}:{}", range[0], range[1]), kind: kind.into(),
                    char_range: range, morpheme_indices: coverage.morpheme_indices.clone(),
                    status: if !complete { FormationStatus::Pending } else if kind=="dependency_compound" { FormationStatus::Candidate } else { FormationStatus::Observed }, evidence: Vec::new(), word: None });
                nodes.len()-1
            };
            let node = &mut nodes[index]; node.evidence.push(evidence);
            if !complete { node.status=FormationStatus::Pending; }
            let word = node.word.get_or_insert_with(|| FormationWord {
                surface: text.chars().skip(range[0]).take(range[1]-range[0]).collect(), head_morpheme: None,
                output_pos: Vec::new(), core_morpheme_indices: coverage.morpheme_indices.clone(), chain_ids: Vec::new(),
                source_token_ids: Vec::new(), source_relation_ids: Vec::new(), component_candidate_ids: Vec::new(), query_forms: Vec::new(),
                dictionary_status: "not_checked".into(), reason: reason.into(),
            });
            if !complete { word.reason = reason.into(); }
            word.head_morpheme = head_range.and_then(|r| coverage.morpheme_indices.iter().rev().find(|&&i| r[0] <= morphemes[i].char_range[0] && morphemes[i].char_range[1] <= r[1]).copied());
            word.source_token_ids.extend(token_ids); word.source_token_ids.sort(); word.source_token_ids.dedup();
            word.source_relation_ids.extend(relation_ids); word.source_relation_ids.sort(); word.source_relation_ids.dedup();
        }
    }
    Ok(())
}

/// 活用核心决定整体词的还原形式，所有成员仍保留原子查询引用。
pub fn attach_words(text: &str, morphemes: &[MorphemeToken], morphology: &crate::morphology::MorphologyArtifact, artifact: &mut FormationArtifact) {
    for node in &mut artifact.nodes {
        let range = node.char_range;
        let word = node.word.get_or_insert_with(|| FormationWord {
            surface: text.chars().skip(range[0]).take(range[1]-range[0]).collect(),
            head_morpheme: node.morpheme_indices.last().copied(), output_pos: Vec::new(),
            core_morpheme_indices: node.morpheme_indices.clone(), chain_ids: Vec::new(), source_token_ids: Vec::new(), source_relation_ids: Vec::new(),
            component_candidate_ids: Vec::new(), query_forms: Vec::new(), dictionary_status: "not_checked".into(), reason: "source_compound".into(),
        });
        word.component_candidate_ids = node.morpheme_indices.iter().filter(|&&i| crate::lexical::is_queryable(&morphemes[i].surface)).map(|&i| format!("dictionary:token:{}", morphemes[i].id)).collect();
        if let Some(head) = word.head_morpheme { word.output_pos = morphemes[head].pos.to_vec(); }
        word.chain_ids = morphology.chains.iter().filter(|c| c.core_morpheme_indices.iter().any(|i| node.morpheme_indices.contains(i))).map(|c| c.chain_id.clone()).collect();
        if node.status != FormationStatus::Pending && morphology.chains.iter().any(|c|
            word.chain_ids.contains(&c.chain_id) && c.status != "resolved") {
            node.status = FormationStatus::Candidate;
            word.reason = "morphology_core_candidate".into();
        }
        let reading = if node.status == FormationStatus::Pending { None } else {
            node.morpheme_indices.iter().map(|&i| morphemes[i].reading.clone()).collect::<Option<Vec<_>>>().map(|p| p.join(""))
        };
        word.query_forms = vec![QueryForm { kind: "compound_observed".into(), form: word.surface.clone(), reading: reading.clone(), reading_field: reading.map(|_| "composed_kana".into()) }];
        if node.status == FormationStatus::Pending { continue; }
        for kind in ["base", "lemma"] {
            let Some(&last) = node.morpheme_indices.last() else { continue; };
            let mut form = String::new(); let mut readings = Some(String::new());
            for &i in &node.morpheme_indices {
                let token = &morphemes[i];
                let query = token.query_forms.iter().find(|q| q.kind == if i == last { kind } else { "observed" });
                form.push_str(query.map(|q| q.form.as_str()).unwrap_or(&token.surface));
                let reading = match query { Some(query) => query.reading.as_ref(), None => token.reading.as_ref() };
                readings = readings.zip(reading).map(|(mut a,b)| { a.push_str(b); a });
            }
            if !word.query_forms.iter().any(|q| q.form == form) {
                word.query_forms.push(QueryForm { kind: format!("compound_{kind}"), form, reading_field: readings.as_ref().map(|_| format!("composed_{kind}")), reading: readings });
            }
        }
    }
}

/// 将结构层 compound 映射到 UniDic token，保留一对多和重叠关系。
pub fn collect_formations(
    text: &str,
    morphemes: &[MorphemeToken],
    structure: &StructureArtifact,
) -> Result<FormationArtifact, String> {
    collect_formations_with_sources(text, morphemes, structure, &[])
}

pub fn collect_formations_with_sources(
    text: &str,
    morphemes: &[MorphemeToken],
    structure: &StructureArtifact,
    sources: &[crate::external::SourceArtifact],
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
        let coverage = crate::alignment::cover_range(span.char_range, morphemes, text);
        nodes.push(FormationNode {
            id: format!("formation:{}", span.id),
            kind: span.kind.clone(),
            char_range: [start, end],
            morpheme_indices: coverage.morpheme_indices,
            status: if coverage.complete && coverage.gaps.is_empty() { status(&span.status) } else { FormationStatus::Pending },
            word: None,
            evidence: vec![FormationEvidence {
                provider: span.provider.clone(),
                source_id: span.source_id.clone(),
                reason: coverage.reason,
            }],
        });
    }
    add_ginza_candidates(text, morphemes, sources, &mut nodes)?;
    let mut merged: Vec<FormationNode> = Vec::new();
    for node in nodes {
        if let Some(existing) = merged.iter_mut().find(|n| n.char_range == node.char_range && n.morpheme_indices == node.morpheme_indices) {
            existing.evidence.extend(node.evidence);
            if existing.word.is_none() { existing.word = node.word; }
        } else { merged.push(node); }
    }
    let nodes = merged;
    let mut conflicts = Vec::new();
    for left in 0..nodes.len() {
        for right in (left + 1)..nodes.len() {
            let a = nodes[left].char_range;
            let b = nodes[right].char_range;
            let contains = (a[0] <= b[0] && b[1] <= a[1]) || (b[0] <= a[0] && a[1] <= b[1]);
            if a[0] < b[1] && b[0] < a[1] && !contains {
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
