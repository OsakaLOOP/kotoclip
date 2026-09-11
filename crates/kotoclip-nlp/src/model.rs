use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.unified-document.v4";
pub const FIELD_NAMES: [&str; 29] = [
    "pos1", "pos2", "pos3", "pos4", "cType", "cForm", "lForm", "lemma", "orth", "pron", "orthBase",
    "pronBase", "goshu", "iType", "iForm", "fType", "fForm", "iConType", "fConType", "type",
    "kana", "kanaBase", "form", "formBase", "aType", "aConType", "aModType", "lid", "lemma_id",
];
pub const FIELD_LABELS: [&str; 29] = [
    "词性一级",
    "词性二级",
    "词性三级",
    "词性四级",
    "活用型",
    "活用形",
    "词元读法",
    "词元",
    "出现表记",
    "出现发音",
    "基本表记",
    "基本发音",
    "语种",
    "词首变化类型",
    "词首变化形式",
    "词尾变化类型",
    "词尾变化形式",
    "词首结合类型",
    "词尾结合类型",
    "词类补充",
    "出现假名",
    "基本假名",
    "语形",
    "基本语形",
    "重音型",
    "重音结合型",
    "重音变化型",
    "词条 ID",
    "词元 ID",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Register {
    Cwj,
    Csj,
}

impl Register {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cwj => "cwj",
            Self::Csj => "csj",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetadata {
    pub id: String,
    pub version: String,
    pub dictionary_sha256: String,
    pub field_schema: String,
    pub max_grouping_length: usize,
    pub ignore_space: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureField {
    pub index: usize,
    pub name: String,
    pub label: String,
    pub raw: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderToken {
    pub index: usize,
    pub surface: String,
    pub char_range: [usize; 2],
    pub byte_range: [usize; 2],
    pub lexicon_type: String,
    pub left_id: u16,
    pub right_id: u16,
    pub word_cost: i16,
    pub total_cost: i32,
    pub raw_feature: String,
    pub fields: Vec<FeatureField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryForm {
    pub kind: String,
    pub form: String,
    pub reading: Option<String>,
    pub reading_field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphemeToken {
    pub id: String,
    pub source_index: usize,
    pub surface: String,
    pub char_range: [usize; 2],
    pub pos: [Option<String>; 4],
    pub lemma: Option<String>,
    pub reading: Option<String>,
    pub query_forms: Vec<QueryForm>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextGap {
    pub char_range: [usize; 2],
    pub surface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceAnalysis {
    pub provider: ProviderMetadata,
    pub tokens: Vec<ProviderToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RubyValidation {
    pub base: String,
    pub ruby_reading: String,
    pub expected_reading: String,
    pub char_range: [usize; 2],
    pub original_char_range: [usize; 2],
    pub token_range: Option<[usize; 2]>,
    pub observed_reading: Option<String>,
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRouting {
    pub requested: Register,
    pub selected: Register,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedDocument {
    pub schema: String,
    pub id: String,
    pub text: String,
    pub characters: usize,
    pub source: SourceAnalysis,
    pub routing: RegisterRouting,
    pub ruby_validations: Vec<RubyValidation>,
    pub morphemes: Vec<MorphemeToken>,
    pub gaps: Vec<TextGap>,
    pub structure: crate::structure::StructureArtifact,
    pub formation: crate::formation::FormationArtifact,
    pub bunsetsu: crate::bunsetsu::BunsetsuArtifact,
    pub clause: crate::clause::ClauseArtifact,
    pub dictionary_candidates: crate::lexical::DictionaryCandidateArtifact,
    pub grammar: crate::grammar::GrammarArtifact,
    pub expression: crate::expression::ExpressionArtifact,
    pub projection: crate::projection::ProjectionArtifact,
    pub morphology: crate::morphology::MorphologyArtifact,
    #[serde(default)]
    pub structure_diagnostics: Vec<crate::syntax::AlignmentDiagnostic>,
    #[serde(default)]
    pub provider_token_alignments: Vec<crate::alignment::TokenAlignmentArtifact>,
    pub elapsed_ms: f64,
}
