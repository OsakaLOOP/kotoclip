//! L3 表达候选层。表达规则由独立目录提供，默认保持空候选。
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.expression-artifact.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpressionStatus { Observed, Candidate, Pending, Rejected }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpressionOccurrence {
    pub id: String,
    pub rule_id: Option<String>,
    pub char_range: [usize; 2],
    pub morpheme_indices: Vec<usize>,
    pub status: ExpressionStatus,
    pub provider: String,
    pub source_id: Option<String>,
    pub expression_type: String,
    pub labels: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpressionArtifact {
    pub schema: String,
    pub occurrences: Vec<ExpressionOccurrence>,
}

pub fn empty() -> ExpressionArtifact {
    ExpressionArtifact { schema: SCHEMA.into(), occurrences: Vec::new() }
}

pub fn merge(mut base: ExpressionArtifact, overlay: ExpressionArtifact) -> ExpressionArtifact {
    for occurrence in overlay.occurrences {
        if let Some(index) = base.occurrences.iter().position(|item| item.id == occurrence.id) {
            base.occurrences[index] = occurrence;
        } else {
            base.occurrences.push(occurrence);
        }
    }
    base
}

pub fn validate(artifact: &ExpressionArtifact, text: &str, morphemes: &[crate::model::MorphemeToken]) -> Result<(), String> {
    let length = text.chars().count();
    for occurrence in &artifact.occurrences {
        let [start, end] = occurrence.char_range;
        if start >= end || end > length {
            return Err(format!("表达 occurrence {} 范围无效", occurrence.id));
        }
        if occurrence.morpheme_indices.iter().any(|index| *index >= morphemes.len()) {
            return Err(format!("表达 occurrence {} 的 token 引用无效", occurrence.id));
        }
    }
    Ok(())
}
