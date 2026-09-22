//! 词典查询候选层。
//! 候选来自 UniDic token 与已对齐 Formation 节点，不生成新的复合词范围。
use crate::{formation::{FormationArtifact, FormationStatus}, model::{MorphemeToken, QueryForm}};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.dictionary-candidates.v2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryCandidateStatus { Observed, Candidate, Pending }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryCandidate {
    pub id: String,
    pub kind: String,
    pub char_range: [usize; 2],
    pub surface: String,
    pub morpheme_indices: Vec<usize>,
    pub query_forms: Vec<QueryForm>,
    pub status: DictionaryCandidateStatus,
    pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryCandidateArtifact {
    pub schema: String,
    pub candidates: Vec<DictionaryCandidate>,
}

pub(crate) fn is_queryable(surface: &str) -> bool {
    surface.chars().any(char::is_alphanumeric)
}

fn status(value: &FormationStatus) -> DictionaryCandidateStatus {
    match value {
        FormationStatus::Observed => DictionaryCandidateStatus::Observed,
        FormationStatus::Candidate => DictionaryCandidateStatus::Candidate,
        FormationStatus::Pending => DictionaryCandidateStatus::Pending,
    }
}

/// 构建可追溯的 token 与 compound 查询候选。
pub fn collect_dictionary_candidates(text: &str, morphemes: &[MorphemeToken], formation: &FormationArtifact) -> Result<DictionaryCandidateArtifact, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut candidates = Vec::new();
    for (index, token) in morphemes.iter().enumerate() {
        if !is_queryable(&token.surface) { continue; }
        candidates.push(DictionaryCandidate {
            id: format!("dictionary:token:{}", token.id), kind: "token".into(),
            char_range: token.char_range, surface: token.surface.clone(), morpheme_indices: vec![index],
            query_forms: token.query_forms.clone(), status: DictionaryCandidateStatus::Observed,
            source_id: token.id.clone(),
        });
    }
    for node in &formation.nodes {
        let [start, end] = node.char_range;
        if start >= end || end > chars.len() { return Err(format!("词典候选 {} 超出文本范围", node.id)); }
        let surface: String = chars[start..end].iter().collect();
        if !is_queryable(&surface) { continue; }
        let reading = if node.status == FormationStatus::Pending { None } else { node.morpheme_indices.iter().map(|index| morphemes.get(*index).and_then(|token| token.reading.clone())).collect::<Option<Vec<_>>>().map(|parts| parts.join("")) };
        candidates.push(DictionaryCandidate {
            id: format!("dictionary:formation:{}", node.id), kind: "compound".into(),
            char_range: node.char_range, surface: surface.clone(), morpheme_indices: node.morpheme_indices.clone(),
            query_forms: node.word.as_ref().map(|w| w.query_forms.clone()).unwrap_or_else(|| vec![QueryForm { kind: "compound_observed".into(), form: surface, reading_field: reading.as_ref().map(|_| "kana_sequence".into()), reading }]),
            status: status(&node.status), source_id: node.id.clone(),
        });
    }
    Ok(DictionaryCandidateArtifact { schema: SCHEMA.into(), candidates })
}

pub fn add_morphology_candidates(artifact: &mut DictionaryCandidateArtifact, morphology: &crate::morphology::MorphologyArtifact) {
    for chain in &morphology.chains {
        if chain.morpheme_indices.len() == 1 && chain.parent_chain_id.is_none() { continue; }
        artifact.candidates.push(DictionaryCandidate {
            id: format!("dictionary:chain:{}", chain.chain_id), kind: "morphology".into(),
            char_range: chain.char_range, surface: chain.surface_form.clone(), morpheme_indices: chain.morpheme_indices.clone(),
            query_forms: chain.query_forms.clone(), source_id: chain.chain_id.clone(),
            status: if chain.status == "pending" { DictionaryCandidateStatus::Pending } else if chain.status == "candidate" { DictionaryCandidateStatus::Candidate } else { DictionaryCandidateStatus::Observed },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn token(index: usize, range: [usize; 2], surface: &str, reading: &str) -> MorphemeToken { MorphemeToken { id: format!("m{index}"), source_index: index, surface: surface.into(), char_range: range, pos: [None, None, None, None], lemma: None, reading: Some(reading.into()), query_forms: vec![QueryForm { kind: "observed".into(), form: surface.into(), reading: Some(reading.into()), reading_field: Some("kana".into()) }] } }

    #[test]
    fn exposes_token_and_formation_queries() {
        let formation = FormationArtifact { schema: crate::formation::SCHEMA.into(), nodes: vec![crate::formation::FormationNode { id: "formation:c".into(), kind: "compound".into(), char_range: [0, 3], morpheme_indices: vec![0, 1], status: FormationStatus::Observed, evidence: Vec::new(), word: None }], conflicts: Vec::new() };
        let result = collect_dictionary_candidates("警察署。", &[token(0, [0, 2], "警察", "ケイサツ"), token(1, [2, 3], "署", "ショ"), token(2, [3, 4], "。", "")], &formation).unwrap();
        assert_eq!(result.candidates.len(), 3);
        let compound = result.candidates.iter().find(|item| item.kind == "compound").unwrap();
        assert_eq!(compound.query_forms[0].form, "警察署");
        assert_eq!(compound.query_forms[0].reading.as_deref(), Some("ケイサツショ"));
    }
}
