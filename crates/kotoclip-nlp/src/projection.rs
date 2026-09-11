//! L3 到阅读展示层的稳定引用，不包含颜色或 UI 决策。
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.l3-projection.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectionTarget {
    pub id: String,
    pub char_range: [usize; 2],
    pub layer: String,
    pub source_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectionArtifact {
    pub schema: String,
    pub targets: Vec<ProjectionTarget>,
}

pub fn from_layers(grammar: &crate::grammar::GrammarArtifact, expression: &crate::expression::ExpressionArtifact) -> ProjectionArtifact {
    let mut targets = Vec::new();
    targets.extend(grammar.occurrences.iter().map(|item| ProjectionTarget {
        id: format!("grammar:{}", item.id), char_range: item.char_range,
        layer: "grammar".into(), source_id: item.source_id.clone().unwrap_or_else(|| item.id.clone()),
        status: serde_json::to_value(&item.status).ok().and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_else(|| "pending".into()),
    }));
    targets.extend(expression.occurrences.iter().map(|item| ProjectionTarget {
        id: format!("expression:{}", item.id), char_range: item.char_range,
        layer: "expression".into(), source_id: item.source_id.clone().unwrap_or_else(|| item.id.clone()),
        status: serde_json::to_value(&item.status).ok().and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_else(|| "pending".into()),
    }));
    ProjectionArtifact { schema: SCHEMA.into(), targets }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_only_declared_occurrences() {
        let grammar = crate::grammar::GrammarArtifact {
            schema: crate::grammar::SCHEMA.into(),
            occurrences: vec![crate::grammar::GrammarOccurrence {
                id: "g1".into(), concept_id: Some("c1".into()), char_range: [0, 2],
                morpheme_indices: vec![0], status: crate::grammar::GrammarStatus::Candidate,
                provider: "catalog".into(), source_id: None, labels: Vec::new(), evidence: vec!["manual".into()],
            }],
        };
        let projection = from_layers(&grammar, &crate::expression::empty());
        assert_eq!(projection.targets.len(), 1);
        assert_eq!(projection.targets[0].id, "grammar:g1");
        assert_eq!(projection.targets[0].status, "candidate");
    }
}
