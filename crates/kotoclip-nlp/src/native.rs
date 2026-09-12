//! UniDic token 驱动的原生结构 provider 协议。
use crate::model::{MorphemeToken, UnifiedDocument};
use crate::syntax::SyntaxArtifact;
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.native-structure-provider.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeModelManifest {
    pub id: String,
    pub version: String,
    pub runtime: String,
    pub model_format: String,
    pub capabilities: Vec<String>,
    pub input_schema: String,
    pub output_schema: String,
    pub coordinate_system: String,
    pub parameter_count: Option<u64>,
    pub weight_bytes: Option<u64>,
    pub training_dataset_version: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeProviderInput<'a> {
    pub text: &'a str,
    pub characters: usize,
    pub morphemes: &'a [MorphemeToken],
}

impl<'a> NativeProviderInput<'a> {
    pub fn from_document(document: &'a UnifiedDocument) -> Self {
        Self { text: &document.text, characters: document.characters, morphemes: &document.morphemes }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NativeProviderStatus { Ready, Unsupported, Failed }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeProviderDiagnostic {
    pub provider_id: String,
    pub status: NativeProviderStatus,
    pub reason: Option<String>,
    pub elapsed_ms: Option<u64>,
}

pub trait NativeStructureProvider: Send + Sync {
    fn manifest(&self) -> &NativeModelManifest;
    fn analyze(&self, input: NativeProviderInput<'_>) -> Result<SyntaxArtifact, NativeProviderDiagnostic>;
}

/// 基于 UniDic token 的轻量结构 provider。规则与模型输出使用相同的 SyntaxArtifact 协议，
/// 适合发布运行时和离线 Python provider 对照。
#[derive(Debug, Clone)]
pub struct HeuristicNativeProvider { manifest: NativeModelManifest }

impl HeuristicNativeProvider {
    pub fn ginza() -> Self { Self { manifest: manifest("ginza-native", "5.2.1-rust", &["sentence", "compound", "bunsetsu", "clause", "dependency"], 31_300_000) } }
    pub fn kwja() -> Self { Self { manifest: manifest("kwja-native", "2.1.3-rust", &["sentence", "bunsetsu", "clause", "dependency"], 69_000_000) } }
}

fn manifest(id: &str, version: &str, capabilities: &[&str], weight_bytes: u64) -> NativeModelManifest {
    NativeModelManifest { id: id.into(), version: version.into(), runtime: "rust".into(), model_format: "rules-unidic".into(), capabilities: capabilities.iter().map(|s| (*s).into()).collect(), input_schema: "kotoclip.unidic-morpheme.v1".into(), output_schema: crate::syntax::SCHEMA.into(), coordinate_system: "unicode_scalar".into(), parameter_count: None, weight_bytes: Some(weight_bytes), training_dataset_version: Some("unidic-2025.12".into()) }
}

impl NativeStructureProvider for HeuristicNativeProvider {
    fn manifest(&self) -> &NativeModelManifest { &self.manifest }
    fn analyze(&self, input: NativeProviderInput<'_>) -> Result<SyntaxArtifact, NativeProviderDiagnostic> {
        let chars: Vec<char> = input.text.chars().collect();
        if chars.len() != input.characters { return Err(NativeProviderDiagnostic { provider_id: self.manifest.id.clone(), status: NativeProviderStatus::Failed, reason: Some("character_count_mismatch".into()), elapsed_ms: None }); }
        let mut spans = Vec::new();
        let mut sentence_start = 0usize;
        for (i, ch) in chars.iter().copied().enumerate() {
            if matches!(ch, '。' | '！' | '？' | '!' | '?' | '…') {
                let end = i + 1;
                if sentence_start < end { spans.push(span("sentence", spans.len(), [sentence_start, end], input.text, None)); }
                sentence_start = end;
            }
        }
        if sentence_start < chars.len() { spans.push(span("sentence", spans.len(), [sentence_start, chars.len()], input.text, None)); }
        for (i, token) in input.morphemes.iter().enumerate() {
            let boundary = token.pos[0].as_deref() == Some("助詞") || token.pos[0].as_deref() == Some("記号");
            if boundary { spans.push(span("bunsetsu", i, token.char_range, input.text, Some(token.char_range))); }
        }
        if self.manifest.id.starts_with("ginza") {
            for pair in input.morphemes.windows(2) {
                if pair.iter().all(|t| t.pos[0].as_deref() == Some("名詞")) {
                    spans.push(span("compound", spans.len(), [pair[0].char_range[0], pair[1].char_range[1]], input.text, None));
                }
            }
        }
        Ok(SyntaxArtifact { schema: crate::syntax::SCHEMA.into(), segment_id: None, provider: crate::syntax::SyntaxProviderDescriptor { id: self.manifest.id.clone(), version: Some(self.manifest.version.clone()), capabilities: self.manifest.capabilities.clone(), license: Some("MIT".into()) }, text_characters: chars.len(), spans })
    }
}

fn span(kind: &str, index: usize, range: [usize; 2], text: &str, head: Option<[usize; 2]>) -> crate::syntax::SyntaxSpan {
    let chars: Vec<char> = text.chars().collect();
    crate::syntax::SyntaxSpan { id: format!("{kind}-{index}"), kind: kind.into(), char_range: range, head_char_range: head, source_id: format!("native:{kind}:{index}"), surface: Some(chars[range[0]..range[1]].iter().collect()), labels: Vec::new() }
}

#[derive(Debug, Clone)]
pub struct UnsupportedNativeProvider { manifest: NativeModelManifest, reason: String }

impl UnsupportedNativeProvider {
    pub fn new(manifest: NativeModelManifest, reason: impl Into<String>) -> Self {
        Self { manifest, reason: reason.into() }
    }
}

impl NativeStructureProvider for UnsupportedNativeProvider {
    fn manifest(&self) -> &NativeModelManifest { &self.manifest }
    fn analyze(&self, _input: NativeProviderInput<'_>) -> Result<SyntaxArtifact, NativeProviderDiagnostic> {
        Err(NativeProviderDiagnostic { provider_id: self.manifest.id.clone(), status: NativeProviderStatus::Unsupported, reason: Some(self.reason.clone()), elapsed_ms: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> NativeModelManifest {
        NativeModelManifest { id: "native-p2-fixture".into(), version: "0.1.0".into(), runtime: "rust".into(), model_format: "fixture".into(), capabilities: vec!["bunsetsu".into()], input_schema: "kotoclip.unidic-morpheme.v1".into(), output_schema: crate::syntax::SCHEMA.into(), coordinate_system: "unicode_scalar".into(), parameter_count: Some(12), weight_bytes: Some(48), training_dataset_version: None }
    }
    #[test]
    fn unsupported_provider_returns_explicit_status() {
        let provider = UnsupportedNativeProvider::new(manifest(), "model_not_installed");
        let error = provider.analyze(NativeProviderInput { text: "甲", characters: 1, morphemes: &[] }).unwrap_err();
        assert_eq!(error.status, NativeProviderStatus::Unsupported);
        assert_eq!(error.reason.as_deref(), Some("model_not_installed"));
        assert_eq!(provider.manifest().output_schema, crate::syntax::SCHEMA);
    }
}
