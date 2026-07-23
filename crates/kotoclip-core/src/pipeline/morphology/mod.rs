use crate::models::{
    Bunsetsu, Morpheme, MorphologyArtifact, MorphologyChain, MorphologyChainRole,
    MorphologyOperator,
};

pub const ANALYZER_VERSION: &str = "morphology-3";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootKind {
    Standard,
    Sahen,
    DerivedAdjective,
    NaAdjective,
}

struct ChainRoot {
    kind: RootKind,
    start: usize,
    anchor: usize,
    end: usize,
    role: MorphologyChainRole,
    base_lexeme: String,
    dictionary_form: String,
    lemma_form: String,
    lookup_form: String,
    evidence: Vec<String>,
}

pub fn analyze_bunsetsu(bunsetsu: &Bunsetsu, global_offset: usize) -> MorphologyArtifact {
    analyze_morphemes(&bunsetsu.morphemes, global_offset)
}

/// 在原始 IPADIC 语素序列上恢复活用链。
///
/// 该入口不依赖文节、构词或语法目录，因此文节边界、词汇展示和语法识别可以
/// 消费同一结果。原始 Morpheme 与字符坐标保持不变。
pub fn analyze_morphemes(morphemes: &[Morpheme], global_offset: usize) -> MorphologyArtifact {
    let mut chains = Vec::new();
    let mut index = 0;

    while index < morphemes.len() {
        let Some(root) = detect_root(morphemes, index) else {
            index += 1;
            continue;
        };

        let mut end = root.end;
        while end < morphemes.len()
            && attaches_to_chain(
                &morphemes[end],
                root.role.clone(),
                &morphemes[root.start..end],
            )
        {
            end += 1;
        }

        let mut operators = Vec::new();
        let mut connection_forms = Vec::new();
        let mut feature_candidates = Vec::new();
        let anchor = &morphemes[root.anchor];
        let mut evidence = root.evidence.clone();
        evidence.push(format!(
            "ipadic_anchor:{}:{}",
            anchor.conjugation_type, anchor.conjugation_form
        ));
        evidence.push(format!(
            "chain_role:{}",
            match root.role {
                MorphologyChainRole::Lexical => "lexical",
                MorphologyChainRole::Functional => "functional",
            }
        ));

        push_anchor_form_operator(&mut operators, morphemes, &root, global_offset);

        for local in root.end..end {
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
                        if conjectural {
                            "conjectural"
                        } else {
                            "volitional"
                        },
                        if conjectural {
                            "morphology.modality.conjectural"
                        } else {
                            "morphology.mood.volitional"
                        },
                        98,
                        if conjectural {
                            "推量助动词"
                        } else {
                            "意向助动词"
                        },
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
                && !operators.iter().any(|item| {
                    item.output_state == "conditional" && item.char_range == morpheme.char_range
                })
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

        operators.sort_by_key(|item| {
            (
                item.source_morpheme_range.0,
                operator_order(item.kind.as_str()),
                item.char_range.0,
            )
        });
        feature_candidates.extend(operators.iter().map(|item| item.concept_id.clone()));
        feature_candidates.sort();
        feature_candidates.dedup();
        let source_ranges = (root.start..end)
            .map(|local| morphemes[local].char_range)
            .collect::<Vec<_>>();
        let char_range = (
            morphemes[root.start].char_range.0,
            morphemes[end - 1].char_range.1,
        );
        let surface_form = morphemes[root.start..end]
            .iter()
            .map(|item| item.surface.as_str())
            .collect();
        chains.push(MorphologyChain {
            chain_id: format!("morph:{}:{}", char_range.0, char_range.1),
            anchor_morpheme: global_offset + root.anchor,
            anchor_range: anchor.char_range,
            morpheme_range: (global_offset + root.start, global_offset + end),
            char_range,
            role: root.role,
            base_lexeme: root.base_lexeme,
            surface_form,
            dictionary_form: root.dictionary_form,
            lemma_form: root.lemma_form,
            lookup_form: root.lookup_form,
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

/// 将词汇所有权的活用链应用到词头展示范围。
///
/// 词典查询形保持原有 head_word.base_form，不在这里改写。
pub fn apply_lexical_head(bunsetsu: &mut Bunsetsu) {
    let head_ranges = head_source_ranges(bunsetsu);
    if head_ranges.is_empty() {
        return;
    }
    let Some(chain) = bunsetsu
        .morphology
        .chains
        .iter()
        .filter(|chain| chain.role == MorphologyChainRole::Lexical)
        .filter(|chain| {
            head_ranges.iter().all(|head_range| {
                chain
                    .source_ranges
                    .iter()
                    .any(|range| contains_range(*range, *head_range))
            })
        })
        .max_by_key(|chain| chain.char_range.1 - chain.char_range.0)
    else {
        return;
    };
    bunsetsu.head_word.surface = chain.surface_form.clone();
}

fn detect_root(morphemes: &[Morpheme], index: usize) -> Option<ChainRoot> {
    let current = morphemes.get(index)?;

    if is_sahen_stem(current) {
        if let Some(verb) = morphemes
            .get(index + 1)
            .filter(|item| item.pos.major == "動詞" && item.base_form == "する")
        {
            let lookup_form = normalized_base(current);
            let dictionary_form = format!("{lookup_form}する");
            return Some(ChainRoot {
                kind: RootKind::Sahen,
                start: index,
                anchor: index + 1,
                end: index + 2,
                role: MorphologyChainRole::Lexical,
                base_lexeme: dictionary_form.clone(),
                lemma_form: dictionary_form.clone(),
                dictionary_form,
                lookup_form,
                evidence: vec![format!("sahen_anchor:{}", verb.conjugation_form)],
            });
        }
    }

    if current.pos.major == "名詞" {
        if let (Some(suffix), Some(auxiliary)) =
            (morphemes.get(index + 1), morphemes.get(index + 2))
        {
            if is_na_adjective_suffix(suffix) && is_copular_auxiliary(auxiliary) {
                let lookup_form =
                    format!("{}{}", normalized_base(current), normalized_base(suffix));
                let dictionary_form = format!("{lookup_form}だ");
                let lemma_form = format!("{lookup_form}な");
                return Some(ChainRoot {
                    kind: RootKind::NaAdjective,
                    start: index,
                    anchor: index + 2,
                    end: index + 3,
                    role: MorphologyChainRole::Lexical,
                    base_lexeme: dictionary_form.clone(),
                    dictionary_form,
                    lemma_form,
                    lookup_form,
                    evidence: vec!["na_adjective_suffix_chain".to_string()],
                });
            }
            if suffix.pos.major == "形容詞" && suffix.pos.sub1 == "接尾" {
                let lookup_form =
                    format!("{}{}", normalized_base(current), normalized_base(suffix));
                return Some(ChainRoot {
                    kind: RootKind::DerivedAdjective,
                    start: index,
                    anchor: index + 1,
                    end: index + 2,
                    role: MorphologyChainRole::Lexical,
                    base_lexeme: lookup_form.clone(),
                    dictionary_form: lookup_form.clone(),
                    lemma_form: lookup_form.clone(),
                    lookup_form,
                    evidence: vec!["derived_adjective_chain".to_string()],
                });
            }
        }
        if current.pos.sub1 == "形容動詞語幹" {
            if morphemes.get(index + 1).is_some_and(is_copular_auxiliary) {
                let lookup_form = normalized_base(current);
                let dictionary_form = format!("{lookup_form}だ");
                let lemma_form = format!("{lookup_form}な");
                return Some(ChainRoot {
                    kind: RootKind::NaAdjective,
                    start: index,
                    anchor: index + 1,
                    end: index + 2,
                    role: MorphologyChainRole::Lexical,
                    base_lexeme: dictionary_form.clone(),
                    dictionary_form,
                    lemma_form,
                    lookup_form,
                    evidence: vec!["na_adjective_stem_chain".to_string()],
                });
            }
        }
    }

    if !matches!(current.pos.major.as_str(), "動詞" | "形容詞" | "助動詞") {
        return None;
    }
    let role = if current.pos.major == "助動詞"
        || (current.pos.major == "動詞" && matches!(current.pos.sub1.as_str(), "非自立" | "接尾"))
    {
        MorphologyChainRole::Functional
    } else {
        MorphologyChainRole::Lexical
    };
    let base = normalized_base(current);
    Some(ChainRoot {
        kind: RootKind::Standard,
        start: index,
        anchor: index,
        end: index + 1,
        role,
        base_lexeme: base.clone(),
        dictionary_form: base.clone(),
        lemma_form: base.clone(),
        lookup_form: base,
        evidence: vec!["simple_inflecting_lexeme".to_string()],
    })
}

fn push_anchor_form_operator(
    operators: &mut Vec<MorphologyOperator>,
    morphemes: &[Morpheme],
    root: &ChainRoot,
    global_offset: usize,
) {
    let anchor = &morphemes[root.anchor];
    let global = global_offset + root.anchor;
    let form = anchor.conjugation_form.as_str();
    if root.kind == RootKind::NaAdjective && form == "体言接続" {
        push_operator(
            operators,
            global,
            anchor.char_range,
            "inflection_form",
            "adjectival_attributive",
            "morphology.form.adjectival_attributive",
            99,
            "形容动词・体言接续",
            &[],
        );
        return;
    }
    if form.contains("仮定") {
        push_operator(
            operators,
            global,
            anchor.char_range,
            "inflection_form",
            "conditional",
            "morphology.mood.conditional",
            95,
            form,
            &[],
        );
    } else if form.contains("命令") {
        push_operator(
            operators,
            global,
            anchor.char_range,
            "inflection_form",
            "imperative",
            "morphology.form.imperative",
            98,
            form,
            &[],
        );
    } else if form.contains("未然ウ接続") {
        let has_explicit_marker = morphemes
            .get(root.anchor + 1)
            .is_some_and(|next| matches!(next.base_form.as_str(), "う" | "よう"));
        if !has_explicit_marker {
            push_operator(
                operators,
                global,
                anchor.char_range,
                "modality",
                "volitional",
                "morphology.mood.volitional",
                95,
                form,
                &[],
            );
        }
    } else if form.contains("未然") {
        push_operator(
            operators,
            global,
            anchor.char_range,
            "inflection_form",
            "imperfective",
            "morphology.form.imperfective",
            96,
            form,
            &[],
        );
    } else if form.contains("連用") {
        push_operator(
            operators,
            global,
            anchor.char_range,
            "inflection_form",
            "continuative",
            "morphology.form.continuative",
            96,
            form,
            &[],
        );
    }
}

fn attaches_to_chain(morpheme: &Morpheme, role: MorphologyChainRole, chain: &[Morpheme]) -> bool {
    // 只有词汇谓词的 て／で 后面才把 いる／おる 的体貌用法收回词汇所有权。
    // provider 可能把这里的 いる 标作自立动词，因此要先于词性过滤判断。
    if role == MorphologyChainRole::Lexical
        && matches!(morpheme.base_form.as_str(), "いる" | "おる")
        && chain.last().is_some_and(|item| {
            item.pos.major == "助詞"
                && item.pos.sub1 == "接続助詞"
                && matches!(item.base_form.as_str(), "て" | "で")
        })
    {
        return true;
    }
    if morpheme.pos.major == "動詞" && morpheme.pos.sub1 == "接尾" {
        return true;
    }
    if morpheme.pos.major == "助詞"
        && morpheme.pos.sub1 == "接続助詞"
        && matches!(morpheme.base_form.as_str(), "て" | "で" | "ば")
    {
        return true;
    }
    if morpheme.pos.major != "助動詞" {
        return false;
    }

    // 助動词本身仍可能继续活用，但 `べし`、`だ＋ある` 等独立功能成分
    // 不应因为 provider 都标作助动词而被吞进同一条链。
    let owned_auxiliary = matches!(
        morpheme.base_form.as_str(),
        "た" | "ない"
            | "ぬ"
            | "ん"
            | "ます"
            | "たい"
            | "う"
            | "よう"
            | "せる"
            | "させる"
            | "す"
            | "れる"
            | "られる"
            | "がる"
            | "やす"
    );
    if owned_auxiliary {
        return true;
    }

    // くれる、みる、しまう 等补助用言保持独立的蓝色功能链。
    false
}

fn is_sahen_stem(morpheme: &Morpheme) -> bool {
    morpheme.pos.major == "名詞" && morpheme.pos.sub1 == "サ変接続"
}

fn is_na_adjective_suffix(morpheme: &Morpheme) -> bool {
    morpheme.pos.major == "名詞"
        && morpheme.pos.sub1 == "接尾"
        && (morpheme.pos.sub2 == "形容動詞語幹" || morpheme.base_form == "的")
}

fn is_copular_auxiliary(morpheme: &Morpheme) -> bool {
    morpheme.pos.major == "助動詞" && morpheme.base_form == "だ"
}

fn normalized_base(morpheme: &Morpheme) -> String {
    if morpheme.base_form.is_empty() || morpheme.base_form == "*" {
        morpheme.surface.clone()
    } else {
        morpheme.base_form.clone()
    }
}

fn head_source_ranges(bunsetsu: &Bunsetsu) -> Vec<(usize, usize)> {
    for start in 0..bunsetsu.morphemes.len() {
        let mut surface = String::new();
        let mut base_form = String::new();
        let mut ranges = Vec::new();
        for morpheme in bunsetsu.morphemes.iter().skip(start) {
            surface.push_str(&morpheme.surface);
            base_form.push_str(&normalized_base(morpheme));
            ranges.push(morpheme.char_range);
            if surface == bunsetsu.head_word.surface || base_form == bunsetsu.head_word.base_form {
                return ranges;
            }
            if !bunsetsu.head_word.surface.starts_with(&surface)
                && !bunsetsu.head_word.base_form.starts_with(&base_form)
            {
                break;
            }
        }
    }
    bunsetsu
        .morphemes
        .iter()
        .find(|morpheme| {
            morpheme.surface == bunsetsu.head_word.surface
                || morpheme.base_form == bunsetsu.head_word.base_form
        })
        .map(|morpheme| vec![morpheme.char_range])
        .unwrap_or_default()
}

fn operator_order(kind: &str) -> usize {
    match kind {
        "inflection_form" => 0,
        "phonological_alternation" => 1,
        "voice" => 2,
        "polarity" => 3,
        "tense" => 4,
        "modality" => 5,
        "politeness" => 6,
        "connection_form" => 7,
        _ => 8,
    }
}

fn contains_range(container: (usize, usize), inner: (usize, usize)) -> bool {
    inner.0 >= container.0 && inner.1 <= container.1
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
        label: String::new(),
        description: String::new(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{HeadWord, PosTag};

    fn morpheme(
        surface: &str,
        base: &str,
        major: &str,
        sub1: &str,
        sub2: &str,
        form: &str,
        start: usize,
    ) -> Morpheme {
        Morpheme {
            surface: surface.to_string(),
            base_form: base.to_string(),
            reading: String::new(),
            pos: PosTag {
                major: major.to_string(),
                sub1: sub1.to_string(),
                sub2: sub2.to_string(),
                sub3: "*".to_string(),
            },
            conjugation_type: String::new(),
            conjugation_form: form.to_string(),
            char_range: (start, start + surface.chars().count()),
        }
    }

    fn bunsetsu(morphemes: Vec<Morpheme>, head_surface: &str, head_base: &str) -> Bunsetsu {
        let pos = morphemes[0].pos.clone();
        let end = morphemes.last().unwrap().char_range.1;
        Bunsetsu {
            surface: morphemes.iter().map(|item| item.surface.as_str()).collect(),
            head_word: HeadWord {
                surface: head_surface.to_string(),
                base_form: head_base.to_string(),
                reading: String::new(),
                pos,
            },
            morphemes,
            grammar_tags: Vec::new(),
            morphology: MorphologyArtifact::default(),
            grammar_occurrences: Vec::new(),
            functional_residuals: Vec::new(),
            word_formations: Vec::new(),
            lexical_units: Vec::new(),
            function: None,
            char_range: (0, end),
        }
    }

    #[test]
    fn builds_reversible_operator_chain() {
        let morphemes = vec![
            morpheme("行か", "行く", "動詞", "自立", "*", "未然形", 0),
            morpheme("せ", "せる", "動詞", "接尾", "*", "未然形", 2),
            morpheme("られ", "られる", "動詞", "接尾", "*", "連用形", 3),
            morpheme("なかっ", "ない", "助動詞", "*", "*", "連用タ接続", 5),
            morpheme("た", "た", "助動詞", "*", "*", "基本形", 8),
        ];
        let artifact = analyze_morphemes(&morphemes, 0);
        let states = artifact.chains[0]
            .operators
            .iter()
            .map(|item| item.output_state.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            states,
            vec![
                "imperfective",
                "causative",
                "passive_potential",
                "negative",
                "past"
            ]
        );
        assert_eq!(artifact.chains[0].role, MorphologyChainRole::Lexical);
        assert_eq!(artifact.chains[0].surface_form, "行かせられなかった");
    }

    #[test]
    fn sahen_chain_keeps_existing_lookup_form_and_expands_head_surface() {
        let morphemes = vec![
            morpheme("分類", "分類", "名詞", "サ変接続", "*", "*", 0),
            morpheme("し", "する", "動詞", "自立", "*", "連用形", 2),
        ];
        let mut bunsetsu = bunsetsu(morphemes, "分類", "分類");
        bunsetsu.morphology = analyze_bunsetsu(&bunsetsu, 0);
        apply_lexical_head(&mut bunsetsu);
        let chain = &bunsetsu.morphology.chains[0];
        assert_eq!(chain.dictionary_form, "分類する");
        assert_eq!(chain.lemma_form, "分類する");
        assert_eq!(chain.lookup_form, "分類");
        assert_eq!(bunsetsu.head_word.surface, "分類し");
        assert_eq!(bunsetsu.head_word.base_form, "分類");
    }

    #[test]
    fn na_adjective_chain_is_lexical_but_sentence_final_na_is_not() {
        let adjective = vec![
            morpheme("静か", "静か", "名詞", "形容動詞語幹", "*", "*", 0),
            morpheme("な", "だ", "助動詞", "*", "*", "体言接続", 2),
        ];
        let mut bunsetsu = bunsetsu(adjective, "静か", "静か");
        bunsetsu.morphology = analyze_bunsetsu(&bunsetsu, 0);
        apply_lexical_head(&mut bunsetsu);
        assert_eq!(bunsetsu.head_word.surface, "静かな");
        assert_eq!(bunsetsu.morphology.chains[0].dictionary_form, "静かだ");
        assert_eq!(bunsetsu.morphology.chains[0].lemma_form, "静かな");
        assert_eq!(
            bunsetsu.morphology.chains[0].operators[0].concept_id,
            "morphology.form.adjectival_attributive"
        );

        let terminal = vec![
            morpheme("行く", "行く", "動詞", "自立", "*", "基本形", 0),
            morpheme("な", "な", "助詞", "終助詞", "*", "*", 2),
        ];
        let artifact = analyze_morphemes(&terminal, 0);
        assert_eq!(artifact.chains[0].surface_form, "行く");
        assert_eq!(artifact.chains[0].char_range, (0, 2));
    }

    #[test]
    fn lexical_and_functional_verbs_form_separate_chains() {
        let morphemes = vec![
            morpheme("読ん", "読む", "動詞", "自立", "*", "連用タ接続", 0),
            morpheme("で", "で", "助詞", "接続助詞", "*", "*", 2),
            morpheme(
                "くださっ",
                "くださる",
                "動詞",
                "非自立",
                "*",
                "連用タ接続",
                3,
            ),
            morpheme("た", "た", "助動詞", "*", "*", "基本形", 7),
        ];
        let artifact = analyze_morphemes(&morphemes, 0);
        assert_eq!(artifact.chains.len(), 2);
        assert_eq!(artifact.chains[0].role, MorphologyChainRole::Lexical);
        assert_eq!(artifact.chains[0].surface_form, "読んで");
        assert_eq!(artifact.chains[1].role, MorphologyChainRole::Functional);
        assert_eq!(artifact.chains[1].surface_form, "くださった");
    }

    #[test]
    fn te_iru_is_owned_by_the_lexical_predicate() {
        let morphemes = vec![
            morpheme("座っ", "座る", "動詞", "自立", "*", "連用タ接続", 0),
            morpheme("て", "て", "助詞", "接続助詞", "*", "*", 2),
            morpheme("い", "いる", "動詞", "非自立", "*", "未然形", 3),
            morpheme("なかっ", "ない", "助動詞", "*", "*", "連用タ接続", 5),
            morpheme("た", "た", "助動詞", "*", "*", "基本形", 8),
        ];
        let artifact = analyze_morphemes(&morphemes, 0);
        assert_eq!(artifact.chains.len(), 1);
        assert_eq!(artifact.chains[0].role, MorphologyChainRole::Lexical);
        assert_eq!(artifact.chains[0].surface_form, "座っていなかった");
    }

    #[test]
    fn independent_functional_verbs_stop_the_lexical_chain() {
        let morphemes = vec![
            morpheme("読ん", "読む", "動詞", "自立", "*", "連用タ接続", 0),
            morpheme("で", "で", "助詞", "接続助詞", "*", "*", 2),
            morpheme("くれ", "くれる", "動詞", "非自立", "*", "連用形", 3),
        ];
        let artifact = analyze_morphemes(&morphemes, 0);
        assert_eq!(artifact.chains.len(), 2);
        assert_eq!(artifact.chains[0].surface_form, "読んで");
        assert_eq!(artifact.chains[1].role, MorphologyChainRole::Functional);
    }
}
