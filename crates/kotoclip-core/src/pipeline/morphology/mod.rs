use crate::models::{Bunsetsu, MorphologyArtifact, MorphologyChain, MorphologyOperator};

pub const ANALYZER_VERSION: &str = "morphology-1";

pub fn analyze_bunsetsu(bunsetsu: &Bunsetsu, global_offset: usize) -> MorphologyArtifact {
    let mut chains = Vec::new();
    let morphemes = &bunsetsu.morphemes;
    let mut index = 0;

    while index < morphemes.len() {
        let current = &morphemes[index];
        let suffix_chain = index > 0
            && current.pos.major == "動詞"
            && current.pos.sub1 == "接尾";
        if !suffix_chain
            && !matches!(current.pos.major.as_str(), "動詞" | "形容詞" | "助動詞")
        {
            index += 1;
            continue;
        }

        let start = if suffix_chain { index - 1 } else { index };
        let anchor = &morphemes[start];
        let mut end = index + 1;
        while end < morphemes.len() {
            let next = &morphemes[end];
            let attaches = next.pos.major == "助動詞"
                || (next.pos.major == "動詞" && next.pos.sub1 == "接尾")
                || (next.pos.major == "助詞"
                    && next.pos.sub1 == "接続助詞"
                    && matches!(next.base_form.as_str(), "て" | "で" | "ば"));
            if !attaches {
                break;
            }
            end += 1;
        }

        let mut operators = Vec::new();
        let mut connection_forms = Vec::new();
        let mut feature_candidates = Vec::new();
        let mut evidence = vec![format!(
            "ipadic_anchor:{}:{}",
            anchor.conjugation_type, anchor.conjugation_form
        )];

        if anchor.conjugation_form.contains("仮定") {
            push_operator(
                &mut operators,
                global_offset + start,
                anchor.char_range,
                "inflection_form",
                "conditional",
                "morphology.mood.conditional",
                95,
                &anchor.conjugation_form,
                &[],
            );
        }
        let has_explicit_volitional_marker = morphemes
            .get(start + 1)
            .is_some_and(|next| matches!(next.base_form.as_str(), "う" | "よう"));
        if anchor.conjugation_form.contains("未然ウ接続") && !has_explicit_volitional_marker {
            push_operator(
                &mut operators,
                global_offset + start,
                anchor.char_range,
                "modality",
                "volitional",
                "morphology.mood.volitional",
                95,
                &anchor.conjugation_form,
                &[],
            );
        }

        for local in start + 1..end {
            let morpheme = &morphemes[local];
            let global = global_offset + local;
            match morpheme.base_form.as_str() {
                "て" => {
                    connection_forms.push("te_form".to_string());
                    feature_candidates.push("morphology.form.te".to_string());
                    evidence.push("connector:て".to_string());
                    push_operator(
                        &mut operators,
                        global,
                        morpheme.char_range,
                        "connection_form",
                        "te_form",
                        "morphology.form.te",
                        99,
                        "接续助词・て",
                        &[],
                    );
                }
                "で" => {
                    connection_forms.push("de_form".to_string());
                    feature_candidates.push("morphology.form.de".to_string());
                    evidence.push("connector:で".to_string());
                    push_operator(
                        &mut operators,
                        global,
                        morpheme.char_range,
                        "connection_form",
                        "de_form",
                        "morphology.form.de",
                        99,
                        "接续助词・で",
                        &[],
                    );
                }
                "ば" => {
                    connection_forms.push("conditional_ba".to_string());
                    feature_candidates.push("morphology.mood.conditional".to_string());
                    evidence.push("connector:ば".to_string());
                }
                "た" if !morpheme.conjugation_form.contains("仮定") => push_operator(
                    &mut operators,
                    global,
                    morpheme.char_range,
                    "tense",
                    "past",
                    "morphology.tense.past",
                    99,
                    "助动词・た",
                    &[],
                ),
                "ない" | "ぬ" | "ん" if morpheme.pos.major == "助動詞" => push_operator(
                    &mut operators,
                    global,
                    morpheme.char_range,
                    "polarity",
                    "negative",
                    "morphology.polarity.negative",
                    99,
                    "否定助动词",
                    &[],
                ),
                "ます" => push_operator(
                    &mut operators,
                    global,
                    morpheme.char_range,
                    "politeness",
                    "politeness_masu",
                    "morphology.politeness.masu",
                    99,
                    "助动词・ます",
                    &[],
                ),
                "たい" => push_operator(
                    &mut operators,
                    global,
                    morpheme.char_range,
                    "modality",
                    "desire",
                    "morphology.modality.desire",
                    96,
                    "助动词・たい",
                    &[],
                ),
                "う" | "よう" => {
                    let conjectural = matches!(anchor.base_form.as_str(), "です" | "だ")
                        || matches!(anchor.surface.as_str(), "でしょ" | "だろ");
                    push_operator(
                        &mut operators,
                        global,
                        morpheme.char_range,
                        "modality",
                        if conjectural { "conjectural" } else { "volitional" },
                        if conjectural {
                            "morphology.modality.conjectural"
                        } else {
                            "morphology.mood.volitional"
                        },
                        98,
                        if conjectural { "推量助动词" } else { "意向助动词" },
                        &[],
                    );
                }
                "せる" | "させる" | "す" if morpheme.pos.sub1 == "接尾" => push_operator(
                    &mut operators,
                    global,
                    morpheme.char_range,
                    "voice",
                    "causative",
                    "morphology.voice.causative",
                    99,
                    "动词性接尾・使役",
                    &[],
                ),
                "れる" | "られる" => push_operator(
                    &mut operators,
                    global,
                    morpheme.char_range,
                    "voice",
                    "passive_potential",
                    "morphology.voice.passive_potential",
                    72,
                    "动词性接尾・られる",
                    &["passive", "potential", "honorific", "spontaneous"],
                ),
                "がる" => push_operator(
                    &mut operators,
                    global,
                    morpheme.char_range,
                    "modality",
                    "observable_tendency",
                    "morphology.modality.garu",
                    96,
                    "动词性接尾・がる",
                    &[],
                ),
                "やす" => push_operator(
                    &mut operators,
                    global,
                    morpheme.char_range,
                    "modality",
                    "ease",
                    "morphology.modality.easy",
                    96,
                    "接尾助动词・やすい",
                    &[],
                ),
                _ => {}
            }
            if morpheme.conjugation_form.contains("仮定")
                && !operators.iter().any(|item| item.output_state == "conditional")
            {
                push_operator(
                    &mut operators,
                    global,
                    morpheme.char_range,
                    "inflection_form",
                    "conditional",
                    "morphology.mood.conditional",
                    95,
                    &morpheme.conjugation_form,
                    &[],
                );
            }
        }

        operators.sort_by_key(|item| item.source_morpheme_range.0);
        feature_candidates.extend(operators.iter().map(|item| item.concept_id.clone()));
        feature_candidates.sort();
        feature_candidates.dedup();
        let source_ranges = (start..end)
            .map(|local| morphemes[local].char_range)
            .collect::<Vec<_>>();
        chains.push(MorphologyChain {
            chain_id: format!("morph:{}:{}", anchor.char_range.0, anchor.char_range.1),
            anchor_morpheme: global_offset + start,
            base_lexeme: anchor.base_form.clone(),
            source_ranges,
            operators,
            connection_forms,
            feature_candidates,
            evidence,
        });
        index = end.max(index + 1);
    }

    MorphologyArtifact {
        chains,
        unclassified: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_operator(
    target: &mut Vec<MorphologyOperator>,
    global_morpheme: usize,
    char_range: (usize, usize),
    kind: &str,
    output_state: &str,
    concept_id: &str,
    confidence: u8,
    evidence: &str,
    candidates: &[&str],
) {
    if target.iter().any(|item| {
        item.concept_id == concept_id && item.source_morpheme_range.0 == global_morpheme
    }) {
        return;
    }
    target.push(MorphologyOperator {
        operator_id: format!("{}:{}", concept_id, char_range.0),
        kind: kind.to_string(),
        source_morpheme_range: (global_morpheme, global_morpheme + 1),
        char_range,
        input_requirement: None,
        output_state: output_state.to_string(),
        concept_id: concept_id.to_string(),
        confidence,
        evidence: vec![evidence.to_string()],
        candidates: candidates.iter().map(|item| (*item).to_string()).collect(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Bunsetsu, HeadWord, Morpheme, PosTag};

    fn morpheme(surface: &str, base: &str, major: &str, sub1: &str, form: &str, start: usize) -> Morpheme {
        Morpheme {
            surface: surface.to_string(),
            base_form: base.to_string(),
            reading: String::new(),
            pos: PosTag { major: major.to_string(), sub1: sub1.to_string(), sub2: "*".to_string(), sub3: "*".to_string() },
            conjugation_type: String::new(),
            conjugation_form: form.to_string(),
            char_range: (start, start + surface.chars().count()),
        }
    }

    #[test]
    fn builds_reversible_operator_chain() {
        let morphemes = vec![
            morpheme("行か", "行く", "動詞", "自立", "未然形", 0),
            morpheme("せ", "せる", "動詞", "接尾", "未然形", 2),
            morpheme("られ", "られる", "動詞", "接尾", "連用形", 3),
            morpheme("なかっ", "ない", "助動詞", "*", "連用タ接続", 5),
            morpheme("た", "た", "助動詞", "*", "基本形", 8),
        ];
        let bunsetsu = Bunsetsu {
            surface: "行かせられなかった".to_string(),
            head_word: HeadWord { surface: "行か".to_string(), base_form: "行く".to_string(), reading: String::new(), pos: morphemes[0].pos.clone() },
            morphemes,
            grammar_tags: Vec::new(), morphology: MorphologyArtifact::default(), grammar_occurrences: Vec::new(), functional_residuals: Vec::new(),
            word_formations: Vec::new(), lexical_units: Vec::new(), function: None, char_range: (0, 9),
        };
        let artifact = analyze_bunsetsu(&bunsetsu, 0);
        let states = artifact.chains[0].operators.iter().map(|item| item.output_state.as_str()).collect::<Vec<_>>();
        assert_eq!(states, vec!["causative", "passive_potential", "negative", "past"]);
    }
}
