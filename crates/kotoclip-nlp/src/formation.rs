use serde::{Deserialize, Serialize};

use crate::entities::{CandidateStatus, Capture, SpanLayer, SpanNode};
use crate::LexemeToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormationAtom {
    pub literal: Option<String>,
    pub pos_major: Option<String>,
    pub conjugation_form: Option<String>,
    pub capture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormationRule {
    pub id: String,
    pub kind: String,
    pub sequence: Vec<FormationAtom>,
    pub output_kind: String,
    pub boundary_effect: String,
}

pub fn match_rule(rule: &FormationRule, tokens: &[LexemeToken]) -> Vec<SpanNode> {
    if rule.sequence.is_empty() || tokens.len() < rule.sequence.len() {
        return Vec::new();
    }
    tokens
        .windows(rule.sequence.len())
        .enumerate()
        .filter_map(|(start, window)| {
            let mut captures = Vec::new();
            for (atom, token) in rule.sequence.iter().zip(window) {
                if let Some(literal) = &atom.literal {
                    if token.surface_forms.first().is_none_or(|surface| surface != literal) {
                        return None;
                    }
                }
                if let Some(pos_major) = &atom.pos_major {
                    if token.pos_profile.first().and_then(|pos| pos.first()) != Some(pos_major) {
                        return None;
                    }
                }
                if let Some(capture_name) = &atom.capture {
                    captures.push(Capture {
                        name: capture_name.clone(),
                        morpheme_ids: vec![token.source_token_ids[0]],
                        char_range: token.char_range,
                        surface: token.surface_forms[0].clone(),
                    });
                }
                if let Some(form) = &atom.conjugation_form {
                    if token.inflection.first().map(|(_, actual)| actual) != Some(form) {
                        return None;
                    }
                }
            }
            let char_range = (window.first()?.char_range.0, window.last()?.char_range.1);
            let morpheme_ids = window.iter().flat_map(|token| token.source_token_ids.clone()).collect();
            Some(SpanNode {
                id: format!("formation:{}:{}-{}", rule.id, char_range.0, char_range.1),
                layer: SpanLayer::Formation,
                kind: rule.output_kind.clone(),
                provider_id: None,
                morpheme_ids,
                char_range,
                matched_range: Some(char_range),
                covered_range: Some(char_range),
                display_range: Some(char_range),
                captures,
                evidence: vec![format!("rule:{}", rule.id)],
                counter_evidence: Vec::new(),
                confidence: 100,
                status: CandidateStatus::Accepted,
                relations: Vec::new(),
                boundary_effect: rule.boundary_effect.clone(),
                rule_ref: Some(rule.id.clone()),
                source_refs: Vec::new(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_feature;
    use crate::alignment::project_lexemes;

    #[test]
    fn matches_action_plus_fixed_suffix() {
        let raw = vec![
            parse_feature("cwj", "冷やし", "動詞,一般,*,*,五段,連用形,ヒヤス,冷やす,冷やし,ヒヤシ,冷やす,ヒヤス,和", (0, 3)),
            parse_feature("cwj", "神", "名詞,普通名詞,一般,*,*,*,カミ,神,神,カミ,神,カミ,和", (3, 4)),
        ];
        let tokens = project_lexemes(&raw);
        let rule = FormationRule {
            id: "deity_by_action".to_string(),
            kind: "word_formation".to_string(),
            sequence: vec![
                FormationAtom { literal: None, pos_major: Some("動詞".to_string()), conjugation_form: Some("連用形".to_string()), capture: Some("action".to_string()) },
                FormationAtom { literal: Some("神".to_string()), pos_major: None, conjugation_form: None, capture: None },
            ],
            output_kind: "derived_noun".to_string(),
            boundary_effect: "merge".to_string(),
        };
        let matches = match_rule(&rule, &tokens);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].char_range, (0, 4));
        assert_eq!(matches[0].captures[0].name, "action");
    }
}
