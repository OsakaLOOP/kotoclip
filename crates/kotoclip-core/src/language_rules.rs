//! 编译目录的 UniDic 条件适配与知识引用校验。
use kotoclip_nlp::rules::{Atom, Rule};
use serde_json::Value;
use std::sync::OnceLock;

fn strings(value: &Value) -> Vec<String> { value.as_array().map(|v| v.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default() }

fn forms(values: Vec<String>) -> Vec<String> {
    values.into_iter().flat_map(|v| match v.as_str() {
        "連用タ接続" | "連用テ接続" => vec!["連用形".into()],
        "未然ウ接続" => vec!["意志推量形".into()],
        "命令ｉ" => vec!["命令形".into()],
        "体言接続" => vec!["連体形".into()],
        _ => vec![v],
    }).collect()
}

fn bases(mut values: Vec<String>) -> Vec<String> {
    // 词元形与基本表记分别保留，目录中的假名补助用言映射到 UniDic 词元。
    for (kana, lemma) in [("いる", "居る"), ("おる", "居る"), ("なる", "成る"), ("くださる", "下さる"),
        ("しまう", "仕舞う"), ("みる", "見る"), ("くる", "来る"), ("いく", "行く"), ("いう", "言う"),
        ("ない", "無い"), ("ぬ", "ず"), ("する", "為る")] {
        if values.iter().any(|v| v == kana) { values.push(lemma.into()); }
    }
    values
}

fn compile_grammar() -> Result<Vec<Rule>, String> {
    let catalog: Value = serde_json::from_str(include_str!("../resources/grammar/compiled/grammar_catalog.json")).map_err(|e| e.to_string())?;
    let mut rules = Vec::new();
    for raw in catalog["rules"].as_array().ok_or("缺少语法规则")? {
        let concept = catalog["concepts"].as_array().unwrap().iter().find(|c| c["concept_id"] == raw["concept_id"]).ok_or("规则概念引用无效")?;
        let realization = catalog["realizations"].as_array().unwrap().iter().find(|r| r["realization_id"] == raw["realization_id"]).ok_or("规则实现引用无效")?;
        let mut atoms = Vec::new();
        for value in raw["atoms"].as_array().ok_or("缺少语法条件")? {
            // provider_components 是历史切词形式，按正式词元序列迁移。
            if let Some(parts) = value["provider_components"].as_array() {
                for part in parts {
                    atoms.push(Atom { base_forms: bases(vec![part["base_form"].as_str().ok_or("成分基本形缺失")?.into()]), ..Default::default() });
                }
                continue;
            }
            let sub = strings(&value["pos_sub1"]).into_iter().flat_map(|v| if v == "非自立" { vec!["非自立可能".into(), "非自立".into()] } else { vec![v] }).collect();
            atoms.push(Atom { surfaces: strings(&value["surfaces"]), base_forms: bases(strings(&value["base_forms"])),
                pos_major: strings(&value["pos_major"]), pos_sub1: sub, conjugation_types: strings(&value["conjugation_types"]),
                conjugation_forms: forms(strings(&value["conjugation_forms"])), morphology_features: strings(&value["morphology_features"]),
                capture: value["capture"].as_str().map(str::to_owned), optional: value["optional"].as_bool().unwrap_or(false), gap_before: 0 });
        }
        let id = raw["rule_id"].as_str().ok_or("规则身份缺失")?.to_owned();
        if id == "construction.neba_naranai" {
            atoms[0].surfaces = vec!["ね".into()]; atoms[0].base_forms = vec!["ず".into()];
            atoms[0].pos_major = vec!["助動詞".into()]; atoms[0].pos_sub1.clear();
        }
        if id == "construction.zaru_wo_enai" {
            atoms[0].base_forms = vec!["ず".into()]; atoms[0].pos_major = vec!["助動詞".into()];
        }
        let rule = Rule { id, label: concept["canonical_label"].as_str().unwrap_or("").into(), description: String::new(),
            kind: raw["kind"].as_str().unwrap_or("grammar_construction").into(), atoms,
            priority: raw["priority"].as_i64().unwrap_or(0) as i32, enabled: raw["enabled"].as_bool().unwrap_or(true), document_id: None,
            allow_whitespace: false, concept_id: raw["concept_id"].as_str().map(str::to_owned), sense_ids: strings(&realization["possible_sense_ids"]),
            display_from: raw["display_from"].as_u64().unwrap_or(0) as usize, display_to: raw["display_to"].as_u64().map(|n| n as usize), source_refs: strings(&raw["source_refs"]),
            gap_after_atom: None, gap_bunsetsu: None };
        rule.validate()?;
        rules.push(rule);
    }
    // UniDic 以助动词词元识别文语使役和缩略接续。
    rules.push(Rule { id: "unidic.contracted.te_iru".into(), label: "〜ている".into(), description: String::new(), kind: "grammar_construction".into(),
        atoms: vec![Atom { pos_major: vec!["動詞".into(), "助動詞".into()], conjugation_forms: vec!["連用形".into()], capture: Some("predicate".into()), ..Default::default() },
            Atom { base_forms: vec!["てる".into(), "でる".into()], pos_major: vec!["助動詞".into()], capture: Some("contracted_auxiliary".into()), ..Default::default() }],
        priority: 100, enabled: true, document_id: None, allow_whitespace: false, concept_id: Some("grammar.aspect.te_iru".into()), sense_ids: Vec::new(), display_from: 1, display_to: None, source_refs: vec!["p4-sample-review:p09".into()],
        gap_after_atom: None, gap_bunsetsu: None });
    Ok(rules)
}

pub fn grammar() -> Result<&'static [Rule], String> {
    static RULES: OnceLock<Result<Vec<Rule>, String>> = OnceLock::new();
    RULES.get_or_init(compile_grammar).as_ref().map(|r| r.as_slice()).map_err(Clone::clone)
}

fn expression_atom(value: &Value) -> Atom {
    Atom {
        base_forms: bases(strings(&value["lemmas"])), surfaces: strings(&value["surfaces"]),
        pos_major: value["pos"].as_str().map(|v| vec![v.into()]).unwrap_or_default(),
        pos_sub1: match value["pos_sub1"].as_str() { Some("自立") | None => Vec::new(), Some(v) => vec![v.into()] },
        conjugation_types: strings(&value["conjugation_types"]), conjugation_forms: forms(strings(&value["conjugation_forms"])),
        capture: value["capture"].as_str().map(str::to_owned), ..Default::default()
    }
}

pub fn expressions() -> Result<Vec<Rule>, String> {
    let file: Value = serde_json::from_str(include_str!("../resources/expression_patterns.json")).map_err(|e| e.to_string())?;
    let mut rules = file["patterns"].as_array().unwrap().iter().map(|raw| {
        let atoms = raw["atoms"].as_array().unwrap().iter().map(expression_atom).collect();
        let rule = Rule { id: raw["id"].as_str().unwrap().into(), label: raw["label"].as_str().unwrap_or("").into(), description: raw["description"].as_str().unwrap_or("").into(),
            kind: raw["expression_type"].as_str().unwrap_or("grammar_construction").into(), atoms, priority: 50, enabled: true, document_id: None,
            allow_whitespace: false, concept_id: None, sense_ids: Vec::new(), display_from: 0, display_to: None, source_refs: vec!["expression_patterns.json".into()],
            gap_after_atom: None, gap_bunsetsu: None };
        rule.validate()?; Ok(rule)
    }).collect::<Result<Vec<_>, String>>()?;
    let empty = Vec::new();
    for raw in file["correlative_patterns"].as_array().unwrap_or(&empty) {
        let head_variants = if let Some(variants) = raw["head_variants"].as_array() {
            variants.iter().map(|variant| variant.as_array().ok_or("非连续表达前部条件无效")).collect::<Result<Vec<_>, _>>()?
        } else {
            vec![raw["head_atoms"].as_array().ok_or("非连续表达缺少前部条件")?]
        };
        let gap = raw["gap_bunsetsu"].as_array().ok_or("非连续表达缺少间隔")?;
        let gap_bunsetsu = [gap.first().and_then(Value::as_u64).unwrap_or(0) as usize,
            gap.get(1).and_then(Value::as_u64).unwrap_or(12) as usize];
        for (head_index, heads) in head_variants.into_iter().enumerate() {
            for (tail_index, tail) in raw["tail_variants"].as_array().ok_or("非连续表达缺少后部条件")?.iter().enumerate() {
                let mut atoms = heads.iter().map(expression_atom).collect::<Vec<_>>();
                let gap_after_atom = atoms.len() - 1;
                let mut tail_atoms = tail.as_array().ok_or("非连续表达后部条件无效")?.iter().map(expression_atom).collect::<Vec<_>>();
                if let Some(first) = tail_atoms.first_mut() { first.gap_before = 128; }
                atoms.extend(tail_atoms);
                let rule = Rule { id: format!("{}:variant:{head_index}:{tail_index}", raw["id"].as_str().unwrap_or("correlative")),
                    label: raw["label"].as_str().unwrap_or("").into(), description: raw["description"].as_str().unwrap_or("").into(),
                    kind: "correlative".into(), atoms, priority: 40, enabled: true, document_id: None, allow_whitespace: false,
                    concept_id: None, sense_ids: Vec::new(), display_from: 0, display_to: None,
                    source_refs: vec!["expression_patterns.json".into()], gap_after_atom: Some(gap_after_atom), gap_bunsetsu: Some(gap_bunsetsu) };
                rule.validate()?;
                rules.push(rule);
            }
        }
    }
    Ok(rules)
}

pub fn word_formations() -> Result<Vec<Rule>, String> {
    let file: Value = serde_json::from_str(include_str!("../resources/word_formation_patterns.json")).map_err(|error| error.to_string())?;
    file["rules"].as_array().ok_or("缺少构词规则")?.iter().filter(|raw| raw["enabled"].as_bool().unwrap_or(true)).map(|raw| {
        let atoms = raw["atoms"].as_array().ok_or("构词规则缺少条件")?.iter().map(|value| {
            let mut major = value["pos"]["major"].as_str().map(|pos| match pos { "接頭詞" => "接頭辞", _ => pos }.to_owned()).into_iter().collect::<Vec<_>>();
            if value["pos"]["sub1"].as_str() == Some("接尾") && !major.iter().any(|value| value == "接尾辞") { major.push("接尾辞".into()); }
            let sub = value["pos"]["sub1"].as_str().and_then(|pos| match pos { "自立" | "名詞接続" | "数接続" | "数" | "接尾" => None, "非自立" => Some("非自立可能"), other => Some(other) }).map(str::to_owned).into_iter().collect();
            Atom { surfaces: strings(&value["surfaces"]), base_forms: bases(strings(&value["base_forms"])), pos_major: major,
                pos_sub1: sub, conjugation_types: strings(&value["conjugation_type_prefixes"]), conjugation_forms: forms(strings(&value["conjugation_forms"])),
                capture: value["capture"].as_str().map(str::to_owned), ..Default::default() }
        }).collect();
        let rule = Rule { id: raw["id"].as_str().ok_or("构词规则身份缺失")?.into(), label: raw["category"].as_str().unwrap_or("构词").into(),
            description: String::new(), kind: "lexical_unit".into(), atoms, priority: raw["priority"].as_i64().unwrap_or(0) as i32,
            enabled: true, document_id: None, allow_whitespace: false, concept_id: None, sense_ids: Vec::new(), display_from: 0,
            display_to: None, source_refs: vec![raw["source"].as_str().unwrap_or("word_formation_patterns.json").into()], gap_after_atom: None, gap_bunsetsu: None };
        rule.validate()?;
        Ok(rule)
    }).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn compiled_catalog_has_valid_unidic_rules() {
        assert!(super::grammar().unwrap().len() > 100);
        assert!(!super::expressions().unwrap().is_empty());
        assert!(super::word_formations().unwrap().len() > 10);
    }
}
