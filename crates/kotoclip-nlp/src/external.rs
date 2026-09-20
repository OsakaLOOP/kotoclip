//! 外部模型原始空间、正文映射和结构关系。
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

pub const SCHEMA: &str = "kotoclip.source-analysis.v1";
pub type Range = [usize; 2];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceIdentity {
    pub role: String,
    pub name: String,
    pub path: Option<String>,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub model: String,
    pub model_version: String,
    pub versions: BTreeMap<String, String>,
    pub tasks: Vec<String>,
    pub capabilities: Vec<String>,
    pub coordinate_system: String,
    #[serde(default)]
    pub resources: Vec<ResourceIdentity>,
    #[serde(default)]
    pub resource_digest: String,
    #[serde(default)]
    pub execution: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind { Token, Compound, Bunsetsu, BasicPhrase, Sentence, Clause, Predicate, Entity }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceNode {
    pub id: String,
    pub kind: NodeKind,
    pub source_ranges: Vec<Range>,
    pub source_surface: String,
    pub text_ranges: Vec<Range>,
    pub surface: String,
    pub head: Option<String>,
    pub members: Vec<String>,
    pub features: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Endpoint {
    Node { id: String },
    Root,
    Exophora { label: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind { Dependency, PredicateArgument, Coreference, Bridging, Discourse }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRelation {
    pub id: String,
    pub kind: RelationKind,
    pub source: Endpoint,
    pub target: Endpoint,
    pub label: String,
    pub features: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceArtifact {
    pub schema: String,
    pub provider: Manifest,
    pub text_sha256: String,
    pub text_characters: usize,
    pub normalized_text: String,
    pub normalization_map: Vec<Range>,
    pub deleted_ranges: Vec<Range>,
    pub nodes: Vec<SourceNode>,
    pub relations: Vec<SourceRelation>,
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub raw: Option<String>,
    pub elapsed_ms: f64,
}

pub fn text_digest(text: &str) -> String { format!("{:x}", Sha256::digest(text.as_bytes())) }

pub fn merge_ranges(mut ranges: Vec<Range>) -> Vec<Range> {
    ranges.sort_unstable();
    let mut result: Vec<Range> = Vec::new();
    for range in ranges {
        if let Some(last) = result.last_mut().filter(|last| range[0] <= last[1]) {
            last[1] = last[1].max(range[1]);
        } else { result.push(range); }
    }
    result
}

fn surface(chars: &[char], ranges: &[Range]) -> Result<String, String> {
    let mut result = String::new();
    let mut previous_end = 0;
    for &[start, end] in ranges {
        if start < previous_end || start >= end || end > chars.len() {
            return Err("来源范围越界、交叉或为空".into());
        }
        result.extend(&chars[start..end]);
        previous_end = end;
    }
    Ok(result)
}

impl SourceArtifact {
    pub fn validate(&self, text: &str) -> Result<(), String> {
        let chars: Vec<char> = text.chars().collect();
        let normalized: Vec<char> = self.normalized_text.chars().collect();
        if self.schema != SCHEMA || self.provider.coordinate_system != "unicode_scalar" {
            return Err("来源协议或坐标版本不匹配".into());
        }
        if self.text_sha256 != text_digest(text) || self.text_characters != chars.len() {
            return Err("来源正文身份不匹配".into());
        }
        if self.normalization_map.len() != normalized.len() {
            return Err("规范化映射长度不匹配".into());
        }
        for (index, &[start, end]) in self.normalization_map.iter().enumerate() {
            if start >= end || end > chars.len() || (index > 0 && start < self.normalization_map[index - 1][0]) {
                return Err("规范化映射无效".into());
            }
        }
        surface(&chars, &self.deleted_ranges)?;
        let mut coverage = self.normalization_map.clone();
        coverage.extend(&self.deleted_ranges);
        if !chars.is_empty() && merge_ranges(coverage) != vec![[0, chars.len()]] {
            return Err("规范化映射未覆盖正文".into());
        }
        let ids: HashSet<&str> = self.nodes.iter().map(|node| node.id.as_str()).collect();
        if ids.len() != self.nodes.len() { return Err("来源实体 ID 重复".into()); }
        for node in &self.nodes {
            if node.source_ranges.is_empty() || node.text_ranges.is_empty()
                || surface(&normalized, &node.source_ranges)? != node.source_surface
                || surface(&chars, &node.text_ranges)? != node.surface {
                return Err(format!("来源实体 {} 表面串与坐标不符", node.id));
            }
            let mapped = merge_ranges(node.source_ranges.iter().flat_map(|&[a, b]| self.normalization_map[a..b].iter().copied()).collect());
            if mapped != node.text_ranges { return Err(format!("来源实体 {} 的映射不符", node.id)); }
            for reference in node.members.iter().chain(node.head.iter()) {
                if !ids.contains(reference.as_str()) { return Err(format!("来源引用 {reference} 无效")); }
            }
        }
        let mut relation_ids = HashSet::new();
        for relation in &self.relations {
            if !relation_ids.insert(&relation.id) { return Err("来源关系 ID 重复".into()); }
            for endpoint in [&relation.source, &relation.target] {
                if let Endpoint::Node { id } = endpoint {
                    if !ids.contains(id.as_str()) { return Err(format!("关系端点 {id} 无效")); }
                }
            }
        }
        Ok(())
    }

    /// 连续正文跨度进入现有结构消费方；完整来源空间保存在 SourceArtifact。
    pub fn syntax(&self) -> crate::syntax::SyntaxArtifact {
        use crate::syntax::{SyntaxArtifact, SyntaxProviderDescriptor, SyntaxSpan};
        let spans = self.nodes.iter().filter_map(|node| {
            let kind = match node.kind {
                NodeKind::Token => "token", NodeKind::Compound => "compound",
                NodeKind::Bunsetsu => "bunsetsu", NodeKind::Sentence => "sentence",
                NodeKind::Clause => "clause", _ => return None,
            };
            if node.text_ranges.len() != 1 { return None; }
            let head = node.head.as_ref().and_then(|id| self.nodes.iter().find(|n| &n.id == id));
            Some(SyntaxSpan { id: node.id.clone(), kind: kind.into(), char_range: node.text_ranges[0],
                head_char_range: head.filter(|n| n.text_ranges.len() == 1).map(|n| n.text_ranges[0]),
                source_id: self.text_sha256.clone(), surface: Some(node.surface.clone()),
                labels: node.features.as_object().map(|f| f.iter().map(|(k,v)| format!("{k}:{v}")).collect()).unwrap_or_default() })
        }).collect();
        SyntaxArtifact { schema: crate::syntax::SCHEMA.into(), segment_id: Some(self.text_sha256.clone()),
            provider: SyntaxProviderDescriptor { id: self.provider.id.clone(), version: Some(self.provider.version.clone()),
                capabilities: self.provider.capabilities.clone(), license: None }, text_characters: self.text_characters, text_sha256: self.text_sha256.clone(), spans }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "　警察署へ\t行った。\n彼は２８歳。 ｶﾞ、か\u{3099}、㍿、𠮷。  \n";

    fn samples() -> Vec<SourceArtifact> {
        serde_json::from_str(include_str!("../../../data/validation/behavior/integration-boundaries.json")).unwrap()
    }

    #[test]
    fn validates_real_model_outputs_and_normalization_expansion() {
        for source in samples() {
            source.validate(TEXT).unwrap();
            let syntax = source.syntax();
            assert!(crate::syntax::align_to_text_content(&syntax, TEXT).iter().all(|d| d.status == "aligned"));
            crate::structure::merge_external(crate::structure::local_candidates(TEXT), &syntax, TEXT).unwrap();
            if source.provider.id == "kwja" {
                let nodes: Vec<_> = source.nodes.iter().filter(|node| node.kind == NodeKind::Token && node.surface == "㍿").collect();
                assert_eq!(nodes.len(), 2);
                assert_eq!(nodes[0].text_ranges, vec![[24, 25]]);
                assert_eq!(nodes[1].text_ranges, vec![[24, 25]]);
            }
        }
    }

    #[test]
    fn rejects_same_length_wrong_text_and_dangling_reference() {
        let mut source = samples().remove(0);
        assert!(source.validate(&TEXT.replacen("彼", "私", 1)).is_err());
        source.nodes[0].head = Some("missing".into());
        assert!(source.validate(TEXT).unwrap_err().contains("引用"));
    }

    #[test]
    fn rejects_mapping_that_changes_original_location() {
        let mut source = samples().remove(1);
        source.nodes[0].text_ranges = vec![[1, 2]];
        assert!(source.validate(TEXT).is_err());
        let mut source = samples().remove(1);
        source.normalization_map.pop();
        assert!(source.validate(TEXT).unwrap_err().contains("映射长度"));
    }
}
