mod common;
mod crown;
mod daijirin;
mod shogakukan;

use crate::models::{
    DictionaryAdapterDiagnostics, DictionaryContentBlock, DictionaryLink,
    DictionaryOccurrenceHeader, DictionarySection, DictionarySense,
};

#[derive(Debug, Clone, Default)]
pub struct AdaptedOccurrence {
    pub source_record_index: usize,
    pub occurrence_suffix: String,
    pub entry_kind: String,
    pub header: DictionaryOccurrenceHeader,
    pub senses: Vec<DictionarySense>,
    pub sections: Vec<DictionarySection>,
    pub links: Vec<DictionaryLink>,
    pub definition_html: String,
    pub style_profile: String,
    pub content_blocks: Vec<DictionaryContentBlock>,
    pub diagnostics: DictionaryAdapterDiagnostics,
}

pub fn adapt(
    dict_name: &str,
    indexed_headword: &str,
    raw_headword: &str,
    structured_reading: Option<&str>,
    definition: &str,
) -> Vec<AdaptedOccurrence> {
    if dict_name.contains("大辞林") {
        daijirin::adapt(
            indexed_headword,
            raw_headword,
            structured_reading,
            definition,
        )
    } else if dict_name.contains("小学館") || dict_name.contains("小学馆") {
        shogakukan::adapt(
            indexed_headword,
            raw_headword,
            structured_reading,
            definition,
        )
    } else if dict_name.contains("CROWN") || dict_name.contains("Crown") {
        crown::adapt(
            indexed_headword,
            raw_headword,
            structured_reading,
            definition,
        )
    } else {
        vec![common::fallback(
            "generic",
            indexed_headword,
            structured_reading,
            definition,
            "unknown",
        )]
    }
}
