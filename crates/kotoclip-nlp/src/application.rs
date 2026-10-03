//! 应用决定、阅读单位和精确解释引用。
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApplicationArtifact {
    pub version: String,
    pub rules_version: u64,
    pub lexical: Vec<LexicalDecision>,
    pub reading_units: Vec<ReadingUnit>,
    pub explanations: Vec<Explanation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexicalDecision {
    pub id: String,
    pub char_range: [usize; 2],
    pub members: Vec<usize>,
    pub candidate_ids: Vec<String>,
    pub status: String,
    pub reason: String,
    pub bindings: Vec<DictionaryBinding>,
    pub competing_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryBinding {
    pub dictionary: String,
    pub entry_key: String,
    pub occurrence_id: String,
    pub headword: String,
    pub reading: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingUnit {
    pub id: String,
    pub char_range: [usize; 2],
    pub members: Vec<usize>,
    pub lexical_id: Option<String>,
    pub chain_ids: Vec<String>,
    pub bunsetsu_ids: Vec<String>,
    pub query_target_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explanation {
    pub id: String,
    pub layer: String,
    pub source_id: String,
    pub char_range: [usize; 2],
    pub hit_ranges: Vec<[usize; 2]>,
    pub members: Vec<usize>,
    pub captures: BTreeMap<String, Vec<usize>>,
    pub chain_ids: Vec<String>,
    pub concept_id: Option<String>,
    pub sense_id: Option<String>,
    pub sense_candidates: Vec<String>,
    pub status: String,
    pub reason: String,
    pub title: String,
    pub summary: String,
    pub evidence: Vec<String>,
}
