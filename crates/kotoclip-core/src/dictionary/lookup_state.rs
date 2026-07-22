use crate::models::{
    DictEntry, DictionaryFormAvailability, DictionaryFormGroup, DictionaryFormVariant,
    DictionaryLookup, DictionaryLookupTiming,
};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone)]
pub struct DictionaryFormVariantSeed {
    pub surface_form: String,
    pub readings: Vec<String>,
    pub evidence: Vec<String>,
    pub score: i32,
    pub dictionary_names: Vec<String>,
    order: usize,
}

#[derive(Debug, Clone)]
pub struct DictionaryFormSeed {
    pub display_form: String,
    pub normalized_form: String,
    pub readings: Vec<String>,
    pub evidence: Vec<String>,
    pub score: i32,
    pub variants: Vec<DictionaryFormVariantSeed>,
    pub available_dictionary_names: Vec<String>,
    order: usize,
    admissible: bool,
}

/// 从发现阶段的 occurrence 提取全局表记行。
/// alias 与读音回退只提供发现证据；没有兼容读音的非精确表记不会进入矩阵。
pub fn collect_form_seeds(
    query: &str,
    observed_form: Option<&str>,
    requested_reading: Option<&str>,
    entries: &[DictEntry],
) -> Vec<DictionaryFormSeed> {
    let query_key = normalize_form_identity(query);
    let observed_key = observed_form.map(normalize_form_identity);
    let query_surface_key = original_surface_identity(query);
    let observed_surface_key = observed_form.map(original_surface_identity);
    let reading_key = requested_reading.map(normalize_reading_identity);
    let mut seeds = Vec::<DictionaryFormSeed>::new();

    for entry in entries
        .iter()
        .filter(|entry| !matches!(entry.entry_kind.as_str(), "navigation" | "redirect"))
    {
        let entry_reading = entry
            .header
            .reading
            .as_deref()
            .or(entry.reading.as_deref())
            .map(normalize_reading_identity);
        let reading_compatible = match (&reading_key, &entry_reading) {
            (Some(requested), Some(actual)) => requested == actual,
            (Some(_), None) => true,
            (None, _) => true,
        };
        let base_score = entry
            .match_evidence
            .as_ref()
            .map_or(0, |evidence| evidence.score);
        let evidence = format!("{}:{}", entry.match_type, entry.dict_name);

        for form in entry_forms(entry) {
            let normalized = normalize_form_identity(&form);
            if normalized.is_empty() {
                continue;
            }
            let query_exact = normalized == query_key;
            let observed_exact = observed_key.as_deref() == Some(normalized.as_str());
            let surface_key = original_surface_identity(&form);
            let query_surface_exact = surface_key == query_surface_key;
            let observed_surface_exact =
                observed_surface_key.as_deref() == Some(surface_key.as_str());
            let admissible = query_exact || observed_exact || reading_compatible;
            let score = base_score
                + if observed_exact { 240 } else { 0 }
                + if query_exact { 180 } else { 0 }
                + if reading_compatible { 40 } else { 0 }
                + if observed_surface_exact { 1_000 } else { 0 }
                + if query_surface_exact { 700 } else { 0 };

            if let Some(seed) = seeds
                .iter_mut()
                .find(|seed| seed.normalized_form == normalized)
            {
                if let Some(reading) = entry_reading.as_ref() {
                    push_unique(&mut seed.readings, reading.clone());
                }
                push_unique(&mut seed.evidence, evidence.clone());
                push_unique(
                    &mut seed.available_dictionary_names,
                    entry.dict_name.clone(),
                );
                seed.score = seed.score.max(score);
                seed.admissible |= admissible;
                if let Some(variant) = seed
                    .variants
                    .iter_mut()
                    .find(|variant| original_surface_identity(&variant.surface_form) == surface_key)
                {
                    if let Some(reading) = entry_reading.as_ref() {
                        push_unique(&mut variant.readings, reading.clone());
                    }
                    push_unique(&mut variant.evidence, evidence.clone());
                    push_unique(&mut variant.dictionary_names, entry.dict_name.clone());
                    variant.score = variant.score.max(score);
                } else {
                    seed.variants.push(DictionaryFormVariantSeed {
                        surface_form: form,
                        readings: entry_reading.clone().into_iter().collect(),
                        evidence: vec![evidence.clone()],
                        score,
                        dictionary_names: vec![entry.dict_name.clone()],
                        order: seed.variants.len(),
                    });
                }
                continue;
            }

            seeds.push(DictionaryFormSeed {
                display_form: form.clone(),
                normalized_form: normalized,
                readings: entry_reading.clone().into_iter().collect(),
                evidence: vec![evidence.clone()],
                score,
                variants: vec![DictionaryFormVariantSeed {
                    surface_form: form,
                    readings: entry_reading.clone().into_iter().collect(),
                    evidence: vec![evidence.clone()],
                    score,
                    dictionary_names: vec![entry.dict_name.clone()],
                    order: 0,
                }],
                available_dictionary_names: vec![entry.dict_name.clone()],
                order: seeds.len(),
                admissible,
            });
        }
    }

    let mut context_forms = vec![(query, "context:query", 2_000)];
    if let Some(observed) = observed_form {
        context_forms.push((observed, "context:observed", 3_000));
    }
    for (surface_form, evidence, score) in context_forms {
        let normalized = normalize_form_identity(surface_form);
        let surface_key = original_surface_identity(surface_form);
        let Some(seed) = seeds
            .iter_mut()
            .find(|seed| seed.normalized_form == normalized)
        else {
            continue;
        };
        push_unique(&mut seed.evidence, evidence.to_string());
        seed.score = seed.score.max(score);
        seed.admissible = true;
        if let Some(variant) = seed
            .variants
            .iter_mut()
            .find(|variant| original_surface_identity(&variant.surface_form) == surface_key)
        {
            push_unique(&mut variant.evidence, evidence.to_string());
            variant.score = variant.score.max(score);
            if let Some(reading) = reading_key.as_ref() {
                push_unique(&mut variant.readings, reading.clone());
            }
        } else {
            seed.variants.push(DictionaryFormVariantSeed {
                surface_form: surface_form.to_string(),
                readings: reading_key.clone().into_iter().collect(),
                evidence: vec![evidence.to_string()],
                score,
                dictionary_names: Vec::new(),
                order: seed.variants.len(),
            });
        }
    }

    seeds.retain(|seed| seed.admissible);
    for seed in &mut seeds {
        seed.variants.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.order.cmp(&right.order))
        });
        if let Some(preferred) = seed.variants.first() {
            seed.display_form = preferred.surface_form.clone();
        }
    }
    seeds.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.order.cmp(&right.order))
    });
    seeds
}

pub fn build_form_groups(
    seeds: Vec<DictionaryFormSeed>,
    dictionary_names: &[String],
) -> Vec<DictionaryFormGroup> {
    let mut groups = seeds
        .into_iter()
        .map(|seed| {
            let available = seed.available_dictionary_names;
            DictionaryFormGroup {
                form_id: form_id(&seed.normalized_form),
                display_form: seed.display_form,
                normalized_form: seed.normalized_form,
                readings: seed.readings,
                evidence: seed.evidence,
                score: seed.score,
                variants: seed
                    .variants
                    .into_iter()
                    .map(|variant| DictionaryFormVariant {
                        surface_form: variant.surface_form,
                        readings: variant.readings,
                        evidence: variant.evidence,
                        score: variant.score,
                        dictionary_names: variant.dictionary_names,
                    })
                    .collect(),
                dictionaries: dictionary_names
                    .iter()
                    .map(|dictionary_name| DictionaryFormAvailability {
                        dictionary_name: dictionary_name.clone(),
                        available: available.contains(dictionary_name),
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        right.score.cmp(&left.score).then_with(|| {
            let left_count = left
                .dictionaries
                .iter()
                .filter(|item| item.available)
                .count();
            let right_count = right
                .dictionaries
                .iter()
                .filter(|item| item.available)
                .count();
            right_count.cmp(&left_count)
        })
    });
    groups
}

pub fn selected_form_id(forms: &[DictionaryFormGroup], requested: Option<&str>) -> Option<String> {
    if let Some(requested) = requested {
        let requested_key = normalize_form_identity(requested);
        if let Some(form) = forms.iter().find(|form| {
            form.form_id == requested
                || form.normalized_form == requested_key
                || normalize_form_identity(&form.display_form) == requested_key
        }) {
            return Some(form.form_id.clone());
        }
    }
    forms.first().map(|form| form.form_id.clone())
}

pub fn selected_form<'a>(
    forms: &'a [DictionaryFormGroup],
    selected_form_id: Option<&str>,
) -> Option<&'a DictionaryFormGroup> {
    selected_form_id
        .and_then(|selected| forms.iter().find(|form| form.form_id == selected))
        .or_else(|| forms.first())
}

pub fn entry_matches_form(entry: &DictEntry, form: &str) -> bool {
    let target = normalize_form_identity(form);
    entry_forms(entry)
        .into_iter()
        .any(|candidate| normalize_form_identity(&candidate) == target)
}

pub fn entry_matches_surface_form(entry: &DictEntry, form: &str) -> bool {
    let target = normalize_surface_identity(form);
    entry_forms(entry)
        .into_iter()
        .any(|candidate| normalize_surface_identity(&candidate) == target)
}

pub fn build_lookup(
    query: &str,
    observed_form: Option<&str>,
    reading: Option<&str>,
    pos: Option<&crate::models::PosTag>,
    selected_form_id: Option<String>,
    mode: &str,
    forms: Vec<DictionaryFormGroup>,
    dictionary_names: Vec<String>,
    entries: Vec<DictEntry>,
    timing: Option<DictionaryLookupTiming>,
) -> DictionaryLookup {
    DictionaryLookup {
        query: query.to_string(),
        observed_form: observed_form.map(str::to_string),
        reading: reading.map(str::to_string),
        pos: pos.cloned(),
        selected_form_id,
        mode: mode.to_string(),
        forms,
        dictionary_names,
        entries,
        timing,
    }
}

pub fn normalize_form_identity(value: &str) -> String {
    let normalized = normalize_surface_identity(value);
    let kana_only = normalized.chars().any(is_kana)
        && normalized
            .chars()
            .all(|character| is_kana(character) || matches!(character, 'ー' | '・' | '-' | '‐'));
    if kana_only {
        normalized.chars().map(katakana_to_hiragana).collect()
    } else {
        normalized
    }
}

pub fn normalize_surface_identity(value: &str) -> String {
    value
        .nfkc()
        .map(|character| match character {
            '繋' => '繫',
            _ => character,
        })
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn original_surface_identity(value: &str) -> String {
    value.trim().to_string()
}

pub fn normalize_reading_identity(value: &str) -> String {
    normalize_form_identity(value)
        .chars()
        .map(katakana_to_hiragana)
        .collect()
}

fn form_id(normalized: &str) -> String {
    format!("form:{normalized}")
}

fn entry_forms(entry: &DictEntry) -> Vec<String> {
    let mut forms = Vec::new();
    for scoped in &entry.header.scoped_forms {
        if scoped.kind == "original" {
            continue;
        }
        for form in expand_form(&scoped.form) {
            push_unique(&mut forms, form);
        }
    }
    if forms.is_empty() {
        let form = if entry.header.display_form.is_empty() {
            &entry.headword
        } else {
            &entry.header.display_form
        };
        for form in expand_form(form) {
            push_unique(&mut forms, form);
        }
    }
    forms
}

fn expand_form(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.is_empty() {
        return Vec::new();
    }
    let alternatives = value.split('・').collect::<Vec<_>>();
    if alternatives.len() > 1
        && alternatives
            .iter()
            .all(|alternative| alternative.chars().any(is_han))
    {
        return alternatives
            .into_iter()
            .flat_map(expand_optional_kana)
            .collect();
    }
    expand_optional_kana(value)
}

fn expand_optional_kana(value: &str) -> Vec<String> {
    let Some(open) = value.find('（') else {
        return vec![value.to_string()];
    };
    let after_open = open + '（'.len_utf8();
    let Some(relative_close) = value[after_open..].find('）') else {
        return vec![value.to_string()];
    };
    let close = after_open + relative_close;
    let optional = &value[after_open..close];
    if optional.is_empty() || !optional.chars().all(is_kana) {
        return vec![value.to_string()];
    }
    let suffix = &value[close + '）'.len_utf8()..];
    vec![
        format!("{}{}{}", &value[..open], optional, suffix),
        format!("{}{}", &value[..open], suffix),
    ]
}

fn is_han(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
    )
}

fn is_kana(character: char) -> bool {
    matches!(character, '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}')
}

fn katakana_to_hiragana(character: char) -> char {
    if ('\u{30a1}'..='\u{30f6}').contains(&character)
        || ('\u{30fd}'..='\u{30fe}').contains(&character)
    {
        char::from_u32(character as u32 - 0x60).unwrap_or(character)
    } else {
        character
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !value.is_empty() && !values.contains(&value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DictionaryForm, DictionaryMatchEvidence};

    fn entry(
        dictionary: &str,
        occurrence: &str,
        form: &str,
        reading: &str,
        match_type: &str,
        score: i32,
    ) -> DictEntry {
        DictEntry {
            entry_key: occurrence.to_string(),
            dict_name: dictionary.to_string(),
            headword: form.to_string(),
            reading: Some(reading.to_string()),
            is_preferred: false,
            definition_html: String::new(),
            style_profile: String::new(),
            content_blocks: Vec::new(),
            match_type: match_type.to_string(),
            links: Vec::new(),
            occurrence_id: occurrence.to_string(),
            source_record_index: 0,
            entry_kind: "lexical".to_string(),
            header: crate::models::DictionaryOccurrenceHeader {
                display_form: form.to_string(),
                reading: Some(reading.to_string()),
                scoped_forms: vec![DictionaryForm {
                    form: form.to_string(),
                    reading: Some(reading.to_string()),
                    kind: "canonical".to_string(),
                }],
                ..Default::default()
            },
            senses: Vec::new(),
            sections: Vec::new(),
            adapter_diagnostics: Default::default(),
            match_evidence: Some(DictionaryMatchEvidence {
                kind: match_type.to_string(),
                score,
                ..Default::default()
            }),
            raw_definition: None,
        }
    }

    #[test]
    fn same_form_across_dictionaries_and_readings_is_one_row() {
        let entries = vec![
            entry("大辞林", "one", "行く", "いく", "exact_form", 200),
            entry("小学馆", "two", "行く", "ゆく", "exact_form", 150),
            entry("Crown", "three", "行く", "いく", "exact_form", 160),
        ];
        let seeds = collect_form_seeds("行く", Some("行く"), Some("イク"), &entries);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].display_form, "行く");
        assert_eq!(seeds[0].readings, vec!["いく", "ゆく"]);
    }

    #[test]
    fn normalization_keeps_original_forms_and_observed_form_priority() {
        let entries = vec![entry(
            "大辞林",
            "one",
            "イク",
            "いく",
            "reading_fallback",
            300,
        )];
        let seeds = collect_form_seeds("いく", Some("いく"), Some("イク"), &entries);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].display_form, "いく");
        assert_eq!(seeds[0].variants.len(), 2);
        assert_eq!(seeds[0].variants[0].surface_form, "いく");
        assert_eq!(seeds[0].variants[1].surface_form, "イク");
        assert!(seeds[0].variants[0].score > seeds[0].variants[1].score);
        assert!(seeds[0].variants[0].dictionary_names.is_empty());
    }

    #[test]
    fn compatibility_group_keeps_each_original_surface_and_source() {
        let entries = vec![
            entry("大辞林", "one", "繫ぐ", "つなぐ", "exact_form", 200),
            entry("小学馆", "two", "繋ぐ", "つなぐ", "exact_form", 200),
        ];
        let seeds = collect_form_seeds("繫ぐ", Some("繋ぐ"), Some("ツナグ"), &entries);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].display_form, "繋ぐ");
        assert_eq!(seeds[0].variants.len(), 2);
        assert_eq!(seeds[0].variants[0].surface_form, "繋ぐ");
        assert_eq!(seeds[0].variants[0].dictionary_names, vec!["小学馆"]);
        assert_eq!(seeds[0].variants[1].surface_form, "繫ぐ");
        assert_eq!(seeds[0].variants[1].dictionary_names, vec!["大辞林"]);
    }

    #[test]
    fn incompatible_alias_is_rejected_but_other_readings_of_admitted_form_remain() {
        let entries = vec![
            entry("大辞林", "one", "熟れる", "なれる", "explicit_alias", 170),
            entry("小学馆", "two", "熟れる", "うれる", "explicit_alias", 70),
            entry("大辞林", "three", "縄", "なわ", "explicit_alias", 70),
        ];
        let seeds = collect_form_seeds("なれる", None, Some("ナレル"), &entries);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].display_form, "熟れる");
        assert_eq!(seeds[0].readings, vec!["なれる", "うれる"]);
    }

    #[test]
    fn expands_kanji_alternatives_without_splitting_katakana_compounds() {
        assert_eq!(
            expand_form("擦る・磨る・摺る"),
            vec!["擦る", "磨る", "摺る"]
        );
        assert_eq!(expand_form("オープン・カー"), vec!["オープン・カー"]);
        assert_eq!(
            expand_form("寄り掛（か）る"),
            vec!["寄り掛かる", "寄り掛る"]
        );
    }

    #[test]
    fn every_form_contains_the_global_dictionary_columns() {
        let seeds = vec![DictionaryFormSeed {
            display_form: "頑張る".to_string(),
            normalized_form: "頑張る".to_string(),
            readings: vec!["がんばる".to_string()],
            evidence: Vec::new(),
            score: 1,
            variants: vec![DictionaryFormVariantSeed {
                surface_form: "頑張る".to_string(),
                readings: vec!["がんばる".to_string()],
                evidence: Vec::new(),
                score: 1,
                dictionary_names: vec!["大辞林".to_string(), "Crown".to_string()],
                order: 0,
            }],
            available_dictionary_names: vec!["大辞林".to_string(), "Crown".to_string()],
            order: 0,
            admissible: true,
        }];
        let names = vec![
            "大辞林".to_string(),
            "小学馆".to_string(),
            "Crown".to_string(),
        ];
        let groups = build_form_groups(seeds, &names);
        assert_eq!(groups[0].dictionaries.len(), 3);
        assert!(groups[0].dictionaries[0].available);
        assert!(!groups[0].dictionaries[1].available);
        assert!(groups[0].dictionaries[2].available);
    }

    #[test]
    fn equal_scores_prefer_broader_dictionary_coverage_without_dropping_forms() {
        let make_seed = |form: &str, dictionaries: Vec<String>, order: usize| DictionaryFormSeed {
            display_form: form.to_string(),
            normalized_form: form.to_string(),
            readings: Vec::new(),
            evidence: Vec::new(),
            score: 100,
            variants: vec![DictionaryFormVariantSeed {
                surface_form: form.to_string(),
                readings: Vec::new(),
                evidence: Vec::new(),
                score: 100,
                dictionary_names: dictionaries.clone(),
                order: 0,
            }],
            available_dictionary_names: dictionaries,
            order,
            admissible: true,
        };
        let groups = build_form_groups(
            vec![
                make_seed("眼張る", vec!["大辞林".to_string()], 0),
                make_seed(
                    "頑張る",
                    vec![
                        "大辞林".to_string(),
                        "小学馆".to_string(),
                        "Crown".to_string(),
                    ],
                    1,
                ),
            ],
            &[
                "大辞林".to_string(),
                "小学馆".to_string(),
                "Crown".to_string(),
            ],
        );
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].display_form, "頑張る");
        assert_eq!(groups[1].display_form, "眼張る");
    }
}
