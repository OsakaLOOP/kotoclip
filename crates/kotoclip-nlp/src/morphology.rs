//! L2 形态链层。
//!
//! 该层只整理 UniDic 已提供的词形、活用和连接字段，不根据表面字符串推导语法规则。
use crate::model::MorphemeToken;
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.morphology-artifact.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MorphologyRole { Lexical, Functional }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MorphologyOperator {
    pub operator_id: String,
    pub kind: String,
    pub source_morpheme_range: [usize; 2],
    pub char_range: [usize; 2],
    pub output_state: String,
    pub concept_id: String,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub candidates: Vec<String>,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MorphologyChain {
    pub chain_id: String,
    pub anchor_morpheme: usize,
    pub anchor_range: [usize; 2],
    pub morpheme_range: [usize; 2],
    pub char_range: [usize; 2],
    pub role: MorphologyRole,
    pub base_lexeme: String,
    pub surface_form: String,
    pub dictionary_form: String,
    pub lemma_form: String,
    pub lookup_form: String,
    pub source_ranges: Vec<[usize; 2]>,
    pub operators: Vec<MorphologyOperator>,
    pub connection_forms: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MorphologyArtifact {
    pub schema: String,
    pub chains: Vec<MorphologyChain>,
}

fn value(token: &crate::model::ProviderToken, index: usize) -> Option<String> {
    token.fields.get(index).and_then(|field| field.value.clone()).filter(|item| !item.trim().is_empty())
}

fn first(value: Option<String>, fallback: &str) -> String { value.unwrap_or_else(|| fallback.to_owned()) }

/// 从规范 UniDic token 生成可逆形态链。每条链保留一个原子 token，复合活用由后续阶段按连接字段组合。
pub fn collect(source_tokens: &[crate::model::ProviderToken], morphemes: &[MorphemeToken]) -> Result<MorphologyArtifact, String> {
    if source_tokens.len() != morphemes.len() {
        return Err("形态链输入的来源 token 与统一 token 数量不一致".into());
    }
    let mut chains = Vec::new();
    for (index, (source, morpheme)) in source_tokens.iter().zip(morphemes).enumerate() {
        let pos_major = value(source, 0).unwrap_or_default();
        if pos_major == "記号" { continue; }
        let lemma = first(value(source, 7), &morpheme.surface);
        let dictionary_form = first(value(source, 10), &lemma);
        let lemma_form = first(value(source, 6), &lemma);
        let lookup_form = dictionary_form.clone();
        let role = if matches!(pos_major.as_str(), "助詞" | "助動詞") { MorphologyRole::Functional } else { MorphologyRole::Lexical };
        let mut operators = Vec::new();
        for (kind, type_index, form_index, label) in [
            ("conjugation", 4usize, 5usize, "活用"),
            ("final_conjugation", 15usize, 16usize, "終端活用"),
        ] {
            let type_value = value(source, type_index);
            let form_value = value(source, form_index);
            if type_value.is_none() && form_value.is_none() { continue; }
            let output_state = form_value.clone().or(type_value.clone()).unwrap_or_default();
            let detail = [type_value.clone(), form_value.clone()].into_iter().flatten().collect::<Vec<_>>().join("/");
            operators.push(MorphologyOperator {
                operator_id: format!("morphology:{index}:{kind}"), kind: kind.into(),
                source_morpheme_range: [index, index + 1], char_range: morpheme.char_range,
                output_state, concept_id: "morphology_feature".into(), confidence: 1.0,
                evidence: vec![format!("unidic:{label}:{detail}")], candidates: Vec::new(),
                label: detail.clone(), description: format!("UniDic {label}字段：{detail}"),
            });
        }
        let connection_forms = [value(source, 17), value(source, 18)].into_iter().flatten().collect();
        let mut evidence = vec!["provider:unidic".to_owned(), format!("pos1:{pos_major}")];
        if let Some(form) = value(source, 5) { evidence.push(format!("cForm:{form}")); }
        chains.push(MorphologyChain {
            chain_id: format!("morphology:m{index}"), anchor_morpheme: index,
            anchor_range: morpheme.char_range, morpheme_range: [index, index + 1], char_range: morpheme.char_range,
            role, base_lexeme: lemma.clone(), surface_form: morpheme.surface.clone(),
            dictionary_form, lemma_form, lookup_form, source_ranges: vec![morpheme.char_range],
            operators, connection_forms, evidence,
        });
    }
    Ok(MorphologyArtifact { schema: SCHEMA.into(), chains })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FeatureField, MorphemeToken, ProviderToken};

    fn source(fields: &[(&str, &str)]) -> ProviderToken {
        let mut values = vec![None; 29];
        for (name, value) in fields {
            if let Some(index) = crate::model::FIELD_NAMES.iter().position(|item| item == name) {
                values[index] = Some((*value).into());
            }
        }
        ProviderToken { index: 0, surface: "読ん".into(), char_range: [0, 2], byte_range: [0, 6], lexicon_type: "known".into(), left_id: 0, right_id: 0, word_cost: 0, total_cost: 0, raw_feature: String::new(), fields: values.into_iter().enumerate().map(|(index, value)| FeatureField { index, name: index.to_string(), label: String::new(), raw: value.clone(), value }).collect() }
    }

    #[test]
    fn preserves_unidic_inflection_and_functional_role() {
        let token = source(&[("pos1", "動詞"), ("pos2", "自立"), ("pos3", ""), ("pos4", ""), ("cType", "五段-ラ行"), ("cForm", "連用形-撥音便"), ("lForm", "ヨム"), ("lemma", "読む"), ("orth", "読ん"), ("orthBase", "読む"), ("iConType", "動詞接続")]);
        let morpheme = MorphemeToken { id: "m0".into(), source_index: 0, surface: "読ん".into(), char_range: [0, 2], pos: [Some("動詞".into()), None, None, None], lemma: Some("読む".into()), reading: None, query_forms: Vec::new() };
        let artifact = collect(&[token], &[morpheme]).unwrap();
        assert_eq!(artifact.chains[0].dictionary_form, "読む");
        assert_eq!(artifact.chains[0].operators[0].kind, "conjugation");
        assert!(matches!(artifact.chains[0].role, MorphologyRole::Lexical));
    }
}
