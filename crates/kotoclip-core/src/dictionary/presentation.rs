use crate::dictionary::adapters;

pub use crate::dictionary::adapters::AdaptedOccurrence as DictionaryPresentation;

pub fn present(
    dict_name: &str,
    indexed_headword: &str,
    raw_headword: &str,
    structured_reading: Option<&str>,
    definition: &str,
) -> Vec<DictionaryPresentation> {
    adapters::adapt(
        dict_name,
        indexed_headword,
        raw_headword,
        structured_reading,
        definition,
    )
}
