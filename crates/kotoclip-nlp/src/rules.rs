//! 基于正式词元和 UniDic 字段的类型化规则匹配。
use crate::model::{MorphemeToken, ProviderToken};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Atom {
    pub surfaces: Vec<String>,
    pub base_forms: Vec<String>,
    pub pos_major: Vec<String>,
    pub pos_sub1: Vec<String>,
    pub conjugation_types: Vec<String>,
    pub conjugation_forms: Vec<String>,
    pub morphology_features: Vec<String>,
    pub capture: Option<String>,
    pub optional: bool,
    pub gap_before: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub label: String,
    #[serde(default)] pub description: String,
    pub kind: String,
    pub atoms: Vec<Atom>,
    #[serde(default)] pub priority: i32,
    #[serde(default = "enabled")] pub enabled: bool,
    #[serde(default)] pub document_id: Option<String>,
    #[serde(default)] pub allow_whitespace: bool,
    #[serde(default)] pub concept_id: Option<String>,
    #[serde(default)] pub sense_ids: Vec<String>,
    #[serde(default)] pub display_from: usize,
    #[serde(default)] pub display_to: Option<usize>,
    #[serde(default)] pub source_refs: Vec<String>,
    #[serde(default)] pub gap_after_atom: Option<usize>,
    #[serde(default)] pub gap_bunsetsu: Option<[usize; 2]>,
}
fn enabled() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuleMatch {
    pub rule_id: String,
    pub members: Vec<usize>,
    pub char_range: [usize; 2],
    pub hit_ranges: Vec<[usize; 2]>,
    pub captures: BTreeMap<String, Vec<usize>>,
}

impl Rule {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() || self.label.trim().is_empty() || self.atoms.is_empty() || self.atoms.len() > 32 {
            return Err("规则需要身份、名称和 1 至 32 个条件".into());
        }
        if !matches!(self.kind.as_str(), "idiom" | "grammar_construction" | "correlative" | "lexical_unit" | "functional_morpheme" | "morphology_feature") {
            return Err(format!("未知规则类型：{}", self.kind));
        }
        if self.atoms.iter().all(|a| a.optional) || self.display_from >= self.atoms.len()
            || self.display_to.is_some_and(|end| end <= self.display_from || end > self.atoms.len()) {
            return Err("规则的必选成员或显示范围无效".into());
        }
        if self.gap_after_atom.is_some_and(|index| index + 1 >= self.atoms.len())
            || self.gap_bunsetsu.is_some_and(|range| range[0] > range[1] || range[1] > 64) {
            return Err("非连续规则的间隔位置或文节数量无效".into());
        }
        let mut captures = std::collections::BTreeSet::new();
        for atom in &self.atoms {
            if atom.gap_before > 128 { return Err("间隔最多允许 128 个词元".into()); }
            if let Some(name) = &atom.capture {
                if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || !captures.insert(name) {
                    return Err("捕获名称必须唯一且使用字母、数字或下划线".into());
                }
            }
        }
        Ok(())
    }
}

pub fn respects_bunsetsu_gap(
    rule: &Rule,
    matched: &RuleMatch,
    bunsetsu: &crate::bunsetsu::BunsetsuArtifact,
) -> bool {
    let (Some(gap_after), Some([minimum, maximum])) = (rule.gap_after_atom, rule.gap_bunsetsu) else {
        return true;
    };
    let Some(&left) = matched.members.get(gap_after) else { return false; };
    let Some(&right) = matched.members.get(gap_after + 1) else { return false; };
    let between = bunsetsu.nodes.iter().filter(|node| {
        let Some(&first) = node.morpheme_indices.first() else { return false; };
        let Some(&last) = node.morpheme_indices.last() else { return false; };
        first > left && last < right
    }).count();
    between >= minimum && between <= maximum
}

fn any(values: &[String], actual: &str) -> bool { values.is_empty() || values.iter().any(|v| v == actual) }
fn field(token: &ProviderToken, index: usize) -> &str { token.fields.get(index).and_then(|f| f.value.as_deref()).unwrap_or("") }

pub fn atom_matches(atom: &Atom, token: &MorphemeToken, source: &ProviderToken) -> bool {
    any(&atom.surfaces, &token.surface)
        && (atom.base_forms.is_empty() || atom.base_forms.iter().any(|v| token.query_forms.iter().any(|f| &f.form == v) || token.lemma.as_ref() == Some(v)))
        && any(&atom.pos_major, field(source, 0)) && any(&atom.pos_sub1, field(source, 1))
        && (atom.conjugation_types.is_empty() || atom.conjugation_types.iter().any(|v| field(source, 4).starts_with(v)))
        && (atom.conjugation_forms.is_empty() || atom.conjugation_forms.iter().any(|v| field(source, 5).starts_with(v)))
        && atom.morphology_features.iter().all(|v| crate::morphology::features(source).contains(v))
}

pub fn hard_boundary(c: char) -> bool { matches!(c, '。' | '！' | '？' | '!' | '?' | '\n' | '\r' | '「' | '」' | '『' | '』' | '“' | '”' | '"') }

pub fn connected(text: &[char], tokens: &[MorphemeToken], left: usize, right: usize, whitespace: bool) -> bool {
    let start = tokens[left].char_range[0];
    let end = tokens[right].char_range[1];
    if start >= end || end > text.len() || text[start..end].iter().any(|&c| hard_boundary(c)) { return false; }
    for pair in tokens[left..=right].windows(2) {
        let a = pair[0].char_range[1]; let b = pair[1].char_range[0];
        if a > b || (a != b && (!whitespace || !text[a..b].iter().all(|c| c.is_whitespace()))) { return false; }
    }
    true
}

pub fn matches(rule: &Rule, text: &str, tokens: &[MorphemeToken], sources: &[ProviderToken]) -> Result<Vec<RuleMatch>, String> {
    rule.validate()?;
    if tokens.len() != sources.len() { return Err("规则的来源词元数量不一致".into()); }
    if !rule.enabled { return Ok(Vec::new()); }
    let chars: Vec<_> = text.chars().collect();
    let mut results = Vec::new();
    for start in 0..tokens.len() {
        let mut states = vec![(start, Vec::<(usize, usize)>::new())];
        for (atom_index, atom) in rule.atoms.iter().enumerate() {
            let mut next = Vec::new();
            for (cursor, selected) in states {
                if atom.optional { next.push((cursor, selected.clone())); }
                let limit = if selected.is_empty() { cursor + 1 } else { cursor + atom.gap_before + 1 };
                for index in cursor..limit.min(tokens.len()) {
                    if let Some(&(_, previous)) = selected.last() {
                        if !connected(&chars, tokens, previous, index, rule.allow_whitespace) { break; }
                    }
                    if atom_matches(atom, &tokens[index], &sources[index]) {
                        let mut selected = selected.clone(); selected.push((atom_index, index));
                        next.push((index + 1, selected));
                    }
                }
            }
            if next.len() > 4096 { return Err(format!("规则 {} 的候选过多，请收紧条件", rule.id)); }
            states = next;
        }
        for (_, selected) in states {
            if selected.is_empty() { continue; }
            let mut captures = BTreeMap::new();
            for &(atom, token) in &selected {
                if let Some(name) = &rule.atoms[atom].capture { captures.insert(name.clone(), vec![token]); }
            }
            let members: Vec<_> = selected.iter().filter(|(a, _)| *a >= rule.display_from && *a < rule.display_to.unwrap_or(rule.atoms.len())).map(|(_, t)| *t).collect();
            if members.is_empty() { continue; }
            let hit_ranges: Vec<_> = members.iter().map(|&i| tokens[i].char_range).collect();
            let found = RuleMatch { rule_id: rule.id.clone(), char_range: [hit_ranges[0][0], hit_ranges.last().unwrap()[1]], members, hit_ranges, captures };
            if !results.contains(&found) { results.push(found); }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FeatureField, QueryForm};

    fn pair(index: usize, surface: &str, lemma: &str, range: [usize; 2], pos: &str) -> (MorphemeToken, ProviderToken) {
        let mut fields = (0..29).map(|field| FeatureField { index: field, name: field.to_string(), label: String::new(), raw: None, value: None }).collect::<Vec<_>>();
        fields[0].value = Some(pos.into());
        fields[7].value = Some(lemma.into());
        (MorphemeToken { id: format!("m{index}"), source_index: index, surface: surface.into(), char_range: range,
            pos: [Some(pos.into()), None, None, None], lemma: Some(lemma.into()), reading: None,
            query_forms: vec![QueryForm { kind: "lemma".into(), form: lemma.into(), reading: None, reading_field: None }] },
         ProviderToken { index, surface: surface.into(), char_range: range, byte_range: range, lexicon_type: "known".into(),
            left_id: 0, right_id: 0, word_cost: 0, total_cost: 0, raw_feature: String::new(), fields })
    }

    fn rule() -> Rule {
        Rule { id: "user.test".into(), label: "决して〜ない".into(), description: String::new(), kind: "correlative".into(),
            atoms: vec![Atom { base_forms: vec!["決して".into()], ..Default::default() },
                Atom { base_forms: vec!["ない".into()], gap_before: 4, ..Default::default() }], priority: 1, enabled: true,
            document_id: None, allow_whitespace: false, concept_id: None, sense_ids: Vec::new(), display_from: 0,
            display_to: None, source_refs: Vec::new(), gap_after_atom: Some(0), gap_bunsetsu: Some([0, 3]) }
    }

    #[test]
    fn matches_non_contiguous_members_and_preserves_hit_ranges() {
        let pairs = [pair(0, "決して", "決して", [0, 3], "副詞"), pair(1, "忘れ", "忘れる", [3, 5], "動詞"),
            pair(2, "ない", "ない", [5, 7], "助動詞")];
        let tokens = pairs.iter().map(|pair| pair.0.clone()).collect::<Vec<_>>();
        let sources = pairs.iter().map(|pair| pair.1.clone()).collect::<Vec<_>>();
        let found = matches(&rule(), "決して忘れない", &tokens, &sources).unwrap();
        assert_eq!(found[0].members, vec![0, 2]);
        assert_eq!(found[0].hit_ranges, vec![[0, 3], [5, 7]]);
    }

    #[test]
    fn rejects_sentence_boundary_inside_gap() {
        let pairs = [pair(0, "決して", "決して", [0, 3], "副詞"), pair(1, "。", "。", [3, 4], "補助記号"),
            pair(2, "ない", "ない", [4, 6], "助動詞")];
        let tokens = pairs.iter().map(|pair| pair.0.clone()).collect::<Vec<_>>();
        let sources = pairs.iter().map(|pair| pair.1.clone()).collect::<Vec<_>>();
        assert!(matches(&rule(), "決して。ない", &tokens, &sources).unwrap().is_empty());
    }
}
