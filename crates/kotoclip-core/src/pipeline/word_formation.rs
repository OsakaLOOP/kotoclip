use crate::models::{
    Morpheme, PosTag, RuleCatalogAudit, WordFormationAnnotation, WordFormationCapture,
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone)]
pub struct AcceptedWordFormation {
    pub morpheme_range: (usize, usize),
    pub annotation: WordFormationAnnotation,
    pub output_pos: PosTag,
}

#[derive(Debug, Clone)]
pub struct RejectedWordFormation {
    pub rule_id: String,
    pub morpheme_range: (usize, usize),
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct WordFormationMatchResult {
    pub accepted: Vec<AcceptedWordFormation>,
    pub rejected: Vec<RejectedWordFormation>,
    pub conflicts: usize,
}

pub struct WordFormationMatcher {
    rules: Vec<WordFormationRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    catalog_version: u32,
    rules: Vec<WordFormationRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WordFormationRule {
    id: String,
    rule_version: u32,
    source: String,
    enabled: bool,
    category: String,
    #[serde(default)]
    priority: i32,
    atoms: Vec<Atom>,
    output: Output,
    examples: Vec<String>,
    counter_examples: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Atom {
    #[serde(default)]
    surfaces: Vec<String>,
    #[serde(default)]
    base_forms: Vec<String>,
    #[serde(default)]
    pos: Option<PartialPos>,
    #[serde(default)]
    conjugation_types: Vec<String>,
    #[serde(default)]
    conjugation_type_prefixes: Vec<String>,
    #[serde(default)]
    conjugation_forms: Vec<String>,
    capture: Option<String>,
    #[serde(default = "one")]
    min: usize,
    #[serde(default = "one")]
    max: usize,
    #[serde(default = "surface_normalization")]
    normalization: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialPos {
    major: String,
    #[serde(default)]
    sub1: Option<String>,
    #[serde(default)]
    sub2: Option<String>,
    #[serde(default)]
    sub3: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    pos: PosTag,
    head_atom: usize,
    #[serde(default = "default_confidence")]
    confidence: u8,
}

fn one() -> usize {
    1
}
fn surface_normalization() -> String {
    "surface".to_string()
}
fn default_confidence() -> u8 {
    90
}

impl WordFormationMatcher {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let catalog: Catalog =
            serde_json::from_str(include_str!("../../resources/word_formation_patterns.json"))?;
        if catalog.schema_version != 2 || catalog.catalog_version == 0 {
            return Err(format!(
                "不支持的构词规则 schema_version：{}",
                catalog.schema_version
            )
            .into());
        }
        if catalog.rules.is_empty() {
            return Err("构词规则目录不能为空".into());
        }
        let mut ids = HashSet::new();
        for rule in &catalog.rules {
            validate_rule(rule, &mut ids)?;
        }
        Ok(Self {
            rules: catalog
                .rules
                .into_iter()
                .filter(|rule| rule.enabled)
                .collect(),
        })
    }

    pub fn match_morphemes(&self, morphemes: &[Morpheme]) -> WordFormationMatchResult {
        let mut candidates = Vec::new();
        let mut rejected = Vec::new();
        for rule in &self.rules {
            for start in 0..morphemes.len() {
                let mut matches = Vec::new();
                match_rule(
                    rule,
                    morphemes,
                    start,
                    0,
                    start,
                    &mut Vec::new(),
                    &mut matches,
                );
                if matches.is_empty() && atom_matches(&rule.atoms[0], &morphemes[start]) {
                    rejected.push(RejectedWordFormation {
                        rule_id: rule.id.clone(),
                        morpheme_range: (start, start + 1),
                        reason: "atom_mismatch".to_string(),
                    });
                }
                for matched in matches {
                    if matched.end <= start {
                        continue;
                    }
                    candidates.push(candidate_from_match(rule, morphemes, start, matched));
                }
            }
        }
        candidates.sort_by(|left, right| {
            right
                .rule
                .priority
                .cmp(&left.rule.priority)
                .then_with(|| (right.end - right.start).cmp(&(left.end - left.start)))
                .then_with(|| left.rule.id.cmp(&right.rule.id))
                .then_with(|| left.start.cmp(&right.start))
        });
        let mut claimed = vec![false; morphemes.len()];
        let mut accepted = Vec::new();
        let mut conflicts = 0;
        for candidate in candidates {
            if claimed[candidate.start..candidate.end]
                .iter()
                .any(|claimed| *claimed)
            {
                conflicts += 1;
                rejected.push(RejectedWordFormation {
                    rule_id: candidate.rule.id.clone(),
                    morpheme_range: (candidate.start, candidate.end),
                    reason: "conflict_lost".to_string(),
                });
                continue;
            }
            claimed[candidate.start..candidate.end].fill(true);
            accepted.push(candidate.accepted);
        }
        accepted.sort_by_key(|item| item.morpheme_range.0);
        WordFormationMatchResult {
            accepted,
            rejected,
            conflicts,
        }
    }
}

pub fn catalog_audit() -> Result<RuleCatalogAudit, Box<dyn std::error::Error>> {
    let catalog: Catalog =
        serde_json::from_str(include_str!("../../resources/word_formation_patterns.json"))?;
    if catalog.schema_version != 2 || catalog.catalog_version == 0 || catalog.rules.is_empty() {
        return Err("构词规则目录版本或内容非法".into());
    }
    let mut ids = HashSet::new();
    for rule in &catalog.rules {
        validate_rule(rule, &mut ids)?;
    }
    Ok(RuleCatalogAudit {
        layer: "word_formation".to_string(),
        schema_version: catalog.schema_version,
        catalog_version: catalog.catalog_version,
        rule_count: catalog.rules.len(),
        enabled_rule_count: catalog.rules.iter().filter(|rule| rule.enabled).count(),
        capabilities: vec![
            "surface_set".to_string(),
            "base_form_set".to_string(),
            "four_level_pos".to_string(),
            "conjugation_type_exact".to_string(),
            "conjugation_type_prefix".to_string(),
            "conjugation_form_set".to_string(),
            "bounded_repeat".to_string(),
            "named_capture".to_string(),
            "typed_output_head".to_string(),
            "strict_unknown_field_rejection".to_string(),
        ],
    })
}

fn validate_rule(
    rule: &WordFormationRule,
    ids: &mut HashSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if rule.id.trim().is_empty() || !ids.insert(rule.id.clone()) {
        return Err(format!("构词规则 ID 为空或重复：{}", rule.id).into());
    }
    if rule.rule_version == 0
        || rule.source.trim().is_empty()
        || rule.category.trim().is_empty()
        || rule.examples.is_empty()
        || rule.counter_examples.is_empty()
    {
        return Err(format!("构词规则治理字段不完整：{}", rule.id).into());
    }
    if rule.atoms.is_empty()
        || rule.output.head_atom >= rule.atoms.len()
        || rule.output.confidence > 100
    {
        return Err(format!("构词规则输出非法：{}", rule.id).into());
    }
    for atom in &rule.atoms {
        if atom.min == 0
            || atom.max < atom.min
            || atom.max > 8
            || atom.normalization != "surface" && atom.normalization != "base_form"
        {
            return Err(format!("构词规则原子非法：{}", rule.id).into());
        }
        if atom.surfaces.is_empty()
            && atom.base_forms.is_empty()
            && atom.pos.is_none()
            && atom.conjugation_types.is_empty()
            && atom.conjugation_type_prefixes.is_empty()
            && atom.conjugation_forms.is_empty()
        {
            return Err(format!("构词规则原子缺少约束：{}", rule.id).into());
        }
        if let Some(pos) = &atom.pos {
            if pos.major.trim().is_empty()
                || pos.sub2.is_some() && pos.sub1.is_none()
                || pos.sub3.is_some() && pos.sub2.is_none()
            {
                return Err(format!("构词规则词性层级非法：{}", rule.id).into());
            }
        }
    }
    Ok(())
}

fn atom_matches(atom: &Atom, morpheme: &Morpheme) -> bool {
    (atom.surfaces.is_empty() || atom.surfaces.iter().any(|value| value == &morpheme.surface))
        && (atom.base_forms.is_empty()
            || atom
                .base_forms
                .iter()
                .any(|value| value == &morpheme.base_form))
        && atom.pos.as_ref().is_none_or(|pos| {
            pos.major == morpheme.pos.major
                && pos
                    .sub1
                    .as_ref()
                    .is_none_or(|value| value == &morpheme.pos.sub1)
                && pos
                    .sub2
                    .as_ref()
                    .is_none_or(|value| value == &morpheme.pos.sub2)
                && pos
                    .sub3
                    .as_ref()
                    .is_none_or(|value| value == &morpheme.pos.sub3)
        })
        && (atom.conjugation_types.is_empty()
            || atom
                .conjugation_types
                .iter()
                .any(|value| value == &morpheme.conjugation_type))
        && (atom.conjugation_type_prefixes.is_empty()
            || atom
                .conjugation_type_prefixes
                .iter()
                .any(|value| morpheme.conjugation_type.starts_with(value)))
        && (atom.conjugation_forms.is_empty()
            || atom
                .conjugation_forms
                .iter()
                .any(|value| value == &morpheme.conjugation_form))
}

#[derive(Clone)]
struct AtomMatch {
    atom: usize,
    start: usize,
    end: usize,
}

struct CompleteMatch {
    end: usize,
    atoms: Vec<AtomMatch>,
}

fn match_rule(
    rule: &WordFormationRule,
    morphemes: &[Morpheme],
    start: usize,
    atom_index: usize,
    cursor: usize,
    matched: &mut Vec<AtomMatch>,
    results: &mut Vec<CompleteMatch>,
) {
    if atom_index == rule.atoms.len() {
        results.push(CompleteMatch {
            end: cursor,
            atoms: matched.clone(),
        });
        return;
    }
    let atom = &rule.atoms[atom_index];
    let mut maximum = 0;
    while maximum < atom.max
        && cursor + maximum < morphemes.len()
        && atom_matches(atom, &morphemes[cursor + maximum])
    {
        maximum += 1;
    }
    for count in (atom.min..=maximum).rev() {
        matched.push(AtomMatch {
            atom: atom_index,
            start: cursor,
            end: cursor + count,
        });
        match_rule(
            rule,
            morphemes,
            start,
            atom_index + 1,
            cursor + count,
            matched,
            results,
        );
        matched.pop();
    }
}

struct Candidate<'a> {
    rule: &'a WordFormationRule,
    start: usize,
    end: usize,
    accepted: AcceptedWordFormation,
}

fn candidate_from_match<'a>(
    rule: &'a WordFormationRule,
    morphemes: &[Morpheme],
    start: usize,
    matched: CompleteMatch,
) -> Candidate<'a> {
    let end = matched.end;
    let mut captures: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut base_form = String::new();
    let mut reading = String::new();
    let mut head_morpheme = start;
    for atom_match in &matched.atoms {
        let atom = &rule.atoms[atom_match.atom];
        if atom_match.atom == rule.output.head_atom {
            head_morpheme = atom_match.end - 1;
        }
        if let Some(name) = &atom.capture {
            captures
                .entry(name.clone())
                .and_modify(|range| range.1 = atom_match.end)
                .or_insert((atom_match.start, atom_match.end));
        }
        for morpheme in &morphemes[atom_match.start..atom_match.end] {
            base_form.push_str(if atom.normalization == "base_form" {
                &morpheme.base_form
            } else {
                &morpheme.surface
            });
            if morpheme.reading != "*" {
                reading.push_str(&morpheme.reading);
            }
        }
    }
    let surface: String = morphemes[start..end]
        .iter()
        .map(|morpheme| morpheme.surface.as_str())
        .collect();
    let char_range = (
        morphemes[start].char_range.0,
        morphemes[end - 1].char_range.1,
    );
    let captures = captures
        .into_iter()
        .map(
            |(name, (capture_start, capture_end))| WordFormationCapture {
                name,
                surface: morphemes[capture_start..capture_end]
                    .iter()
                    .map(|morpheme| morpheme.surface.as_str())
                    .collect(),
                morpheme_range: (capture_start, capture_end),
                char_range: (
                    morphemes[capture_start].char_range.0,
                    morphemes[capture_end - 1].char_range.1,
                ),
            },
        )
        .collect();
    Candidate {
        rule,
        start,
        end,
        accepted: AcceptedWordFormation {
            morpheme_range: (start, end),
            annotation: WordFormationAnnotation {
                rule_id: rule.id.clone(),
                category: rule.category.clone(),
                surface,
                base_form,
                reading,
                output_pos: rule.output.pos.clone(),
                morpheme_range: (start, end),
                char_range,
                head_morpheme,
                captures,
                confidence: rule.output.confidence,
            },
            output_pos: rule.output.pos.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::morpheme::MorphemeAnalyzer;

    fn analyzer() -> Option<MorphemeAnalyzer> {
        [
            "../../ipadic/system.dic",
            "../ipadic/system.dic",
            "ipadic/system.dic",
        ]
        .into_iter()
        .find(|path| std::path::Path::new(path).is_file())
        .and_then(|path| MorphemeAnalyzer::new(path).ok())
    }

    #[test]
    fn matches_representative_compounds_without_crossing_boundaries() {
        let Some(analyzer) = analyzer() else {
            return;
        };
        let matcher = WordFormationMatcher::new().unwrap();
        for (text, expected_rule) in [
            ("冷やし神", "deity_by_godan_action"),
            ("食べ放題", "renyou_houdai"),
            ("煙草臭い", "noun_adjectival_suffix"),
            ("第一話", "ordinal_counter"),
            ("数千冊", "numeric_counter"),
        ] {
            let result = matcher.match_morphemes(&analyzer.analyze(text));
            assert!(
                result
                    .accepted
                    .iter()
                    .any(|item| item.annotation.rule_id == expected_rule),
                "{text} 未命中 {expected_rule}"
            );
        }
        for text in ["煙草が臭い", "冷やして神を見る", "第一", "第話"] {
            let result = matcher.match_morphemes(&analyzer.analyze(text));
            assert!(result.accepted.is_empty(), "{text} 不应产生构词单位");
        }
        let spaced = matcher.match_morphemes(&analyzer.analyze("数 千冊"));
        assert!(spaced
            .accepted
            .iter()
            .all(|item| item.annotation.surface != "数 千冊"));
    }

    #[test]
    fn matches_annotation_audit_prefix_suffix_and_provider_cases() {
        let Some(analyzer) = analyzer() else {
            return;
        };
        let matcher = WordFormationMatcher::new().unwrap();
        for (text, expected_rule, expected_surface) in [
            ("異世界人", "prefix_noun_suffix", "異世界人"),
            ("半笑い", "prefix_noun", "半笑い"),
            ("再集結", "prefix_noun", "再集結"),
            ("各中隊", "prefix_noun", "各中隊"),
            ("超高速", "prefix_noun", "超高速"),
            ("超速い", "prefix_adjective", "超速い"),
            ("超すごい", "prefix_adjective", "超すごい"),
            ("非人道的", "prefix_noun", "非人道的"),
            ("直掩部隊", "prefix_noun_suffix", "直掩部隊"),
            ("航程", "noun_with_misclassified_hodo", "航程"),
            ("伏し目がち", "noun_or_renyou_gachi", "伏し目がち"),
            ("ためらいがち", "renyou_gachi", "ためらいがち"),
            ("やり終える", "compound_verb_renyou_nonself", "やり終える"),
            ("見え始める", "compound_verb_renyou_nonself", "見え始める"),
            ("蹴散らす", "compound_verb_renyou_independent", "蹴散らす"),
            (
                "一眠りしなおす",
                "noun_sahen_compound_verb",
                "一眠りしなおす",
            ),
            ("ライン帰り", "noun_with_kaeri_suffix", "ライン帰り"),
        ] {
            let result = matcher.match_morphemes(&analyzer.analyze(text));
            assert!(
                result.accepted.iter().any(|item| {
                    item.annotation.rule_id == expected_rule
                        && item.annotation.surface == expected_surface
                }),
                "{text} 未命中 {expected_rule}"
            );
        }
    }

    #[test]
    fn limits_misclassified_hodo_compatibility_to_koutei() {
        let Some(analyzer) = analyzer() else {
            return;
        };
        let matcher = WordFormationMatcher::new().unwrap();
        for text in ["彼程", "友達程", "三日程"] {
            let result = matcher.match_morphemes(&analyzer.analyze(text));
            assert!(
                result.accepted.iter().all(|item| {
                    item.annotation.rule_id != "noun_with_misclassified_hodo"
                }),
                "{text} 不应命中航程的有限兼容规则"
            );
        }
    }

    #[test]
    fn accepted_formation_drives_bunsetsu_head_without_losing_morphemes() {
        let Some(path) = [
            "../../ipadic/system.dic",
            "../ipadic/system.dic",
            "ipadic/system.dic",
        ]
        .into_iter()
        .find(|path| std::path::Path::new(path).is_file()) else {
            return;
        };
        let pipeline = crate::pipeline::Pipeline::new(path).unwrap();
        for (text, surface, base_form, pos) in [
            ("冷やし神が座る。", "冷やし神が", "冷やし神", "名詞"),
            ("部屋が煙草臭い。", "煙草臭い", "煙草臭い", "形容詞"),
            ("本を数千冊集める。", "数千冊", "数千冊", "名詞"),
        ] {
            let tokens = pipeline.process(text, &[]);
            let token = tokens
                .iter()
                .find(|token| token.bunsetsu.surface == surface)
                .unwrap();
            assert_eq!(token.bunsetsu.head_word.base_form, base_form);
            assert_eq!(token.bunsetsu.head_word.pos.major, pos);
            assert!(!token.bunsetsu.word_formations.is_empty());
            assert!(token.bunsetsu.morphemes.len() >= 2);
            let reconstructed: String = tokens
                .iter()
                .map(|token| token.bunsetsu.surface.as_str())
                .collect();
            assert_eq!(reconstructed, text);
        }
    }

    #[test]
    fn catalog_rejects_unknown_fields() {
        let invalid = r#"{"schema_version":2,"catalog_version":1,"rules":[],"typo":true}"#;
        assert!(serde_json::from_str::<Catalog>(invalid).is_err());
        let audit = catalog_audit().unwrap();
        assert!(audit.capabilities.contains(&"four_level_pos".to_string()));
        assert!(audit
            .capabilities
            .contains(&"conjugation_type_prefix".to_string()));
    }
}
