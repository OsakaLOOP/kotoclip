//! L2 形态链层。
//!
//! 该层只整理 UniDic 已提供的词形、活用和连接字段，不根据表面字符串推导语法规则。
use crate::model::MorphemeToken;
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "kotoclip.morphology-artifact.v2";

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
    #[serde(default)]
    pub input_state: String,
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
    #[serde(default)]
    pub display_form: String,
    #[serde(default)]
    pub parent_chain_id: Option<String>,
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

/// 先保存原子活用，再按连接形和字符连续性建立所有权。
pub fn collect(source_tokens: &[crate::model::ProviderToken], morphemes: &[MorphemeToken]) -> Result<MorphologyArtifact, String> {
    if source_tokens.len() != morphemes.len() {
        return Err("形态链输入的来源 token 与统一 token 数量不一致".into());
    }
    let mut chains = Vec::new();
    for (index, (source, morpheme)) in source_tokens.iter().zip(morphemes).enumerate() {
        let pos_major = value(source, 0).unwrap_or_default();
        if matches!(pos_major.as_str(), "記号" | "補助記号" | "空白") { continue; }
        let lemma = first(value(source, 7), &morpheme.surface);
        let dictionary_form = first(value(source, 10), &lemma);
        let lemma_form = lemma.clone();
        let lookup_form = dictionary_form.clone();
        let role = if matches!(pos_major.as_str(), "助詞" | "助動詞") { MorphologyRole::Functional } else { MorphologyRole::Lexical };
        let mut operators = Vec::new();
        for (kind, type_index, form_index, label) in [
            ("conjugation", 4usize, 5usize, "活用"),
            ("initial_alternation", 13usize, 14usize, "词首变化"),
            ("final_alternation", 15usize, 16usize, "词尾变化"),
        ] {
            let type_value = value(source, type_index);
            let form_value = value(source, form_index);
            if type_value.is_none() && form_value.is_none() { continue; }
            let output_state = form_value.clone().or(type_value.clone()).unwrap_or_default();
            let detail = [type_value.clone(), form_value.clone()].into_iter().flatten().collect::<Vec<_>>().join("/");
            operators.push(MorphologyOperator {
                operator_id: format!("morphology:{index}:{kind}"), kind: kind.into(),
                source_morpheme_range: [index, index + 1], char_range: morpheme.char_range,
                output_state, input_state: dictionary_form.clone(), concept_id: "morphology.chain".into(), confidence: 1.0,
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
            display_form: dictionary_form.clone(), parent_chain_id: None,
            dictionary_form, lemma_form, lookup_form, source_ranges: vec![morpheme.char_range],
            operators, connection_forms, evidence,
        });
    }
    let mut combined: Vec<MorphologyChain> = Vec::new();
    for mut chain in chains {
        let index = chain.anchor_morpheme;
        let current = &source_tokens[index];
        let pos = value(current, 0).unwrap_or_default();
        if let Some(previous) = combined.last_mut() {
            let last_index = previous.morpheme_range[1] - 1;
            let last = &source_tokens[last_index];
            let form = value(last, 5).unwrap_or_default();
            let contiguous = previous.char_range[1] == chain.char_range[0];
            let sahen = pos == "動詞" && value(current, 4).is_some_and(|v| v.contains("サ行変格"))
                && value(last, 2).is_some_and(|v| v.contains("サ変"));
            let auxiliary = pos == "助動詞" && (!form.is_empty() || value(last, 0).as_deref() == Some("形状詞"));
            let connector = pos == "助詞" && value(current, 1).as_deref() == Some("接続助詞")
                && matches!(chain.surface_form.as_str(), "て" | "で" | "ば") && !form.is_empty();
            let support = pos == "動詞" && value(current, 1).is_some_and(|v| v.starts_with("非自立"))
                && matches!(last.surface.as_str(), "て" | "で") && value(last, 0).as_deref() == Some("助詞");
            if contiguous && support {
                chain.role = MorphologyRole::Functional;
                chain.parent_chain_id = Some(previous.chain_id.clone());
                chain.evidence.push("connection:接续助词后的补助用言".into());
            } else if contiguous && (sahen || auxiliary || connector) {
                if sahen {
                    previous.dictionary_form.push_str(&chain.dictionary_form);
                    previous.lookup_form = previous.dictionary_form.clone();
                    previous.display_form = previous.dictionary_form.clone();
                    previous.lemma_form.push_str(&chain.lemma_form);
                }
                if value(last, 0).as_deref() == Some("形状詞") && chain.surface_form == "な" {
                    previous.display_form = format!("{}だ", previous.dictionary_form);
                }
                let features = features(current);
                for feature in features {
                    previous.operators.push(MorphologyOperator {
                        operator_id: format!("morphology:{index}:{feature}"), kind: feature.clone(),
                        source_morpheme_range: [index, index + 1], char_range: chain.char_range,
                        input_state: previous.surface_form.clone(), output_state: format!("{}{}", previous.surface_form, chain.surface_form),
                        concept_id: feature_concept(&feature).into(), confidence: 1.0,
                        evidence: chain.evidence.clone(), candidates: if feature == "passive_potential" { vec!["受身".into(), "可能".into(), "尊敬".into(), "自発".into()] } else { Vec::new() },
                        label: feature_label(&feature).into(), description: format!("{}：{}", feature_label(&feature), chain.surface_form),
                    });
                }
                previous.surface_form.push_str(&chain.surface_form);
                previous.morpheme_range[1] = chain.morpheme_range[1];
                previous.char_range[1] = chain.char_range[1];
                previous.source_ranges.extend(chain.source_ranges);
                previous.operators.extend(chain.operators);
                previous.connection_forms.extend(chain.connection_forms);
                previous.evidence.extend(chain.evidence);
                continue;
            }
        }
        combined.push(chain);
    }
    Ok(MorphologyArtifact { schema: SCHEMA.into(), chains: combined })
}

pub fn features(token: &crate::model::ProviderToken) -> Vec<String> {
    let pos = value(token, 0).unwrap_or_default();
    let kind = value(token, 4).unwrap_or_default();
    let form = value(token, 5).unwrap_or_default();
    let mut result = Vec::new();
    if pos == "助動詞" {
        for (needle, feature) in [("助動詞-タ", "past"), ("助動詞-ナイ", "negative"), ("助動詞-ヌ", "negative"),
            ("助動詞-ズ", "negative"), ("助動詞-マス", "politeness_masu"), ("助動詞-タイ", "desire")] {
            if kind == needle { result.push(feature.into()); }
        }
        let lemma = value(token, 7).unwrap_or_default();
        if matches!(lemma.as_str(), "れる" | "られる") { result.push("passive_potential".into()); }
        if matches!(lemma.as_str(), "せる" | "させる" | "しめる") { result.push("causative".into()); }
    }
    if form.starts_with("意志推量形") { result.push("volitional".into()); }
    if form.starts_with("仮定形") { result.push("conditional".into()); }
    if pos == "助詞" && value(token, 1).as_deref() == Some("接続助詞") {
        match token.surface.as_str() { "て" => result.push("te_form".into()), "で" => result.push("de_form".into()), _ => {} }
    }
    result
}

pub fn feature_concept(feature: &str) -> &str {
    match feature {
        "past" => "morphology.tense.past", "negative" => "morphology.polarity.negative",
        "causative" => "morphology.voice.causative", "passive_potential" => "morphology.voice.passive_potential",
        "politeness_masu" => "morphology.politeness.masu", "volitional" => "morphology.mood.volitional",
        "conditional" => "morphology.condition.ba", "te_form" => "morphology.form.te", "de_form" => "morphology.form.de",
        _ => "morphology.chain",
    }
}

fn feature_label(feature: &str) -> &str {
    match feature { "past" => "过去", "negative" => "否定", "causative" => "使役", "passive_potential" => "受身等候选",
        "politeness_masu" => "丁寧", "volitional" => "意向", "conditional" => "条件", "te_form" | "de_form" => "接续", _ => "活用" }
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
