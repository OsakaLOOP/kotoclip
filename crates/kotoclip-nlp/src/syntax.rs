//! 外部结构分析器的只读协作协议与字符范围对齐。
//! provider 只追加证据，不改变 UniDic 语素或本地结构候选。
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyntaxProviderDescriptor {
    pub id: String,
    pub version: Option<String>,
    pub capabilities: Vec<String>,
    pub license: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyntaxSpan {
    pub id: String,
    pub kind: String,
    pub char_range: [usize; 2],
    pub head_char_range: Option<[usize; 2]>,
    pub source_id: String,
    #[serde(default)]
    pub surface: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyntaxArtifact {
    pub schema: String,
    #[serde(default)]
    pub segment_id: Option<String>,
    pub provider: SyntaxProviderDescriptor,
    pub text_characters: usize,
    #[serde(default)]
    pub text_sha256: String,
    pub spans: Vec<SyntaxSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlignmentDiagnostic {
    pub provider_span_id: String,
    pub status: String,
    pub reason: String,
    pub matched_range: Option<[usize; 2]>,
}

pub const SCHEMA: &str = "kotoclip.syntax-artifact.v2";

pub fn validate_identity(artifact: &SyntaxArtifact, text: &str) -> Result<(), String> {
    if artifact.schema != SCHEMA { return Err("artifact_schema_mismatch".into()); }
    if artifact.text_characters != text.chars().count() { return Err("artifact_character_count_mismatch".into()); }
    if artifact.text_sha256 != crate::external::text_digest(text) { return Err("artifact_text_digest_mismatch".into()); }
    Ok(())
}

pub fn parse_json(input: &str) -> Result<SyntaxArtifact, serde_json::Error> {
    serde_json::from_str(input)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyntaxArtifactBundle {
    pub schema: String,
    pub artifacts: Vec<SyntaxArtifact>,
}

/// 解析离线采集器输出的多 segment artifact wrapper。
pub fn parse_bundle_json(input: &str) -> Result<SyntaxArtifactBundle, serde_json::Error> {
    serde_json::from_str(input)
}

/// 只接受落在规范文本范围内的 provider 区间；越界或反向区间保持未对齐。
pub fn align_to_text(artifact: &SyntaxArtifact, text_characters: usize) -> Vec<AlignmentDiagnostic> {
    artifact.spans.iter().map(|span| {
        let [start, end] = span.char_range;
        if start < end && end <= text_characters {
            AlignmentDiagnostic {
                provider_span_id: span.id.clone(),
                status: "aligned".into(),
                reason: "unicode_scalar_range".into(),
                matched_range: Some([start, end]),
            }
        } else {
            AlignmentDiagnostic {
                provider_span_id: span.id.clone(),
                status: "unmatched".into(),
                reason: "range_out_of_bounds".into(),
                matched_range: None,
            }
        }
    }).collect()
}

/// 在范围检查之外核对 provider 返回的 surface，避免不同文本坐标被误合并。
pub fn align_to_text_content(artifact: &SyntaxArtifact, text: &str) -> Vec<AlignmentDiagnostic> {
    let chars: Vec<char> = text.chars().collect();
    let identity = validate_identity(artifact, text);
    align_to_text(artifact, chars.len())
        .into_iter()
        .zip(&artifact.spans)
        .map(|(mut diagnostic, span)| {
            if let Err(reason) = &identity {
                diagnostic.status = "unmatched".into();
                diagnostic.reason = reason.clone();
                diagnostic.matched_range = None;
                return diagnostic;
            }
            if diagnostic.status == "aligned" {
                if let Some(surface) = &span.surface {
                    let [start, end] = span.char_range;
                    let observed: String = chars[start..end].iter().collect();
                    if observed != *surface {
                        diagnostic.status = "unmatched".into();
                        diagnostic.reason = "surface_mismatch".into();
                        diagnostic.matched_range = None;
                    }
                }
            }
            diagnostic
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn descriptor() -> SyntaxProviderDescriptor {
        SyntaxProviderDescriptor { id: "fixture".into(), version: Some("1".into()), capabilities: vec!["bunsetsu".into()], license: None }
    }
    #[test]
    fn alignment_preserves_valid_ranges_and_rejects_invalid_ranges() {
        let artifact = SyntaxArtifact {
            schema: SCHEMA.into(), segment_id: None, provider: descriptor(), text_characters: 3,
            text_sha256: crate::external::text_digest("甲乙丙"),
            spans: vec![
                SyntaxSpan { id: "ok".into(), kind: "bunsetsu".into(), char_range: [0, 2], head_char_range: None, source_id: "x".into(), surface: None, labels: vec![] },
                SyntaxSpan { id: "bad".into(), kind: "bunsetsu".into(), char_range: [2, 4], head_char_range: None, source_id: "x".into(), surface: None, labels: vec![] },
            ],
        };
        let result = align_to_text(&artifact, 3);
        assert_eq!(result[0].matched_range, Some([0, 2]));
        assert_eq!(result[1].status, "unmatched");
    }

    #[test]
    fn content_alignment_rejects_surface_mismatch() {
        let artifact = SyntaxArtifact {
            schema: SCHEMA.into(), segment_id: None, provider: descriptor(), text_characters: 2,
            text_sha256: crate::external::text_digest("甲乙"),
            spans: vec![SyntaxSpan { id: "x".into(), kind: "token".into(), char_range: [0, 1], head_char_range: None, source_id: "x".into(), surface: Some("乙".into()), labels: vec![] }],
        };
        let result = align_to_text_content(&artifact, "甲乙");
        assert_eq!(result[0].reason, "surface_mismatch");
    }

    #[test]
    fn content_alignment_rejects_declared_character_count_mismatch() {
        let artifact = SyntaxArtifact {
            schema: SCHEMA.into(), segment_id: None, provider: descriptor(), text_characters: 3,
            text_sha256: crate::external::text_digest("甲乙"),
            spans: vec![SyntaxSpan { id: "x".into(), kind: "token".into(), char_range: [0, 1], head_char_range: None, source_id: "x".into(), surface: Some("甲".into()), labels: vec![] }],
        };
        let result = align_to_text_content(&artifact, "甲乙");
        assert_eq!(result[0].reason, "artifact_character_count_mismatch");
        assert!(result[0].matched_range.is_none());
    }

    #[test]
    fn digest_checks_even_empty_artifact_and_equal_length_text() {
        let artifact = SyntaxArtifact { schema: SCHEMA.into(), segment_id: None, provider: descriptor(), text_characters: 2,
            text_sha256: crate::external::text_digest("甲乙"), spans: Vec::new() };
        assert!(validate_identity(&artifact, "甲乙").is_ok());
        assert_eq!(validate_identity(&artifact, "丙丁").unwrap_err(), "artifact_text_digest_mismatch");
    }

    #[test]
    fn parses_json_artifact() {
        let json = r#"{"schema":"kotoclip.syntax-artifact.v1","provider":{"id":"ginza","version":"5.2.1","capabilities":["bunsetsu"],"license":"MIT"},"text_characters":2,"spans":[{"id":"b0","kind":"bunsetsu","char_range":[0,2],"head_char_range":null,"source_id":"x","surface":"甲。","labels":["基本句-主辞"]}]}"#;
        let artifact = parse_json(json).unwrap();
        assert_eq!(artifact.provider.id, "ginza");
        assert_eq!(artifact.spans[0].surface.as_deref(), Some("甲。"));
        assert_eq!(artifact.spans[0].labels, vec!["基本句-主辞"]);
    }

    #[test]
    fn parses_segment_artifact_bundle() {
        let json = r#"{"schema":"kotoclip.kwja-syntax-artifacts.v1","artifacts":[{"segment_id":"source-collective","schema":"kotoclip.syntax-artifact.v1","provider":{"id":"kwja","version":"2.1.3","capabilities":[],"license":null},"text_characters":1,"spans":[]}] }"#;
        let bundle = parse_bundle_json(json).unwrap();
        assert_eq!(bundle.artifacts[0].segment_id.as_deref(), Some("source-collective"));
    }
}
