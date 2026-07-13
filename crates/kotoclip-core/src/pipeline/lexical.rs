use super::word_formation::AcceptedWordFormation;
use crate::dictionary::lookup::DictionaryEngine;
use crate::models::{
    DictionaryLexicalCandidate, DictionaryLexicalUnitAnnotation, LexicalCandidateStatus, Morpheme,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct AcceptedDictionaryLexicalUnit {
    pub morpheme_range: (usize, usize),
    pub annotation: DictionaryLexicalUnitAnnotation,
}

#[derive(Debug, Clone, Default)]
pub struct DictionaryLexicalMatchResult {
    pub accepted: Vec<AcceptedDictionaryLexicalUnit>,
    pub candidates: Vec<DictionaryLexicalCandidate>,
    pub conflicts: usize,
}

#[derive(Clone)]
struct RawCandidate {
    start: usize,
    end: usize,
    surface: String,
    query: String,
    shape: String,
    confidence: u8,
    auto_accept: bool,
    evidence: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LexicalCatalog {
    schema_version: u32,
    catalog_version: u32,
    patterns: Vec<LexicalPattern>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LexicalPattern {
    id: String,
    confidence: u8,
    auto_accept: bool,
}

static CATALOG: OnceLock<LexicalCatalog> = OnceLock::new();

fn catalog() -> &'static LexicalCatalog {
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../resources/lexical_candidate_patterns.json"
        ))
        .expect("内置词典词汇候选目录必须有效")
    })
}

pub fn validate_catalog() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = catalog();
    let mut ids = HashSet::new();
    if catalog.schema_version != 1 || catalog.catalog_version == 0 || catalog.patterns.is_empty() {
        return Err("词典词汇候选目录版本非法".into());
    }
    for pattern in &catalog.patterns {
        if !ids.insert(pattern.id.as_str())
            || pattern.confidence == 0
            || !matches!(
                pattern.id.as_str(),
                "nominal_compound"
                    | "compound_predicate"
                    | "derived_adjective"
                    | "ambiguous_lexical_sequence"
            )
        {
            return Err(format!("词典词汇候选规则非法：{}", pattern.id).into());
        }
    }
    Ok(())
}

pub fn catalog_audit() -> Result<crate::models::RuleCatalogAudit, Box<dyn std::error::Error>> {
    validate_catalog()?;
    let catalog = catalog();
    Ok(crate::models::RuleCatalogAudit {
        layer: "dictionary_lexical".to_string(),
        schema_version: catalog.schema_version,
        catalog_version: catalog.catalog_version,
        rule_count: catalog.patterns.len(),
        enabled_rule_count: catalog.patterns.len(),
        capabilities: vec![
            "exact_form_binding".to_string(),
            "morpheme_window_generation".to_string(),
            "lexical_shape_routing".to_string(),
            "interval_dynamic_programming".to_string(),
            "word_formation_overlap_guard".to_string(),
            "stable_entry_key".to_string(),
        ],
    })
}

fn pattern(id: &str) -> (u8, bool) {
    let pattern = catalog()
        .patterns
        .iter()
        .find(|pattern| pattern.id == id)
        .expect("分类结果必须存在对应目录项");
    (pattern.confidence, pattern.auto_accept)
}

pub fn match_dictionary_lexical_units(
    morphemes: &[Morpheme],
    dictionary: &DictionaryEngine,
    formations: &[AcceptedWordFormation],
) -> DictionaryLexicalMatchResult {
    let raw = generate_candidates(morphemes);
    let queries: HashSet<_> = raw
        .iter()
        .map(|candidate| candidate.query.clone())
        .collect();
    let matches = dictionary.resolve_exact_forms_batch(&queries);
    resolve_candidates(morphemes, raw, matches, formations)
}

fn is_lexical_atom(morpheme: &Morpheme) -> bool {
    matches!(
        morpheme.pos.major.as_str(),
        "名詞" | "接頭詞" | "動詞" | "形容詞"
    ) && !morpheme.surface.chars().any(char::is_whitespace)
}

fn classify(window: &[Morpheme]) -> Option<(String, u8, bool, Vec<String>)> {
    if window.len() < 2 || window.iter().any(|item| !is_lexical_atom(item)) {
        return None;
    }
    if window
        .iter()
        .all(|item| matches!(item.pos.major.as_str(), "名詞" | "接頭詞"))
    {
        let (confidence, auto_accept) = pattern("nominal_compound");
        return Some((
            "nominal_compound".to_string(),
            confidence,
            auto_accept,
            vec![
                "exact_dictionary_form".to_string(),
                "nominal_compound_shape".to_string(),
            ],
        ));
    }
    let last = window.last().unwrap();
    if last.pos.major == "動詞"
        && window[..window.len() - 1]
            .iter()
            .all(|item| item.pos.major == "動詞" && item.conjugation_form.starts_with("連用"))
    {
        let (confidence, auto_accept) = pattern("compound_predicate");
        return Some((
            "compound_predicate".to_string(),
            confidence,
            auto_accept,
            vec![
                "exact_dictionary_form".to_string(),
                "renyou_compound_predicate".to_string(),
            ],
        ));
    }
    if last.pos.major == "形容詞"
        && last.pos.sub1 == "接尾"
        && window[..window.len() - 1]
            .iter()
            .all(|item| item.pos.major == "名詞")
    {
        let (confidence, auto_accept) = pattern("derived_adjective");
        return Some((
            "derived_adjective".to_string(),
            confidence,
            auto_accept,
            vec![
                "exact_dictionary_form".to_string(),
                "adjectival_suffix_shape".to_string(),
            ],
        ));
    }
    let (confidence, auto_accept) = pattern("ambiguous_lexical_sequence");
    Some((
        "ambiguous_lexical_sequence".to_string(),
        confidence,
        auto_accept,
        vec!["exact_dictionary_form".to_string()],
    ))
}

fn generate_candidates(morphemes: &[Morpheme]) -> Vec<RawCandidate> {
    let mut result = Vec::new();
    let mut run_start = 0;
    while run_start < morphemes.len() {
        while run_start < morphemes.len() && !is_lexical_atom(&morphemes[run_start]) {
            run_start += 1;
        }
        if run_start == morphemes.len() {
            break;
        }
        let mut run_end = run_start + 1;
        while run_end < morphemes.len()
            && is_lexical_atom(&morphemes[run_end])
            && morphemes[run_end - 1].char_range.1 == morphemes[run_end].char_range.0
        {
            run_end += 1;
        }
        for start in run_start..run_end {
            for end in start + 2..=run_end {
                let window = &morphemes[start..end];
                let Some((shape, confidence, auto_accept, evidence)) = classify(window) else {
                    continue;
                };
                let surface: String = window.iter().map(|item| item.surface.as_str()).collect();
                let mut query = surface.clone();
                if let Some(last) = window.last().filter(|item| {
                    matches!(item.pos.major.as_str(), "動詞" | "形容詞")
                        && item.base_form != "*"
                        && item.base_form != item.surface
                }) {
                    let prefix: String = window[..window.len() - 1]
                        .iter()
                        .map(|item| item.surface.as_str())
                        .collect();
                    query = format!("{prefix}{}", last.base_form);
                }
                result.push(RawCandidate {
                    start,
                    end,
                    surface,
                    query,
                    shape,
                    confidence,
                    auto_accept,
                    evidence,
                });
            }
        }
        run_start = run_end;
    }
    result
}

fn overlaps(left: (usize, usize), right: (usize, usize)) -> bool {
    left.0 < right.1 && right.0 < left.1
}

fn resolve_candidates(
    morphemes: &[Morpheme],
    raw: Vec<RawCandidate>,
    matches: HashMap<String, Vec<crate::models::DictionaryEntryRef>>,
    formations: &[AcceptedWordFormation],
) -> DictionaryLexicalMatchResult {
    let mut candidates = Vec::new();
    for item in raw {
        let Some(references) = matches.get(&item.query).filter(|refs| !refs.is_empty()) else {
            continue;
        };
        let range = (item.start, item.end);
        let partial_formation = formations.iter().find(|formation| {
            overlaps(range, formation.morpheme_range) && range != formation.morpheme_range
        });
        let (status, reason, counter) = if partial_formation.is_some() {
            (
                LexicalCandidateStatus::Rejected,
                Some("word_formation_overlap".to_string()),
                vec!["partial_overlap_with_word_formation".to_string()],
            )
        } else if item.auto_accept {
            (LexicalCandidateStatus::Accepted, None, Vec::new())
        } else {
            (
                LexicalCandidateStatus::Pending,
                None,
                vec!["ambiguous_lexical_shape".to_string()],
            )
        };
        candidates.push(DictionaryLexicalCandidate {
            candidate_id: format!("dictionary:{}:{}:{}", item.query, item.start, item.end),
            surface: item.surface,
            query: item.query,
            morpheme_range: range,
            char_range: (
                morphemes[item.start].char_range.0,
                morphemes[item.end - 1].char_range.1,
            ),
            lexical_shape: item.shape,
            status,
            confidence: item.confidence,
            dictionary_refs: references.clone(),
            evidence: item.evidence,
            counter_evidence: counter,
            rejection_reason: reason,
        });
    }

    // 区间动态规划：跳过或选择以当前位置开始的候选。分数不只按长度，
    // 同分时使用候选 ID，保证结果不依赖 SQLite 返回顺序。
    let n = morphemes.len();
    let mut best_score = vec![0_i32; n + 1];
    let mut best_ids: Vec<Vec<String>> = vec![Vec::new(); n + 1];
    for pos in (0..n).rev() {
        best_score[pos] = best_score[pos + 1];
        best_ids[pos] = best_ids[pos + 1].clone();
        for candidate in candidates.iter().filter(|candidate| {
            candidate.status == LexicalCandidateStatus::Accepted
                && candidate.morpheme_range.0 == pos
        }) {
            let width = candidate.morpheme_range.1 - candidate.morpheme_range.0;
            let score = i32::from(candidate.confidence) * 100
                + width as i32 * 10
                + best_score[candidate.morpheme_range.1];
            let mut ids = vec![candidate.candidate_id.clone()];
            ids.extend(best_ids[candidate.morpheme_range.1].clone());
            if score > best_score[pos] || (score == best_score[pos] && ids < best_ids[pos]) {
                best_score[pos] = score;
                best_ids[pos] = ids;
            }
        }
    }
    let selected: HashSet<_> = best_ids[0].iter().cloned().collect();
    let mut conflicts = 0;
    for candidate in &mut candidates {
        if candidate.status == LexicalCandidateStatus::Accepted
            && !selected.contains(&candidate.candidate_id)
        {
            candidate.status = LexicalCandidateStatus::Rejected;
            candidate.rejection_reason = Some("conflict_lost".to_string());
            candidate
                .counter_evidence
                .push("higher_scoring_lexical_path".to_string());
            conflicts += 1;
        }
    }

    let accepted = candidates
        .iter()
        .filter(|candidate| selected.contains(&candidate.candidate_id))
        .map(|candidate| {
            let (start, end) = candidate.morpheme_range;
            let reading: String = morphemes[start..end]
                .iter()
                .filter(|item| item.reading != "*")
                .map(|item| item.reading.as_str())
                .collect();
            let mut reading_candidates: Vec<String> = candidate
                .dictionary_refs
                .iter()
                .flat_map(|reference| reference.readings.clone())
                .filter(|value| !value.is_empty())
                .collect();
            reading_candidates.sort();
            reading_candidates.dedup();
            AcceptedDictionaryLexicalUnit {
                morpheme_range: candidate.morpheme_range,
                annotation: DictionaryLexicalUnitAnnotation {
                    surface: candidate.surface.clone(),
                    base_form: candidate.query.clone(),
                    reading,
                    output_pos: morphemes[end - 1].pos.clone(),
                    morpheme_range: candidate.morpheme_range,
                    char_range: candidate.char_range,
                    head_morpheme: end - 1,
                    lexical_shape: candidate.lexical_shape.clone(),
                    dictionary_refs: candidate.dictionary_refs.clone(),
                    reading_candidates,
                    confidence: candidate.confidence,
                    evidence: candidate.evidence.clone(),
                },
            }
        })
        .collect();
    DictionaryLexicalMatchResult {
        accepted,
        candidates,
        conflicts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn morpheme(surface: &str, start: usize, major: &str, sub1: &str) -> Morpheme {
        Morpheme {
            surface: surface.to_string(),
            pos: crate::models::PosTag {
                major: major.to_string(),
                sub1: sub1.to_string(),
                sub2: "*".to_string(),
                sub3: "*".to_string(),
            },
            base_form: surface.to_string(),
            reading: surface.to_string(),
            conjugation_type: "*".to_string(),
            conjugation_form: "*".to_string(),
            char_range: (start, start + surface.chars().count()),
        }
    }

    #[test]
    fn exact_nominal_compound_is_accepted_without_reading_collision() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("kotoclip-lexical-{nonce}"));
        std::fs::create_dir_all(&directory).unwrap();
        let connection = Connection::open(directory.join("test.sqlite")).unwrap();
        connection.execute_batch("CREATE TABLE entries (id INTEGER PRIMARY KEY, headword TEXT, reading TEXT, definition TEXT, dict_name TEXT); CREATE TABLE entry_forms (entry_id INTEGER, form TEXT, normalized_form TEXT, form_type TEXT, is_primary INTEGER); CREATE TABLE entry_readings (entry_id INTEGER, reading TEXT, normalized_reading TEXT, is_primary INTEGER); CREATE TABLE metadata (schema_version INTEGER); INSERT INTO entries VALUES (1, '血飛沫', 'ちしぶき', 'definition', 'test'), (2, '一和', 'いちわ', 'wrong', 'test'); INSERT INTO entry_forms VALUES (1, '血飛沫', '血飛沫', 'kanji', 1), (2, '一和', '一和', 'kanji', 1); INSERT INTO entry_readings VALUES (2, 'いちわ', 'イチワ', 1); INSERT INTO metadata VALUES (3);").unwrap();
        drop(connection);
        let dictionary = DictionaryEngine::new(&directory).unwrap();
        let blood = vec![
            morpheme("血", 0, "名詞", "一般"),
            morpheme("飛沫", 1, "名詞", "一般"),
        ];
        let result = match_dictionary_lexical_units(&blood, &dictionary, &[]);
        assert_eq!(result.accepted.len(), 1);
        assert_eq!(result.accepted[0].annotation.base_form, "血飛沫");
        let counter = vec![
            morpheme("一", 0, "名詞", "数"),
            morpheme("羽", 1, "名詞", "接尾"),
        ];
        assert!(match_dictionary_lexical_units(&counter, &dictionary, &[])
            .accepted
            .is_empty());
        drop(dictionary);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn accepted_dictionary_units_change_bunsetsu_without_absorbing_syntax() {
        let Some(ipadic) = [
            "../../ipadic/system.dic",
            "../ipadic/system.dic",
            "ipadic/system.dic",
        ]
        .into_iter()
        .find(|path| std::path::Path::new(path).is_file()) else {
            return;
        };
        let Some(dicts) = ["../../data/dicts", "../data/dicts", "data/dicts"]
            .into_iter()
            .find(|path| std::path::Path::new(path).is_dir())
        else {
            return;
        };
        let pipeline = crate::pipeline::Pipeline::new(ipadic).unwrap();
        let dictionary = DictionaryEngine::new(dicts).unwrap();
        let tokens = pipeline.process_with_dictionary(
            "マジックミラーと血飛沫。男になる。",
            &[],
            &dictionary,
        );
        for expected in ["マジックミラーと", "血飛沫"] {
            let token = tokens
                .iter()
                .find(|token| token.bunsetsu.surface == expected)
                .unwrap_or_else(|| panic!("{expected} 应由词典整体改变文节"));
            assert_eq!(token.bunsetsu.lexical_units.len(), 1);
            assert!(!token.bunsetsu.lexical_units[0].dictionary_refs.is_empty());
        }
        assert!(tokens.iter().all(|token| {
            token
                .bunsetsu
                .lexical_units
                .iter()
                .all(|unit| unit.surface != "男になる")
        }));
        let reconstructed: String = tokens
            .iter()
            .map(|token| token.bunsetsu.surface.as_str())
            .collect();
        assert_eq!(reconstructed, "マジックミラーと血飛沫。男になる。");
    }
}
