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
