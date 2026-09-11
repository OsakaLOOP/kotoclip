//! L3 语法候选层。识别结果由外部目录或人工审阅提供，本模块不生成规则。
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.grammar-artifact.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GrammarStatus { Observed, Candidate, Pending, Rejected }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrammarOccurrence {
    pub id: String,
    pub concept_id: Option<String>,
    pub char_range: [usize; 2],
    pub morpheme_indices: Vec<usize>,
    pub status: GrammarStatus,
    pub provider: String,
    pub source_id: Option<String>,
    pub labels: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrammarArtifact {
    pub schema: String,
    pub occurrences: Vec<GrammarOccurrence>,
}

pub fn empty() -> GrammarArtifact {
    GrammarArtifact { schema: SCHEMA.into(), occurrences: Vec::new() }
}

/// 从 UniDic 身份字段产生功能语素候选；候选不包含语义义项和规则判断。
pub fn collect_functional_candidates(
    source_tokens: &[crate::model::ProviderToken],
    morphemes: &[crate::model::MorphemeToken],
) -> Result<GrammarArtifact, String> {
    if source_tokens.len() != morphemes.len() {
        return Err("功能语素输入的来源 token 与统一 token 数量不一致".into());
    }
    let mut occurrences = Vec::new();
    for (index, (source, morpheme)) in source_tokens.iter().zip(morphemes).enumerate() {
        let pos1 = source.fields.get(0).and_then(|field| field.value.as_deref()).unwrap_or("");
        let pos2 = source.fields.get(1).and_then(|field| field.value.as_deref()).unwrap_or("");
        if !matches!(pos1, "助詞" | "助動詞") && pos2 != "非自立" { continue; }
        let lemma = source.fields.get(7).and_then(|field| field.value.clone()).filter(|value| !value.trim().is_empty());
        let mut labels = vec!["functional_morpheme".to_owned(), format!("pos1:{pos1}")];
        if !pos2.is_empty() { labels.push(format!("pos2:{pos2}")); }
        let mut evidence = vec!["provider:unidic".to_owned(), format!("surface:{}", morpheme.surface)];
        if let Some(value) = &lemma { evidence.push(format!("lemma:{value}")); }
        occurrences.push(GrammarOccurrence {
            id: format!("grammar:functional:m{index}"), concept_id: None, char_range: morpheme.char_range,
            morpheme_indices: vec![index], status: GrammarStatus::Candidate, provider: "unidic".into(),
            source_id: Some(format!("m{index}")), labels, evidence,
        });
    }
    Ok(GrammarArtifact { schema: SCHEMA.into(), occurrences })
}

/// 合并目录或人工结果，外部同 ID occurrence 覆盖基础身份候选。
pub fn merge(mut base: GrammarArtifact, overlay: GrammarArtifact) -> GrammarArtifact {
    for occurrence in overlay.occurrences {
        if let Some(index) = base.occurrences.iter().position(|item| item.id == occurrence.id) {
            base.occurrences[index] = occurrence;
        } else {
            base.occurrences.push(occurrence);
        }
    }
    base
}

pub fn validate(artifact: &GrammarArtifact, text: &str, morphemes: &[crate::model::MorphemeToken]) -> Result<(), String> {
    let length = text.chars().count();
    for occurrence in &artifact.occurrences {
        let [start, end] = occurrence.char_range;
        if start >= end || end > length {
            return Err(format!("语法 occurrence {} 范围无效", occurrence.id));
        }
        if occurrence.morpheme_indices.iter().any(|index| *index >= morphemes.len()) {
            return Err(format!("语法 occurrence {} 的 token 引用无效", occurrence.id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FeatureField, MorphemeToken, ProviderToken};

    #[test]
    fn emits_functional_identity_candidates_without_sense_claims() {
        let mut fields = Vec::new();
        for index in 0..29 { fields.push(FeatureField { index, name: index.to_string(), label: String::new(), raw: None, value: None }); }
        fields[0].value = Some("助詞".into()); fields[1].value = Some("係助詞".into()); fields[7].value = Some("は".into());
        let token = ProviderToken { index: 0, surface: "は".into(), char_range: [0, 1], byte_range: [0, 3], lexicon_type: "known".into(), left_id: 0, right_id: 0, word_cost: 0, total_cost: 0, raw_feature: String::new(), fields };
        let morpheme = MorphemeToken { id: "m0".into(), source_index: 0, surface: "は".into(), char_range: [0, 1], pos: [Some("助詞".into()), Some("係助詞".into()), None, None], lemma: Some("は".into()), reading: None, query_forms: Vec::new() };
        let artifact = collect_functional_candidates(&[token], &[morpheme]).unwrap();
        assert_eq!(artifact.occurrences.len(), 1);
        assert_eq!(artifact.occurrences[0].concept_id, None);
        assert_eq!(artifact.occurrences[0].status, GrammarStatus::Candidate);
    }
}
