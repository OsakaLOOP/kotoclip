use serde::{Deserialize, Serialize};

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_none<T>(value: &Option<T>) -> bool {
    value.is_none()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Morpheme {
    pub surface: String,
    pub pos: PosTag,
    pub base_form: String,
    pub reading: String,
    pub conjugation_type: String,
    pub conjugation_form: String,
    pub char_range: (usize, usize),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PosTag {
    pub major: String,
    pub sub1: String,
    pub sub2: String,
    pub sub3: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bunsetsu {
    pub morphemes: Vec<Morpheme>,
    pub surface: String,
    pub head_word: HeadWord,
    pub grammar_tags: Vec<GrammarTag>,
    #[serde(default)]
    pub word_formations: Vec<WordFormationAnnotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<BunsetsuFunctionAnnotation>,
    pub char_range: (usize, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BunsetsuFunction {
    Predicate,
    CasePhrase,
    Adnominal,
    Adverbial,
    Conjunctive,
    Nominal,
    Standalone,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BunsetsuFunctionAnnotation {
    pub function: BunsetsuFunction,
    pub confidence: u8,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub syntax_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BunsetsuBoundaryDecision {
    pub morpheme_index: usize,
    pub decision: String,
    pub score: i32,
    pub evidence: Vec<String>,
    pub alternatives: Vec<String>,
    #[serde(default)]
    pub alternative_score: i32,
    #[serde(default)]
    pub counter_evidence: Vec<String>,
    #[serde(default)]
    pub hard_constraint: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BunsetsuAnalysisReport {
    pub bunsetsus: Vec<Bunsetsu>,
    pub boundaries: Vec<BunsetsuBoundaryDecision>,
    pub unresolved_boundaries: usize,
    pub reconstruction_ok: bool,
    pub range_integrity_ok: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadWord {
    pub surface: String,
    pub base_form: String,
    pub reading: String,
    pub pos: PosTag,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarTag {
    pub pattern_id: String,
    pub name_ja: String,
    pub name_en: String,
    pub jlpt_level: Option<u8>,
    pub description: String,
    pub morpheme_range: (usize, usize),
    pub char_range: (usize, usize),
}

/// 由构词规则确认的连续语素单位。范围相对于所属文节的 morphemes，字符范围仍相对于全文。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WordFormationAnnotation {
    pub rule_id: String,
    pub category: String,
    pub surface: String,
    pub base_form: String,
    pub reading: String,
    pub output_pos: PosTag,
    pub morpheme_range: (usize, usize),
    pub char_range: (usize, usize),
    pub head_morpheme: usize,
    #[serde(default)]
    pub captures: Vec<WordFormationCapture>,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WordFormationCapture {
    pub name: String,
    pub surface: String,
    pub morpheme_range: (usize, usize),
    pub char_range: (usize, usize),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedToken {
    pub bunsetsu: Bunsetsu,
    pub novelty_score: f32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_selected: bool,
    pub is_known: bool,
    #[serde(default, skip_serializing_if = "is_none")]
    pub inference_reason: Option<String>,
    #[serde(default)]
    pub expressions: Vec<ExpressionAnnotation>,
    pub display_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpressionPatternPart {
    pub lemmas: Vec<String>,
    pub pos: Vec<String>,
    #[serde(default)]
    pub pos_details: Vec<PosTag>,
    #[serde(default)]
    pub conjugation_types: Vec<String>,
    #[serde(default)]
    pub conjugation_forms: Vec<String>,
    pub surface_hint: String,
    #[serde(default)]
    pub is_slot: bool,
    #[serde(default = "default_alignment")]
    pub alignment: String,
    #[serde(default)]
    pub is_any: bool,
}

fn default_alignment() -> String {
    "full".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionRule {
    pub id: i64,
    #[serde(default = "default_user_rule_schema")]
    pub schema_version: u32,
    #[serde(default = "default_user_rule_version")]
    pub rule_version: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub requires_review: bool,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_expression_origin")]
    pub origin: String,
    #[serde(default = "default_expression_kind")]
    pub expression_type: String,
    #[serde(default = "default_expression_priority")]
    pub priority: i32,
    #[serde(default = "default_boundary_effect")]
    pub boundary_effect: String,
    pub parts: Vec<ExpressionPatternPart>,
    #[serde(default)]
    pub gap_after: Option<usize>,
    #[serde(default = "default_gap_range")]
    pub gap_bunsetsu: (usize, usize),
    pub created_at: String,
}

fn default_gap_range() -> (usize, usize) {
    (0, 10)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionAnnotation {
    pub match_id: String,
    pub rule_id: i64,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_expression_origin")]
    pub origin: String,
    #[serde(default = "default_expression_kind")]
    pub expression_type: String,
    #[serde(default = "default_expression_priority")]
    pub priority: i32,
    #[serde(default = "default_boundary_effect")]
    pub boundary_effect: String,
    #[serde(default)]
    pub confidence: f32,
    pub position: String,
    pub token_range: (usize, usize),
    #[serde(default)]
    pub char_range: (usize, usize),
    #[serde(default)]
    pub matched_ranges: Vec<(usize, usize)>,
    pub surface: String,
}

fn default_user_rule_schema() -> u32 {
    1
}
fn default_user_rule_version() -> u32 {
    1
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpressionCandidateStatus {
    Accepted,
    Pending,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionCandidateCapture {
    pub name: String,
    pub surface: String,
    pub morpheme_range: (usize, usize),
    pub char_range: (usize, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCatalogAudit {
    pub layer: String,
    pub schema_version: u32,
    pub catalog_version: u32,
    pub rule_count: usize,
    pub enabled_rule_count: usize,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionCandidate {
    pub candidate_id: String,
    pub rule_id: String,
    pub rule_version: u32,
    pub origin: String,
    pub expression_type: String,
    pub status: ExpressionCandidateStatus,
    pub confidence: u8,
    pub label: String,
    pub description: String,
    pub matched_ranges: Vec<(usize, usize)>,
    pub covered_token_range: (usize, usize),
    pub char_range: (usize, usize),
    pub surface: String,
    #[serde(default)]
    pub captures: Vec<ExpressionCandidateCapture>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub counter_evidence: Vec<String>,
    #[serde(default)]
    pub rejection_reason: Option<String>,
    #[serde(default)]
    pub entry_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionRulePreview {
    pub status: ExpressionCandidateStatus,
    pub expression_type: String,
    pub surface: String,
    pub matched_ranges: Vec<(usize, usize)>,
    pub covered_token_range: (usize, usize),
    pub evidence: Vec<String>,
    pub counter_evidence: Vec<String>,
    #[serde(default)]
    pub rejection_reason: Option<String>,
}

fn default_expression_origin() -> String {
    "custom".to_string()
}

fn default_expression_kind() -> String {
    "grammar_construction".to_string()
}

fn default_expression_priority() -> i32 {
    50
}

fn default_boundary_effect() -> String {
    "annotate_only".to_string()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationCandidate {
    pub tokens: Vec<AnnotatedToken>,
    pub total_cost: i32,
    pub relative_cost: i32,
    pub source: String,
    pub vibrato_rank: usize,
    pub rank_score: i64,
    pub dictionary_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationChoice {
    pub surface: String,
    pub morphemes: Vec<Morpheme>,
    pub selected_cost: i32,
    pub selected_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportEntry {
    pub surface: String,
    pub base_form: String,
    pub reading: String,
    pub pos: String,
    pub grammar_tags: Vec<String>,
    pub context_sentence: String,
    pub context_highlight: (usize, usize),
    pub definitions: Vec<DictEntry>,
    pub jlpt_levels: Vec<u8>,
    pub user_note: String,
    pub char_range: Option<(usize, usize)>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictEntry {
    pub entry_key: String,
    pub dict_name: String,
    pub headword: String,
    pub reading: Option<String>,
    pub is_preferred: bool,
    pub definition_html: String,
    pub style_profile: String,
    pub content_blocks: Vec<DictionaryContentBlock>,
    pub match_type: String,
    pub links: Vec<DictionaryLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryContentBlock {
    pub kind: String,
    pub label: Option<String>,
    pub html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryLink {
    pub target: String,
    pub label: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryLookup {
    pub query: String,
    pub reading: Option<String>,
    pub selected_target: Option<String>,
    pub candidates: Vec<DictionaryLink>,
    pub entries: Vec<DictEntry>,
}
