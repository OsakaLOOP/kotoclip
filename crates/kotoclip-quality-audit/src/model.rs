use kotoclip_core::models::{AnnotatedToken, DictionaryLexicalUnitAnnotation, Morpheme};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const AUDIT_SCHEMA_VERSION: &str = "kotoclip.quality.selective-audit.v2";
pub const SUBSTRATE_SCHEMA_VERSION: &str = "kotoclip.quality.morpheme-substrate.v2";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum InfluenceScope {
    Clause,
    Paragraph,
    Book,
    Corpus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanDisposition {
    Selective,
    FullDomain,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticDelta {
    pub change_id: String,
    pub owner: String,
    pub kind: String,
    pub before_hash: String,
    pub after_hash: String,
    pub scope: InfluenceScope,
    pub old_selector: String,
    pub new_selector: String,
    pub observation_set: Vec<String>,
    pub soundness: String,
    #[serde(default)]
    pub reads_absence: bool,
    #[serde(default)]
    pub unbounded_context: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionPlan {
    pub disposition: PlanDisposition,
    pub scope: InfluenceScope,
    pub deltas: Vec<SemanticDelta>,
    pub reasons: Vec<String>,
    pub excluded_features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorpusSpec {
    pub schema_version: String,
    pub books: Vec<BookSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BookSpec {
    pub book_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharRange {
    pub start: usize,
    pub end: usize,
}

impl CharRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn intersects(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClauseRecord {
    pub clause_id: usize,
    pub range: CharRange,
    pub paragraph_id: usize,
    pub reading_sentence_id: usize,
    pub morphemes: Vec<Morpheme>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookChunk {
    pub schema_version: String,
    pub book_id: String,
    pub source_sha256: String,
    pub normalized_text_sha256: String,
    pub character_count: usize,
    pub ruby_annotations: Vec<kotoclip_core::pipeline::ruby::RubyAnnotation>,
    pub paragraph_ranges: Vec<CharRange>,
    pub reading_sentence_ranges: Vec<CharRange>,
    pub clauses: Vec<ClauseRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubstrateFingerprint {
    pub system_dictionary_sha256: String,
    pub prepare_text_protocol: String,
    pub boundary_protocol: String,
    pub morpheme_compatibility_protocol: String,
    pub substrate_schema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubstrateBookDescriptor {
    pub book_id: String,
    pub source_path: PathBuf,
    pub source_sha256: String,
    pub normalized_text_sha256: String,
    pub chunk_path: PathBuf,
    pub chunk_bytes: u64,
    pub chunk_sha256: String,
    pub character_count: usize,
    pub morpheme_count: usize,
    pub clause_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubstrateManifest {
    pub schema_version: String,
    pub substrate_id: String,
    pub created_at: String,
    pub corpus_spec_sha256: String,
    pub fingerprint: SubstrateFingerprint,
    pub books: Vec<SubstrateBookDescriptor>,
    pub total_characters: usize,
    pub total_morphemes: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexicalObservation {
    pub book_id: String,
    pub paragraph_id: usize,
    pub reading_sentence_id: usize,
    pub sentence_text: String,
    pub char_range: CharRange,
    pub changed_ranges: Vec<CharRange>,
    pub before_units: Vec<DictionaryLexicalUnitAnnotation>,
    pub after_units: Vec<DictionaryLexicalUnitAnnotation>,
    pub before_tokens: Vec<AnnotatedToken>,
    pub after_tokens: Vec<AnnotatedToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditCounts {
    pub books: usize,
    pub characters: usize,
    pub scanned_characters: usize,
    pub morphemes: usize,
    pub clauses: usize,
    pub formation_candidates: usize,
    pub lexical_candidates: usize,
    pub selected_clauses: usize,
    pub selected_clause_characters: usize,
    pub selected_paragraphs: usize,
    pub executed_paragraph_characters: usize,
    pub excluded_from_heavy_pipeline_characters: usize,
    pub dictionary_queries: usize,
    pub selector_dictionary_queries: usize,
    pub observation_dictionary_queries: usize,
    pub changed_paragraphs: usize,
    pub changes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEndpoint {
    pub revision: String,
    pub semantic_mode: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResourceUsage {
    pub elapsed_ms: u128,
    pub peak_rss_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub elapsed_ms: u128,
    pub peak_rss_bytes: u64,
    pub temporary_bytes: u64,
    pub artifact_bytes: u64,
    pub substrate_bytes: u64,
    pub stages: BTreeMap<String, StageResourceUsage>,
    pub measurement_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditManifest {
    pub schema_version: String,
    pub audit_id: String,
    pub created_at: String,
    pub substrate_id: String,
    pub corpus_spec_sha256: String,
    pub execution_fingerprint: String,
    pub before: AuditEndpoint,
    pub after: AuditEndpoint,
    pub plan: ExecutionPlan,
    pub counts: AuditCounts,
    pub resource_usage: ResourceUsage,
    pub complete: bool,
    pub n_best_in_scope: bool,
}
