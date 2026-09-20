//! 原生结构 provider 的运行时协议。
//!
//! provider 读取预处理后的原文，在各自的词法空间中生成结构证据。
//! UniDic 只在统一层负责字符范围映射和规范查询字段。
use crate::model::UnifiedDocument;
use crate::syntax::SyntaxArtifact;
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.native-structure-provider.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    #[serde(default)]
    pub installed_bytes: Option<u64>,
    #[serde(default)]
    pub peak_working_set: Option<u64>,
    #[serde(default)]
    pub inference_ms_per_1000_tokens: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeProviderInput<'a> {
    pub text: &'a str,
    pub characters: usize,
}

impl<'a> NativeProviderInput<'a> {
    pub fn from_document(document: &'a UnifiedDocument) -> Self {
        Self {
            text: &document.text,
            characters: document.characters,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NativeProviderStatus {
    Ready,
    Unsupported,
    Failed,
}

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

/// 模型图、权重和解码器尚未具备时保留 provider 身份与资源契约。
/// 调用方据此将该层标记为 unsupported，不生成伪造的结构证据。
#[derive(Debug, Clone)]
pub struct UnavailableNativeProvider {
    manifest: NativeModelManifest,
    reason: String,
}

impl UnavailableNativeProvider {
    pub fn ginza() -> Self {
        Self::new(
            NativeModelManifest {
                id: "ginza-native".into(),
                version: "5.2.1".into(),
                runtime: "rust".into(),
                model_format: "thinc-binary-pending-conversion".into(),
                capabilities: vec![
                    "token".into(),
                    "compound".into(),
                    "bunsetsu".into(),
                    "sentence".into(),
                    "dependency".into(),
                ],
                input_schema: "kotoclip.text.v1".into(),
                output_schema: crate::syntax::SCHEMA.into(),
                coordinate_system: "unicode_scalar".into(),
                parameter_count: None,
                weight_bytes: Some(78_971_273),
                training_dataset_version: None,
                installed_bytes: Some(296_437_312),
                peak_working_set: None,
                inference_ms_per_1000_tokens: None,
            },
            "native_model_graph_and_decoder_not_installed",
        )
    }

    pub fn kwja() -> Self {
        Self::new(
            NativeModelManifest {
                id: "kwja-native".into(),
                version: "2.1.3".into(),
                runtime: "rust".into(),
                model_format: "pytorch-checkpoint-pending-conversion".into(),
                capabilities: vec![
                    "token".into(),
                    "bunsetsu".into(),
                    "sentence".into(),
                    "dependency".into(),
                    "predicate".into(),
                    "basic_phrase".into(),
                ],
                input_schema: "kotoclip.text.v1".into(),
                output_schema: crate::syntax::SCHEMA.into(),
                coordinate_system: "unicode_scalar".into(),
                parameter_count: Some(17_224_819),
                weight_bytes: Some(69_022_387),
                training_dataset_version: None,
                installed_bytes: Some(157_388_505),
                peak_working_set: None,
                inference_ms_per_1000_tokens: None,
            },
            "native_model_graph_and_decoder_not_installed",
        )
    }

    pub fn new(manifest: NativeModelManifest, reason: impl Into<String>) -> Self {
        Self {
            manifest,
            reason: reason.into(),
        }
    }
}

impl NativeStructureProvider for UnavailableNativeProvider {
    fn manifest(&self) -> &NativeModelManifest {
        &self.manifest
    }

    fn analyze(&self, input: NativeProviderInput<'_>) -> Result<SyntaxArtifact, NativeProviderDiagnostic> {
        let actual_characters = input.text.chars().count();
        let reason = if actual_characters == input.characters {
            self.reason.clone()
        } else {
            "character_count_mismatch".into()
        };
        Err(NativeProviderDiagnostic {
            provider_id: self.manifest.id.clone(),
            status: NativeProviderStatus::Unsupported,
            reason: Some(reason),
            elapsed_ms: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_input_uses_text_instead_of_unidic_tokens() {
        let provider = UnavailableNativeProvider::ginza();
        let error = provider
            .analyze(NativeProviderInput {
                text: "太郎は走った。",
                characters: 7,
            })
            .unwrap_err();
        assert_eq!(error.status, NativeProviderStatus::Unsupported);
        assert_eq!(error.provider_id, "ginza-native");
        assert_eq!(provider.manifest().input_schema, "kotoclip.text.v1");
        assert_eq!(provider.manifest().installed_bytes, Some(296_437_312));
    }

    #[test]
    fn mismatched_character_count_is_explicit() {
        let provider = UnavailableNativeProvider::kwja();
        let error = provider
            .analyze(NativeProviderInput {
                text: "太郎",
                characters: 3,
            })
            .unwrap_err();
        assert_eq!(error.reason.as_deref(), Some("character_count_mismatch"));
    }
}
