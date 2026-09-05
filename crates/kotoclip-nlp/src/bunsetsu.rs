use serde::{Deserialize, Serialize};

use crate::entities::{CandidateStatus, SpanLayer, SpanNode};
use crate::LexemeToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BunsetsuCandidate {
    pub span: SpanNode,
    pub function: String,
}

pub fn segment(tokens: &[LexemeToken], formations: &[SpanNode]) -> Vec<BunsetsuCandidate> {
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut groups: Vec<Vec<&LexemeToken>> = Vec::new();
    for token in tokens {
        let attach = groups.last().is_some_and(|group| is_functional(token) && !group.is_empty());
        if attach {
            groups.last_mut().expect("checked above").push(token);
        } else {
            groups.push(vec![token]);
        }
    }
    groups
        .into_iter()
        .enumerate()
        .map(|(index, group)| {
            let start = group.first().expect("non-empty group");
            let end = group.last().expect("non-empty group");
            let char_range = (start.char_range.0, end.char_range.1);
            let morpheme_ids = group.iter().flat_map(|token| token.source_token_ids.clone()).collect::<Vec<_>>();
            let contained_formation = formations.iter().filter(|formation| {
                formation.char_range.0 >= char_range.0 && formation.char_range.1 <= char_range.1
            }).count();
            let function = if group.iter().any(|token| token.pos_profile.iter().any(|pos| pos[0] == "動詞" || pos[0] == "形容詞")) {
                "predicate"
            } else if group.iter().any(|token| token.pos_profile.iter().any(|pos| pos[0] == "助詞")) {
                "case_phrase"
            } else {
                "nominal"
            };
            BunsetsuCandidate {
                span: SpanNode {
                    id: format!("bunsetsu:{}-{}", char_range.0, char_range.1),
                    layer: SpanLayer::Bunsetsu,
                    kind: "bunsetsu_candidate".to_string(),
                    provider_id: None,
                    morpheme_ids,
                    char_range,
                    matched_range: Some(char_range),
                    covered_range: Some(char_range),
                    display_range: Some(char_range),
                    captures: Vec::new(),
                    evidence: vec![format!("functional_attach:{}", group.len()), format!("formations:{}", contained_formation)],
                    counter_evidence: Vec::new(),
                    confidence: 60,
                    status: CandidateStatus::Pending,
                    relations: Vec::new(),
                    boundary_effect: "group".to_string(),
                    rule_ref: None,
                    source_refs: Vec::new(),
                },
                function: function.to_string(),
            }
        })
        .collect()
}

fn is_functional(token: &LexemeToken) -> bool {
    token.pos_profile.iter().any(|pos| matches!(pos[0].as_str(), "助詞" | "助動詞" | "接尾辞" | "接頭辞"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alignment::project_lexemes;
    use crate::parse_feature;

    #[test]
    fn attaches_case_particle_to_content_core() {
        let raw = vec![
            parse_feature("cwj", "警察", "名詞,普通名詞,一般,*,*,*,ケイサツ,警察,警察,ケーサツ,警察,ケーサツ,漢", (0, 2)),
            parse_feature("cwj", "署", "接尾辞,名詞的,一般,*,*,*,ショ,署,署,ショ,署,ショ,漢", (2, 3)),
            parse_feature("cwj", "へ", "助詞,格助詞,*,*,*,*,ヘ,へ,へ,エ,へ,エ,和", (3, 4)),
        ];
        let tokens = project_lexemes(&raw);
        let segments = segment(&tokens, &[]);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].span.char_range, (0, 4));
        assert_eq!(segments[0].function, "case_phrase");
    }
}
