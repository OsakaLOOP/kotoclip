//! 来源协作层的段落、句子和小句结构。
//! 该层只记录字符边界与来源证据，不推断词法或语法规则。
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StructureStatus { Observed, Candidate, Pending }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructureSpan {
    pub id: String,
    pub kind: String,
    pub char_range: [usize; 2],
    pub status: StructureStatus,
    pub provider: String,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub head_char_range: Option<[usize; 2]>,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructureArtifact {
    pub schema: String,
    pub provider: String,
    pub provider_version: Option<String>,
    pub paragraphs: Vec<StructureSpan>,
    pub sentences: Vec<StructureSpan>,
    pub clauses: Vec<StructureSpan>,
    pub bunsetsu: Vec<StructureSpan>,
    pub compounds: Vec<StructureSpan>,
}

pub const SCHEMA: &str = "kotoclip.structure-artifact.v1";

pub trait StructureProvider {
    fn descriptor(&self) -> (&'static str, Option<&'static str>);
    fn analyze(&self, text: &str) -> StructureArtifact;
}

pub struct LocalBoundaryProvider;

impl StructureProvider for LocalBoundaryProvider {
    fn descriptor(&self) -> (&'static str, Option<&'static str>) {
        ("local-boundary-candidate", None)
    }

    fn analyze(&self, text: &str) -> StructureArtifact {
        local_candidates(text)
    }
}

/// 生成来源无关的物理段落和句子候选。句子边界只使用文档标点，状态保持 candidate。
pub fn local_candidates(text: &str) -> StructureArtifact {
    let chars: Vec<char> = text.chars().collect();
    let mut paragraphs = Vec::new();
    let mut sentences = Vec::new();
    let mut clauses = Vec::new();
    let mut start = 0usize;
    let mut sentence_start = 0usize;
    let mut clause_start = 0usize;
    let mut paragraph_index = 0usize;
    let mut sentence_index = 0usize;
    let mut clause_index = 0usize;
    for (i, ch) in chars.iter().copied().enumerate() {
        if ch == '\n' {
            if start < i { paragraphs.push(span("paragraph", paragraph_index, [start, i], "local")); paragraph_index += 1; }
            start = i + 1;
        }
        if matches!(ch, '。' | '！' | '？' | '!' | '?' | '…') {
            let end = i + 1;
            if sentence_start < end { sentences.push(span("sentence", sentence_index, [sentence_start, end], "local")); sentence_index += 1; }
            if clause_start < end { clauses.push(span("clause", clause_index, [clause_start, end], "local")); clause_index += 1; }
            sentence_start = end;
            clause_start = end;
        } else if matches!(ch, '，' | '、' | ',' | ';' | '；') {
            let end = i + 1;
            if clause_start < end { clauses.push(span("clause", clause_index, [clause_start, end], "local")); clause_index += 1; }
            clause_start = end;
        }
    }
    if start < chars.len() { paragraphs.push(span("paragraph", paragraph_index, [start, chars.len()], "local")); }
    if sentence_start < chars.len() { sentences.push(span("sentence", sentence_index, [sentence_start, chars.len()], "local")); }
    if clause_start < chars.len() { clauses.push(span("clause", clause_index, [clause_start, chars.len()], "local")); }
    StructureArtifact { schema: SCHEMA.into(), provider: "local-boundary-candidate".into(), provider_version: None, paragraphs, sentences, clauses, bunsetsu: Vec::new(), compounds: Vec::new() }
}

/// 将外部 provider 的已对齐跨度追加到结构产物，保留本地候选与 provider 冲突。
pub fn merge_external(mut base: StructureArtifact, external: &crate::syntax::SyntaxArtifact, text: &str) -> Result<(StructureArtifact, Vec<crate::syntax::AlignmentDiagnostic>), String> {
    let diagnostics = crate::syntax::align_to_text_content(external, text);
    for (span, diagnostic) in external.spans.iter().zip(&diagnostics) {
        if diagnostic.status != "aligned" { continue; }
        let observed = StructureSpan { id: format!("{}:{}", external.provider.id, span.id), kind: span.kind.clone(), char_range: span.char_range, status: StructureStatus::Observed, provider: external.provider.id.clone(), source_id: Some(span.source_id.clone()), head_char_range: span.head_char_range, labels: span.labels.clone() };
        match span.kind.as_str() {
            "token" => {},
            "paragraph" => base.paragraphs.push(observed),
            "sentence" => base.sentences.push(observed),
            "clause" => base.clauses.push(observed),
            "bunsetsu" => base.bunsetsu.push(observed),
            "compound" => base.compounds.push(observed),
            _ => return Err(format!("未知的外部结构类型：{}", span.kind)),
        }
    }
    Ok((base, diagnostics))
}

fn span(kind: &str, index: usize, range: [usize; 2], provider: &str) -> StructureSpan {
    StructureSpan { id: format!("{kind}-{index}"), kind: kind.into(), char_range: range, status: StructureStatus::Candidate, provider: provider.into(), source_id: None, head_char_range: None, labels: Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_scalar_ranges_and_pending_external_boundary() {
        let a = local_candidates("甲。\n乙、丙。");
        assert_eq!(a.paragraphs[0].char_range, [0, 2]);
        assert_eq!(a.sentences.len(), 2);
        assert!(a.clauses.iter().all(|s| s.status == StructureStatus::Candidate));
        assert!(a.bunsetsu.is_empty());
    }

    #[test]
    fn merges_observed_external_spans_without_replacing_candidates() {
        let base = local_candidates("甲。乙。");
        let external = crate::syntax::SyntaxArtifact {
            schema: crate::syntax::SCHEMA.into(), segment_id: None,
            provider: crate::syntax::SyntaxProviderDescriptor { id: "ginza".into(), version: Some("5".into()), capabilities: vec![], license: None },
            text_characters: 4,
            spans: vec![crate::syntax::SyntaxSpan { id: "b0".into(), kind: "bunsetsu".into(), char_range: [0, 2], head_char_range: None, source_id: "sample".into(), surface: Some("甲。".into()), labels: vec![] }],
        };
        let (merged, diagnostics) = merge_external(base, &external, "甲。乙。").unwrap();
        assert_eq!(diagnostics[0].status, "aligned");
        assert_eq!(merged.bunsetsu[0].status, StructureStatus::Observed);
        assert_eq!(merged.sentences.len(), 2);
    }

    #[test]
    fn preserves_external_head_and_identity_labels() {
        let base = local_candidates("甲。");
        let external = crate::syntax::SyntaxArtifact {
            schema: crate::syntax::SCHEMA.into(), segment_id: None,
            provider: crate::syntax::SyntaxProviderDescriptor { id: "kwja".into(), version: Some("2.1.3".into()), capabilities: vec!["bunsetsu_identity".into()], license: None },
            text_characters: 2,
            spans: vec![crate::syntax::SyntaxSpan { id: "b0".into(), kind: "bunsetsu".into(), char_range: [0, 2], head_char_range: Some([0, 1]), source_id: "sample".into(), surface: Some("甲。".into()), labels: vec!["基本句-主辞".into()] }],
        };
        let (merged, _) = merge_external(base, &external, "甲。").unwrap();
        let span = &merged.bunsetsu[0];
        assert_eq!(span.head_char_range, Some([0, 1]));
        assert_eq!(span.labels, vec!["基本句-主辞"]);
        assert_eq!(span.source_id.as_deref(), Some("sample"));
    }
}
