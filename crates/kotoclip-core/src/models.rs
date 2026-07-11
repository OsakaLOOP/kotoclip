use serde::{Deserialize, Serialize};

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
    pub char_range: (usize, usize),
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedToken {
    pub bunsetsu: Bunsetsu,
    pub novelty_score: f32,
    pub is_selected: bool,
    pub is_known: bool,
    pub inference_reason: Option<String>,
    #[serde(default)]
    pub expressions: Vec<ExpressionAnnotation>,
    pub display_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpressionPatternPart {
    pub lemmas: Vec<String>,
    pub pos: Vec<String>,
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
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_expression_origin")]
    pub origin: String,
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
    pub position: String,
    pub token_range: (usize, usize),
    #[serde(default)]
    pub char_range: (usize, usize),
    pub surface: String,
}

fn default_expression_origin() -> String {
    "custom".to_string()
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
    pub definition_html: String,
    pub match_type: String,
    pub links: Vec<DictionaryLink>,
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
