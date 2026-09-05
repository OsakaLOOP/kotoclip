use serde::{Deserialize, Serialize};

use crate::entities::LexemeToken;
use crate::ProviderToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderDisagreement {
    pub char_range: (usize, usize),
    pub written: Vec<String>,
    pub spoken: Vec<String>,
    pub reason: String,
}

pub fn project_lexemes(tokens: &[ProviderToken]) -> Vec<LexemeToken> {
    tokens
        .iter()
        .enumerate()
        .map(|(index, token)| LexemeToken::from_provider(index, token))
        .collect()
}

pub fn compare_tokenizations(written: &[ProviderToken], spoken: &[ProviderToken]) -> Vec<ProviderDisagreement> {
    let mut ranges = written.iter().map(|token| token.char_range).collect::<Vec<_>>();
    ranges.extend(spoken.iter().map(|token| token.char_range));
    ranges.sort_unstable();
    ranges.dedup();
    ranges
        .into_iter()
        .filter_map(|range| {
            let left = written
                .iter()
                .filter(|token| token.char_range.0 >= range.0 && token.char_range.1 <= range.1)
                .map(|token| token.surface.clone())
                .collect::<Vec<_>>();
            let right = spoken
                .iter()
                .filter(|token| token.char_range.0 >= range.0 && token.char_range.1 <= range.1)
                .map(|token| token.surface.clone())
                .collect::<Vec<_>>();
            (left != right).then_some(ProviderDisagreement {
                char_range: range,
                written: left,
                spoken: right,
                reason: "provider_tokenization_difference".to_string(),
            })
        })
        .collect()
}
