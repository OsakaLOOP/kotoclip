use std::io::BufReader;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use vibrato::{Dictionary, Tokenizer};

pub mod alignment;
pub mod entities;
pub mod formation;
pub mod router;
pub mod stage;
pub use alignment::{compare_tokenizations, project_lexemes, ProviderDisagreement};
pub use entities::{ArtifactDescriptor, CandidateStatus, LexemeToken, SpanLayer, SpanNode};
pub use formation::{match_rule, FormationAtom, FormationRule};
pub use router::{ProviderRouter, RoutedAnalysis, TextRegister};
pub use stage::{EmptyStageArtifact, StageArtifact, StageFingerprint};

pub const ARTIFACT_SCHEMA: &str = "kotoclip.provider-token.v1";
pub const UNIDIC_FEATURE_SCHEMA: &str = "unidic-2025.12-feature.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderManifest {
    pub provider_id: String,
    pub dictionary_sha256: String,
    pub feature_schema: String,
    pub artifact_schema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderToken {
    pub provider_id: String,
    pub surface: String,
    pub char_range: (usize, usize),
    pub pos: [String; 4],
    pub conjugation_type: String,
    pub conjugation_form: String,
    pub lemma_form: String,
    pub lemma: String,
    pub orthographic_form: String,
    pub orthographic_base: String,
    pub pronunciation: String,
    pub pronunciation_base: String,
    pub word_type: String,
    pub raw_feature: String,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("无法打开字典: {0}")]
    Io(#[from] std::io::Error),
    #[error("无法读取 Vibrato 字典: {0}")]
    Dictionary(String),
}

pub struct VibratoProvider {
    provider_id: String,
    tokenizer: Tokenizer,
    manifest: ProviderManifest,
}

impl VibratoProvider {
    pub fn open(provider_id: impl Into<String>, path: impl AsRef<Path>) -> Result<Self, ProviderError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)?;
        let dictionary_sha256 = format!("sha256:{:x}", Sha256::digest(&bytes));
        let dictionary = Dictionary::read(BufReader::new(bytes.as_slice()))
            .map_err(|error| ProviderError::Dictionary(error.to_string()))?;
        let provider_id = provider_id.into();
        Ok(Self {
            manifest: ProviderManifest {
                provider_id: provider_id.clone(),
                dictionary_sha256,
                feature_schema: UNIDIC_FEATURE_SCHEMA.to_string(),
                artifact_schema: ARTIFACT_SCHEMA.to_string(),
            },
            provider_id,
            tokenizer: Tokenizer::new(dictionary),
        })
    }

    pub fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    pub fn analyze(&self, text: &str) -> Vec<ProviderToken> {
        let mut worker = self.tokenizer.new_worker();
        worker.reset_sentence(text);
        worker.tokenize();
        (0..worker.num_tokens())
            .map(|index| {
                let token = worker.token(index);
                let range = token.range_char();
                parse_feature(
                    &self.provider_id,
                    token.surface(),
                    token.feature(),
                    (range.start, range.end),
                )
            })
            .collect()
    }
}

fn field(fields: &[&str], index: usize) -> String {
    fields.get(index).copied().unwrap_or("*").to_string()
}

fn value_or_surface(value: String, surface: &str) -> String {
    if value.is_empty() || value == "*" {
        surface.to_string()
    } else {
        value
    }
}

pub fn parse_feature(provider_id: &str, surface: &str, raw_feature: &str, char_range: (usize, usize)) -> ProviderToken {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(raw_feature.as_bytes());
    let fields = reader
        .records()
        .next()
        .and_then(Result::ok)
        .map(|record| record.iter().map(str::to_string).collect::<Vec<_>>())
        .unwrap_or_else(|| raw_feature.split(',').map(str::to_string).collect());
    let fields = fields.iter().map(String::as_str).collect::<Vec<_>>();
    ProviderToken {
        provider_id: provider_id.to_string(),
        surface: surface.to_string(),
        char_range,
        pos: [field(&fields, 0), field(&fields, 1), field(&fields, 2), field(&fields, 3)],
        conjugation_type: field(&fields, 4),
        conjugation_form: field(&fields, 5),
        lemma_form: field(&fields, 6),
        lemma: value_or_surface(field(&fields, 7), surface),
        orthographic_form: value_or_surface(field(&fields, 8), surface),
        orthographic_base: value_or_surface(field(&fields, 9), surface),
        pronunciation: value_or_surface(field(&fields, 10), surface),
        pronunciation_base: value_or_surface(field(&fields, 11), surface),
        word_type: field(&fields, 12),
        raw_feature: raw_feature.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unidic_fields_without_ipadic_shift() {
        let token = parse_feature(
            "unidic-cwj-202512",
            "向かっ",
            "動詞,一般,*,*,五段-ワア行,連用形-促音便,ムカウ,向かう,向かっ,ムカッ,向かう,ムカウ,和",
            (0, 3),
        );
        assert_eq!(token.lemma, "向かう");
        assert_eq!(token.orthographic_base, "ムカッ");
        assert_eq!(token.pronunciation, "向かう");
        assert_eq!(token.pronunciation_base, "ムカウ");
        assert_eq!(token.word_type, "和");
    }
}
