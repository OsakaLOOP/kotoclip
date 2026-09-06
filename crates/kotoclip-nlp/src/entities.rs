use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ProviderToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SpanLayer {
    Formation,
    Lexical,
    Bunsetsu,
    Clause,
    Sentence,
    Grammar,
    Expression,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Accepted,
    Pending,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpanRelationKind {
    Contains,
    Overlaps,
    Excludes,
    Adjacent,
    Gap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpanRelation {
    pub kind: SpanRelationKind,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpanNode {
    pub id: String,
    pub layer: SpanLayer,
    pub kind: String,
    pub provider_id: Option<String>,
    pub morpheme_ids: Vec<usize>,
    pub char_range: (usize, usize),
    pub matched_range: Option<(usize, usize)>,
    pub covered_range: Option<(usize, usize)>,
    pub display_range: Option<(usize, usize)>,
    pub captures: Vec<Capture>,
    pub evidence: Vec<String>,
    pub counter_evidence: Vec<String>,
    pub confidence: u8,
    pub status: CandidateStatus,
    pub relations: Vec<SpanRelation>,
    pub boundary_effect: String,
    pub rule_ref: Option<String>,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capture {
    pub name: String,
    pub morpheme_ids: Vec<usize>,
    pub char_range: (usize, usize),
    pub surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LexemeToken {
    pub id: String,
    pub char_range: (usize, usize),
    pub surface_forms: Vec<String>,
    pub lemma: String,
    pub reading_candidates: Vec<String>,
    pub pos_profile: Vec<[String; 4]>,
    pub inflection: Vec<(String, String)>,
    pub source_token_ids: Vec<usize>,
    pub alignment_status: String,
}

impl LexemeToken {
    pub fn from_provider(index: usize, token: &ProviderToken) -> Self {
        let mut identity = Sha256::new();
        identity.update(token.char_range.0.to_le_bytes());
        identity.update(token.char_range.1.to_le_bytes());
        identity.update(token.lemma.as_bytes());
        identity.update(token.pos.join("/").as_bytes());
        let id = format!("lexeme:{:x}", identity.finalize());
        Self {
            id,
            char_range: token.char_range,
            surface_forms: vec![token.surface.clone()],
            lemma: token.lemma.clone(),
            reading_candidates: vec![
                token.pronunciation.clone(),
                token.pronunciation_base.clone(),
                token.lexeme_reading.clone(),
            ],
            pos_profile: vec![token.pos.clone()],
            inflection: vec![(token.conjugation_type.clone(), token.conjugation_form.clone())],
            source_token_ids: vec![index],
            alignment_status: "source_native".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactDescriptor {
    pub schema_version: String,
    pub document_id: String,
    pub revision: String,
    pub stage: String,
    pub input_fingerprint: String,
    pub provider_ids: Vec<String>,
    pub context_radius: usize,
}
