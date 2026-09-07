use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PosTag {
    pub major: String,
    pub sub1: String,
    pub sub2: String,
    pub sub3: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryEntryRef {
    pub entry_key: String,
    pub dict_name: String,
    pub headword: String,
    pub matched_form: String,
    pub match_type: String,
    #[serde(default)]
    pub readings: Vec<String>,
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
    #[serde(default)]
    pub occurrence_id: String,
    #[serde(default)]
    pub source_record_index: usize,
    #[serde(default)]
    pub entry_kind: String,
    #[serde(default)]
    pub header: DictionaryOccurrenceHeader,
    #[serde(default)]
    pub senses: Vec<DictionarySense>,
    #[serde(default)]
    pub sections: Vec<DictionarySection>,
    #[serde(default)]
    pub adapter_diagnostics: DictionaryAdapterDiagnostics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_evidence: Option<DictionaryMatchEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_definition: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryOccurrenceHeader {
    pub display_form: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_form: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub historical_reading: Option<String>,
    #[serde(default)]
    pub pronunciations: Vec<DictionaryPronunciation>,
    #[serde(default)]
    pub scoped_forms: Vec<DictionaryForm>,
    #[serde(default)]
    pub pos_tags: Vec<DictionaryTag>,
    #[serde(default)]
    pub usage_tags: Vec<DictionaryTag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryPronunciation {
    pub system: String,
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryForm {
    pub form: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading: Option<String>,
    #[serde(default)]
    pub kind: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryTag {
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryText {
    #[serde(default)]
    pub lang: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<String>,
    pub html: String,
}

/// 单个解释组中的连续子句。分号属于后一个子句的前置分隔符，
/// 以便在保留源顺序的同时让限定语、标签和译文保持为一个逻辑对象。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryGlossClause {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<String>,
    #[serde(default)]
    pub leading_tags: Vec<DictionaryTag>,
    pub text: DictionaryText,
    #[serde(default)]
    pub trailing_tags: Vec<DictionaryTag>,
}

/// 一个义项内部由日文适用范围开启的解释组。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryGlossGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    #[serde(default)]
    pub clauses: Vec<DictionaryGlossClause>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryExample {
    pub source: DictionaryText,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<DictionaryText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<DictionaryText>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionarySense {
    pub sense_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    #[serde(default)]
    pub glosses: Vec<DictionaryText>,
    #[serde(default)]
    pub gloss_groups: Vec<DictionaryGlossGroup>,
    #[serde(default)]
    pub definitions: Vec<DictionaryText>,
    #[serde(default)]
    pub tags: Vec<DictionaryTag>,
    #[serde(default)]
    pub examples: Vec<DictionaryExample>,
    #[serde(default)]
    pub notes: Vec<DictionaryText>,
    #[serde(default)]
    pub relations: Vec<DictionaryLink>,
    #[serde(default)]
    pub children: Vec<DictionarySense>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionarySectionItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_html: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading: Option<String>,
    #[serde(default)]
    pub content: Vec<DictionaryText>,
    #[serde(default)]
    pub tags: Vec<DictionaryTag>,
    #[serde(default)]
    pub examples: Vec<DictionaryExample>,
    #[serde(default)]
    pub senses: Vec<DictionarySense>,
    #[serde(default)]
    pub relations: Vec<DictionaryLink>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionarySection {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub items: Vec<DictionarySectionItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryAdapterDiagnostics {
    #[serde(default)]
    pub coverage: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub omitted: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryMatchEvidence {
    pub kind: String,
    pub query_form: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_form: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_reading: Option<String>,
    #[serde(default)]
    pub reading_match: String,
    #[serde(default)]
    pub pos_match: String,
    #[serde(default)]
    pub dictionary_local: bool,
    #[serde(default)]
    pub penalties: Vec<String>,
    #[serde(default)]
    pub score: i32,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryFormAvailability {
    pub dictionary_name: String,
    #[serde(default)]
    pub available: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryFormGroup {
    pub form_id: String,
    pub display_form: String,
    pub normalized_form: String,
    #[serde(default)]
    pub readings: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub variants: Vec<DictionaryFormVariant>,
    #[serde(default)]
    pub dictionaries: Vec<DictionaryFormAvailability>,
}

/// 归一分组内保留的原始表记及其独立证据。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryFormVariant {
    pub surface_form: String,
    #[serde(default)]
    pub readings: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub dictionary_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryLookup {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_form: Option<String>,
    pub reading: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<PosTag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_form_id: Option<String>,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub forms: Vec<DictionaryFormGroup>,
    pub dictionary_names: Vec<String>,
    pub entries: Vec<DictEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<DictionaryLookupTiming>,
}

/// 悬浮查词端到端诊断数据。仅开发诊断消费，不参与词典语义与缓存键。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryLookupTiming {
    pub resource_wait_ms: u64,
    pub service_ms: u64,
    pub redirect_ms: u64,
    pub sqlite_ms: u64,
    pub definition_ms: u64,
    pub presentation_ms: u64,
    pub definition_cache_hits: usize,
    pub definition_cache_misses: usize,
    pub entries: usize,
}

/// 可在界面中展示和配置的本地词典集合。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionarySettings {
    pub available_dictionaries: Vec<String>,
    pub default_dictionary: Option<String>,
    pub dictionary_order: Vec<String>,
}
