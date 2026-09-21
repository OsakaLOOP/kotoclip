//! P4 十段核查语料的离线回归测试。
use kotoclip_nlp::{
    bunsetsu::BunsetsuArtifact,
    formation::FormationArtifact,
    model::{FeatureField, MorphemeToken, ProviderToken, FIELD_LABELS, FIELD_NAMES},
};
use serde_json::Value;
use std::{collections::BTreeSet, path::PathBuf};

struct Sample {
    text: String,
    morphemes: Vec<MorphemeToken>,
    sources: Vec<ProviderToken>,
    formation: FormationArtifact,
    bunsetsu: BunsetsuArtifact,
}

fn samples() -> Vec<Sample> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/validation/p4-sample-review.json");
    let report: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    report["cases"].as_array().unwrap().iter().map(|case| {
        let document = &case["document"];
        let morphemes = serde_json::from_value::<Vec<MorphemeToken>>(document["morphemes"].clone()).unwrap();
        let compact_fields = document["unidic_fields"].as_array().unwrap();
        let sources = morphemes.iter().enumerate().map(|(index, morpheme)| {
            let values = compact_fields[index]["fields"].as_object().unwrap();
            let fields = FIELD_NAMES.iter().enumerate().map(|(field, name)| {
                let value = values.get(*name).and_then(Value::as_str).map(str::to_owned);
                FeatureField { index: field, name: (*name).into(), label: FIELD_LABELS[field].into(), raw: value.clone(), value }
            }).collect();
            ProviderToken { index, surface: morpheme.surface.clone(), char_range: morpheme.char_range, byte_range: [0, 0],
                lexicon_type: compact_fields[index]["lexicon_type"].as_str().unwrap_or("known").into(), left_id: 0, right_id: 0,
                word_cost: 0, total_cost: 0, raw_feature: String::new(), fields }
        }).collect();
        Sample { text: document["text"].as_str().unwrap().into(), morphemes, sources,
            formation: serde_json::from_value(document["formation"].clone()).unwrap(),
            bunsetsu: serde_json::from_value(document["bunsetsu"].clone()).unwrap() }
    }).collect()
}

#[test]
fn sample_inflection_chains_preserve_ownership_and_operators() {
    let artifacts = samples().into_iter().map(|sample| kotoclip_nlp::morphology::collect(&sample.sources, &sample.morphemes).unwrap()).collect::<Vec<_>>();
    let chains = artifacts.iter().flat_map(|artifact| &artifact.chains).collect::<Vec<_>>();
    for surface in ["掲載された", "発展せしめる", "扱われます", "覗かれて", "しまって"] {
        assert!(chains.iter().any(|chain| chain.surface_form.contains(surface)), "活用链缺少 {surface}");
    }
    assert!(chains.iter().any(|chain| chain.operators.iter().any(|operator| operator.kind == "causative")));
    assert!(chains.iter().any(|chain| chain.operators.iter().any(|operator| operator.kind == "passive_potential")));
    assert!(chains.iter().any(|chain| chain.operators.iter().any(|operator| operator.kind == "politeness_masu")));
    assert!(chains.iter().any(|chain| chain.parent_chain_id.is_some()), "补助用言应引用所属词汇链");
}

#[test]
fn sample_formations_use_formal_tokens_and_keep_unusual_readings() {
    let samples = samples();
    let mut compounds = BTreeSet::new();
    for sample in &samples {
        let chars = sample.text.chars().collect::<Vec<_>>();
        for node in &sample.formation.nodes { compounds.insert(chars[node.char_range[0]..node.char_range[1]].iter().collect::<String>()); }
    }
    for surface in ["教科用図書", "神戸市", "方向転換", "線状降水帯"] {
        assert!(compounds.contains(surface), "构词候选缺少 {surface}");
    }
    for surface in ["見渡す", "這い上がる"] {
        assert_eq!(samples.iter().flat_map(|sample| &sample.morphemes).filter(|token| token.surface == surface).count(), 1,
            "正式词元流应保留 {surface} 整体");
    }
    for surface in ["雜", "霽れ", "ゾワゾワゾワ"] {
        assert!(samples.iter().flat_map(|sample| &sample.morphemes).any(|token| token.surface == surface), "样例缺少 {surface}");
    }
    let rules = kotoclip_core::language_rules::word_formations().unwrap();
    let application_matches = samples.iter().flat_map(|sample| rules.iter().flat_map(|rule|
        kotoclip_nlp::rules::matches(rule, &sample.text, &sample.morphemes, &sample.sources).unwrap())).count();
    assert!(application_matches > 0, "十段语料应触发类型化构词规则");

    let dictionary_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/dicts");
    let dictionary = kotoclip_core::dictionary::lookup::DictionaryEngine::new(dictionary_path).unwrap();
    let bindings = kotoclip_core::language_analysis::lookup_bindings(&dictionary, "方向転換", Some("ホウコウテンカン"));
    assert!(!bindings.is_empty(), "方向転換应绑定本地词典整体记录");
    assert!(bindings.iter().all(|binding| !binding.entry_key.is_empty() && !binding.occurrence_id.is_empty()));
}

#[test]
fn sample_grammar_and_correlative_rules_match_precise_ranges() {
    let samples = samples();
    let grammar = kotoclip_core::language_rules::grammar().unwrap();
    let expressions = kotoclip_core::language_rules::expressions().unwrap();
    let mut concepts = BTreeSet::new();
    let mut expression_ids = BTreeSet::new();
    for sample in &samples {
        for rule in grammar {
            if !kotoclip_nlp::rules::matches(rule, &sample.text, &sample.morphemes, &sample.sources).unwrap().is_empty() {
                if let Some(concept) = &rule.concept_id { concepts.insert(concept.clone()); }
            }
        }
        for rule in &expressions {
            let found = kotoclip_nlp::rules::matches(rule, &sample.text, &sample.morphemes, &sample.sources).unwrap();
            if found.iter().any(|matched| kotoclip_nlp::rules::respects_bunsetsu_gap(rule, matched, &sample.bunsetsu)) {
                expression_ids.insert(rule.id.split(":variant:").next().unwrap().to_owned());
            }
        }
    }
    for concept in ["morphology.voice.causative", "morphology.voice.passive_potential", "morphology.politeness.masu",
        "grammar.aspect.te_iru", "grammar.obligation.nakereba_naranai"] {
        assert!(concepts.contains(concept), "语法样例缺少 {concept}");
    }
    assert!(expression_ids.contains("concessive_tatoe"));
    assert!(expression_ids.contains("concessive_donnani"));
}
