use crate::models::{
    DictEntry, DictionaryCandidate, DictionaryLookup, DictionaryLookupTiming,
};

/// 将查询结果装配为前端与 CLI 共用的完整 Lookup 状态。
/// `initial_entries` 始终指原查询结果，用于保留词典内候选；`entries` 可以是用户已选择候选后的正文。
pub fn build_lookup(
    query: &str,
    reading: Option<&str>,
    selected_target: Option<String>,
    mode: &str,
    initial_entries: &[DictEntry],
    entries: Vec<DictEntry>,
    timing: Option<DictionaryLookupTiming>,
) -> DictionaryLookup {
    DictionaryLookup {
        query: query.to_string(),
        reading: reading.map(str::to_string),
        selected_target,
        selected_occurrence_id: default_occurrence_id(&entries),
        mode: mode.to_string(),
        candidates: collect_candidates(query, initial_entries),
        dictionary_names: collect_dictionary_names(initial_entries, &entries),
        entries,
        timing,
    }
}

pub fn collect_candidates(query: &str, entries: &[DictEntry]) -> Vec<DictionaryCandidate> {
    if !is_kana(query) {
        return Vec::new();
    }
    let mut candidates = Vec::<DictionaryCandidate>::new();
    for entry in entries.iter().filter(|entry| entry.headword == query) {
        for link in entry.links.iter().filter(|link| link.relation == "candidate") {
            if let Some(candidate) = candidates
                .iter_mut()
                .find(|candidate| candidate.target == link.target)
            {
                if !candidate.dictionary_names.contains(&entry.dict_name) {
                    candidate.dictionary_names.push(entry.dict_name.clone());
                }
                continue;
            }
            candidates.push(DictionaryCandidate {
                candidate_id: format!("target:{}", link.target),
                target: link.target.clone(),
                label: link.label.clone(),
                relation: link.relation.clone(),
                dictionary_names: vec![entry.dict_name.clone()],
                reading: None,
                entry_kind: String::new(),
                occurrence_ids: Vec::new(),
            });
        }
    }
    candidates
}

fn collect_dictionary_names(initial_entries: &[DictEntry], entries: &[DictEntry]) -> Vec<String> {
    let mut names = Vec::new();
    for entry in initial_entries.iter().chain(entries.iter()) {
        if !names.contains(&entry.dict_name) {
            names.push(entry.dict_name.clone());
        }
    }
    names
}

fn default_occurrence_id(entries: &[DictEntry]) -> Option<String> {
    let first_dictionary = entries.first()?.dict_name.as_str();
    let local_entries = entries
        .iter()
        .filter(|entry| entry.dict_name == first_dictionary)
        .collect::<Vec<_>>();
    let preferred = local_entries
        .iter()
        .copied()
        .filter(|entry| entry.is_preferred)
        .collect::<Vec<_>>();
    if preferred.len() == 1 {
        return Some(preferred[0].occurrence_id.clone());
    }
    (local_entries.len() == 1).then(|| local_entries[0].occurrence_id.clone())
}

fn is_kana(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_whitespace()
                || matches!(
                    character,
                    '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}'
                )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DictionaryLink;

    fn entry(dictionary: &str, occurrence: &str, preferred: bool) -> DictEntry {
        DictEntry {
            entry_key: occurrence.to_string(),
            dict_name: dictionary.to_string(),
            headword: "もう".to_string(),
            reading: Some("もう".to_string()),
            is_preferred: preferred,
            definition_html: String::new(),
            style_profile: String::new(),
            content_blocks: Vec::new(),
            match_type: "exact_form".to_string(),
            links: Vec::new(),
            occurrence_id: occurrence.to_string(),
            source_record_index: 0,
            entry_kind: "lexical".to_string(),
            header: Default::default(),
            senses: Vec::new(),
            sections: Vec::new(),
            adapter_diagnostics: Default::default(),
            match_evidence: None,
            raw_definition: None,
        }
    }

    #[test]
    fn ambiguous_first_dictionary_has_no_false_default() {
        let entries = vec![entry("Crown", "one", false), entry("Crown", "two", false)];
        assert_eq!(default_occurrence_id(&entries), None);
    }

    #[test]
    fn candidate_dictionary_names_are_merged() {
        let mut first = entry("Crown", "one", true);
        first.links.push(DictionaryLink {
            target: "もう【猛】".to_string(),
            label: "猛".to_string(),
            relation: "candidate".to_string(),
        });
        let mut second = entry("大辞林", "two", true);
        second.links = first.links.clone();
        let candidates = collect_candidates("もう", &[first, second]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].dictionary_names, vec!["Crown", "大辞林"]);
    }
}
