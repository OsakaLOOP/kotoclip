use super::lexical::AcceptedDictionaryLexicalUnit;
use super::word_formation::AcceptedWordFormation;
use crate::models::{
    Bunsetsu, BunsetsuAnalysisReport, BunsetsuBoundaryDecision, BunsetsuFunction,
    BunsetsuFunctionAnnotation, DictionaryLexicalUnitAnnotation, HeadWord, Morpheme,
    RuleCatalogAudit, WordFormationAnnotation,
};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BunsetsuCatalog {
    schema_version: u32,
    catalog_version: u32,
    weights: BoundaryWeights,
    formal_nouns: Vec<String>,
    relational_nouns: Vec<String>,
    boundary_rules: Vec<BoundaryRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundaryWeights {
    hard: i32,
    structural: i32,
    default_split: i32,
    default_join: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundaryRule {
    id: String,
    kind: String,
    decision: String,
    weight: String,
    priority: i32,
    #[serde(default)]
    alternatives: Vec<BoundaryAlternative>,
    #[serde(default)]
    span_alternatives: Vec<SpanBoundaryAlternative>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundaryAlternative {
    left: MorphemeConstraint,
    right: MorphemeConstraint,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpanBoundaryAlternative {
    span_first: MorphemeConstraint,
    span_last: MorphemeConstraint,
    current: MorphemeConstraint,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MorphemeConstraint {
    #[serde(default)]
    surfaces: Vec<String>,
    #[serde(default)]
    base_forms: Vec<String>,
    #[serde(default)]
    pos_major: Vec<String>,
    #[serde(default)]
    pos_sub1: Vec<String>,
    #[serde(default)]
    pos_sub2: Vec<String>,
    #[serde(default)]
    pos_sub3: Vec<String>,
    #[serde(default)]
    conjugation_types: Vec<String>,
    #[serde(default)]
    conjugation_type_prefixes: Vec<String>,
    #[serde(default)]
    conjugation_forms: Vec<String>,
}

pub struct BunsetsuAnalyzer {
    catalog: BunsetsuCatalog,
}

impl BunsetsuAnalyzer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut catalog: BunsetsuCatalog =
            serde_json::from_str(include_str!("../../resources/bunsetsu_patterns.json"))?;
        if catalog.schema_version != 2
            || catalog.catalog_version == 0
            || catalog.weights.hard <= catalog.weights.structural
        {
            return Err("文节规则目录版本或权重非法".into());
        }
        let mut ids = HashSet::new();
        for rule in &catalog.boundary_rules {
            if rule.id.trim().is_empty()
                || !ids.insert(rule.id.clone())
                || !matches!(rule.decision.as_str(), "join" | "split")
                || !matches!(
                    rule.weight.as_str(),
                    "hard" | "structural" | "default_split" | "default_join"
                )
                || !matches!(
                    rule.kind.as_str(),
                    "hard_symbol_boundary"
                        | "formal_noun_after_predicate"
                        | "relational_noun_after_no"
                        | "nominal_predicate_with_suru"
                        | "span_pattern"
                        | "formal_noun_negative_predicate"
                        | "functional_attachment"
                        | "prefix_attachment"
                        | "new_independent_core"
                        | "default_attachment"
                )
            {
                return Err(format!("文节边界规则非法：{}", rule.id).into());
            }
            if rule.kind == "prefix_attachment"
                && (rule.alternatives.is_empty()
                    || rule.alternatives.iter().any(|alternative| {
                        !constraint_is_valid(&alternative.left)
                            || !constraint_is_valid(&alternative.right)
                    }))
            {
                return Err(format!("接续规则缺少合法的多分支约束：{}", rule.id).into());
            }
            if rule.kind == "span_pattern"
                && (rule.span_alternatives.is_empty()
                    || rule.span_alternatives.iter().any(|alternative| {
                        !constraint_is_valid(&alternative.span_first)
                            || !constraint_is_valid(&alternative.span_last)
                            || !constraint_is_valid(&alternative.current)
                    }))
            {
                return Err(format!("跨度规则缺少合法的多分支约束：{}", rule.id).into());
            }
        }
        if !catalog
            .boundary_rules
            .iter()
            .any(|rule| rule.kind == "default_attachment")
        {
            return Err("文节规则目录缺少默认边界规则".into());
        }
        catalog.boundary_rules.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(Self { catalog })
    }

    pub fn analyze(
        &self,
        morphemes: &[Morpheme],
        merge_rules: &[Vec<String>],
        formations: &[AcceptedWordFormation],
    ) -> BunsetsuAnalysisReport {
        self.analyze_with_lexical(morphemes, merge_rules, formations, &[])
    }

    pub fn analyze_with_lexical(
        &self,
        morphemes: &[Morpheme],
        merge_rules: &[Vec<String>],
        formations: &[AcceptedWordFormation],
        lexical_units: &[AcceptedDictionaryLexicalUnit],
    ) -> BunsetsuAnalysisReport {
        analyze_with_catalog(
            morphemes,
            merge_rules,
            formations,
            lexical_units,
            &self.catalog,
        )
    }
}

fn constraint_is_valid(constraint: &MorphemeConstraint) -> bool {
    let has_constraint = !constraint.surfaces.is_empty()
        || !constraint.base_forms.is_empty()
        || !constraint.pos_major.is_empty()
        || !constraint.pos_sub1.is_empty()
        || !constraint.pos_sub2.is_empty()
        || !constraint.pos_sub3.is_empty()
        || !constraint.conjugation_types.is_empty()
        || !constraint.conjugation_type_prefixes.is_empty()
        || !constraint.conjugation_forms.is_empty();
    has_constraint
        && (constraint.pos_sub2.is_empty() || !constraint.pos_sub1.is_empty())
        && (constraint.pos_sub3.is_empty() || !constraint.pos_sub2.is_empty())
}

/// 判断形态素是否是自立语 (能够独立构成词意的词，如动词、名词、形容词等)
fn is_jiritsugo(m: &Morpheme) -> bool {
    let pos = &m.pos;
    match pos.major.as_str() {
        "動詞" => pos.sub1 == "自立",
        "形容詞" => true,
        "名詞" => {
            // 名词中要排除接尾辞、非自立名词等
            pos.sub1 != "接尾" && pos.sub1 != "非自立" && pos.sub1 != "特殊"
        }
        "副詞" | "連体詞" | "接続詞" | "感動詞" | "接頭詞" => true,
        _ => false,
    }
}

/// 判断形态素是否是标点/记号
fn is_punctuation(m: &Morpheme) -> bool {
    m.pos.major == "記号"
}

/// 基于词性状态机，将形态素序列聚合成文节 (Bunsetsu) 列表，并在此阶段应用用户自定义合并规则
pub fn chunk(
    morphemes: &[Morpheme],
    merge_rules: &[Vec<String>],
    formations: &[AcceptedWordFormation],
) -> Vec<Bunsetsu> {
    BunsetsuAnalyzer::new()
        .expect("内置文节规则必须有效")
        .analyze(morphemes, merge_rules, formations)
        .bunsetsus
}

fn analyze_with_catalog(
    morphemes: &[Morpheme],
    merge_rules: &[Vec<String>],
    formations: &[AcceptedWordFormation],
    lexical_units: &[AcceptedDictionaryLexicalUnit],
    catalog: &BunsetsuCatalog,
) -> BunsetsuAnalysisReport {
    if morphemes.is_empty() {
        return BunsetsuAnalysisReport {
            bunsetsus: Vec::new(),
            boundaries: Vec::new(),
            unresolved_boundaries: 0,
            reconstruction_ok: true,
            range_integrity_ok: true,
        };
    }

    let n = morphemes.len();
    let atomic_joins = collect_atomic_joins(morphemes, merge_rules, formations, lexical_units);
    let features = BoundaryFeatures::new(morphemes, formations, catalog);
    let selected_path = choose_bunsetsu_path(morphemes, &atomic_joins, &features, catalog);
    let unresolved_boundaries = selected_path.unresolved;
    let spans = selected_path.spans;
    let mut boundaries = Vec::with_capacity(n.saturating_sub(1));
    let mut span_start = 0;
    let split_points: HashSet<usize> = spans.iter().map(|range| range.1).collect();
    for boundary in 1..n {
        while boundary
            > spans
                .iter()
                .find(|range| range.0 == span_start)
                .map_or(n, |range| range.1)
        {
            span_start = boundary - 1;
        }
        let options = boundary_options(
            morphemes,
            span_start,
            boundary,
            &atomic_joins,
            &features,
            catalog,
        );
        let decision = if split_points.contains(&boundary) {
            "split"
        } else {
            "join"
        };
        let selected = if options.preferred.decision == decision {
            &options.preferred
        } else {
            &options.alternative
        };
        let alternative = if options.preferred.decision == decision {
            &options.alternative
        } else {
            &options.preferred
        };
        boundaries.push(BunsetsuBoundaryDecision {
            morpheme_index: boundary,
            decision: decision.to_string(),
            score: selected.score,
            evidence: selected.evidence.clone(),
            alternatives: vec![if decision == "split" { "join" } else { "split" }.to_string()],
            alternative_score: alternative.score,
            counter_evidence: alternative.evidence.clone(),
            hard_constraint: selected.score.abs() == catalog.weights.hard,
        });
        if decision == "split" {
            span_start = boundary;
        }
    }

    let mut bunsetsus = Vec::with_capacity(spans.len());
    for (start, end) in spans {
        let mut local_formations = Vec::new();
        for formation in formations.iter().filter(|formation| {
            start <= formation.morpheme_range.0 && formation.morpheme_range.1 <= end
        }) {
            let mut annotation = formation.annotation.clone();
            annotation.morpheme_range.0 -= start;
            annotation.morpheme_range.1 -= start;
            annotation.head_morpheme -= start;
            for capture in &mut annotation.captures {
                capture.morpheme_range.0 -= start;
                capture.morpheme_range.1 -= start;
            }
            local_formations.push(annotation);
        }
        let mut local_lexical_units = Vec::new();
        for lexical_unit in lexical_units.iter().filter(|lexical_unit| {
            start <= lexical_unit.morpheme_range.0 && lexical_unit.morpheme_range.1 <= end
        }) {
            let mut annotation = lexical_unit.annotation.clone();
            annotation.morpheme_range.0 -= start;
            annotation.morpheme_range.1 -= start;
            annotation.head_morpheme -= start;
            local_lexical_units.push(annotation);
        }
        bunsetsus.push(build_bunsetsu_with_annotations(
            morphemes[start..end].to_vec(),
            local_formations,
            local_lexical_units,
        ));
    }

    for bunsetsu in &mut bunsetsus {
        bunsetsu.function = Some(infer_function(bunsetsu));
    }
    let reconstructed: String = bunsetsus
        .iter()
        .map(|bunsetsu| bunsetsu.surface.as_str())
        .collect();
    let expected: String = morphemes
        .iter()
        .map(|morpheme| morpheme.surface.as_str())
        .collect();
    let range_integrity_ok = bunsetsus.iter().all(|bunsetsu| {
        bunsetsu.char_range.0 <= bunsetsu.char_range.1
            && bunsetsu
                .morphemes
                .first()
                .is_some_and(|item| item.char_range.0 == bunsetsu.char_range.0)
            && bunsetsu
                .morphemes
                .last()
                .is_some_and(|item| item.char_range.1 == bunsetsu.char_range.1)
    });
    BunsetsuAnalysisReport {
        bunsetsus,
        boundaries,
        unresolved_boundaries,
        reconstruction_ok: reconstructed == expected,
        range_integrity_ok,
    }
}

pub fn catalog_audit() -> Result<RuleCatalogAudit, Box<dyn std::error::Error>> {
    let analyzer = BunsetsuAnalyzer::new()?;
    Ok(RuleCatalogAudit {
        layer: "bunsetsu".to_string(),
        schema_version: analyzer.catalog.schema_version,
        catalog_version: analyzer.catalog.catalog_version,
        rule_count: analyzer.catalog.boundary_rules.len(),
        enabled_rule_count: analyzer.catalog.boundary_rules.len(),
        capabilities: vec![
            "word_formation_atomic_span".to_string(),
            "versioned_boundary_evidence".to_string(),
            "multi_branch_connection_constraints".to_string(),
            "span_first_last_current_constraints".to_string(),
            "candidate_dag".to_string(),
            "deterministic_path_tie_break".to_string(),
            "hard_boundary_violation_count".to_string(),
            "local_function_annotation".to_string(),
            "strict_unknown_field_rejection".to_string(),
        ],
    })
}

#[derive(Clone)]
struct BoundaryDecisionOption {
    decision: &'static str,
    score: i32,
    evidence: Vec<String>,
    hard_violation: usize,
}

struct BoundaryOptions {
    preferred: BoundaryDecisionOption,
    alternative: BoundaryDecisionOption,
}

fn catalog_weight(catalog: &BunsetsuCatalog, name: &str) -> i32 {
    match name {
        "hard" => catalog.weights.hard,
        "structural" => catalog.weights.structural,
        "default_split" => catalog.weights.default_split,
        _ => catalog.weights.default_join,
    }
}

fn collect_atomic_joins(
    morphemes: &[Morpheme],
    merge_rules: &[Vec<String>],
    formations: &[AcceptedWordFormation],
    lexical_units: &[AcceptedDictionaryLexicalUnit],
) -> HashSet<usize> {
    let mut joins = HashSet::new();
    for formation in formations {
        joins.extend(formation.morpheme_range.0 + 1..formation.morpheme_range.1);
    }
    for lexical_unit in lexical_units {
        joins.extend(lexical_unit.morpheme_range.0 + 1..lexical_unit.morpheme_range.1);
    }
    for start in 0..morphemes.len() {
        if let Some(length) = merge_rules
            .iter()
            .filter(|rule| start + rule.len() <= morphemes.len())
            .filter(|rule| {
                rule.iter()
                    .enumerate()
                    .all(|(offset, surface)| morphemes[start + offset].surface == *surface)
            })
            .map(Vec::len)
            .max()
        {
            joins.extend(start + 1..start + length);
        }
    }
    joins
}

struct BoundaryFeatures {
    predicate_prefix: Vec<usize>,
    last_formal_noun: Vec<Option<usize>>,
    last_independent_core: Vec<Option<usize>>,
    effective_pos: Vec<crate::models::PosTag>,
}

impl BoundaryFeatures {
    fn new(
        morphemes: &[Morpheme],
        formations: &[AcceptedWordFormation],
        catalog: &BunsetsuCatalog,
    ) -> Self {
        let mut predicate_prefix = vec![0; morphemes.len() + 1];
        let mut last_formal_noun = vec![None; morphemes.len() + 1];
        let mut last_independent_core = vec![None; morphemes.len() + 1];
        let mut effective_pos: Vec<_> = morphemes
            .iter()
            .map(|morpheme| morpheme.pos.clone())
            .collect();
        for formation in formations {
            if let Some(pos) = effective_pos.get_mut(formation.morpheme_range.0) {
                *pos = formation.output_pos.clone();
            }
        }
        for (index, morpheme) in morphemes.iter().enumerate() {
            predicate_prefix[index + 1] = predicate_prefix[index]
                + usize::from(matches!(
                    morpheme.pos.major.as_str(),
                    "動詞" | "形容詞" | "助動詞"
                ));
            last_formal_noun[index + 1] = if catalog.formal_nouns.contains(&morpheme.base_form) {
                Some(index)
            } else {
                last_formal_noun[index]
            };
            last_independent_core[index + 1] = if is_jiritsugo(morpheme) {
                Some(index)
            } else {
                last_independent_core[index]
            };
        }
        Self {
            predicate_prefix,
            last_formal_noun,
            last_independent_core,
            effective_pos,
        }
    }

    fn has_predicate(&self, start: usize, end: usize) -> bool {
        self.predicate_prefix[end] > self.predicate_prefix[start]
    }

    fn has_local_formal_noun_chain(&self, start: usize, end: usize) -> bool {
        let Some(formal) = self.last_formal_noun[end] else {
            return false;
        };
        if formal < start {
            return false;
        }
        self.last_independent_core[end].is_none_or(|core| core < formal)
    }

    fn matches_constraint(
        &self,
        index: usize,
        morpheme: &Morpheme,
        constraint: &MorphemeConstraint,
    ) -> bool {
        let pos = &self.effective_pos[index];
        (constraint.surfaces.is_empty() || constraint.surfaces.contains(&morpheme.surface))
            && (constraint.base_forms.is_empty()
                || constraint.base_forms.contains(&morpheme.base_form))
            && (constraint.pos_major.is_empty() || constraint.pos_major.contains(&pos.major))
            && (constraint.pos_sub1.is_empty() || constraint.pos_sub1.contains(&pos.sub1))
            && (constraint.pos_sub2.is_empty() || constraint.pos_sub2.contains(&pos.sub2))
            && (constraint.pos_sub3.is_empty() || constraint.pos_sub3.contains(&pos.sub3))
            && (constraint.conjugation_types.is_empty()
                || constraint
                    .conjugation_types
                    .contains(&morpheme.conjugation_type))
            && (constraint.conjugation_type_prefixes.is_empty()
                || constraint
                    .conjugation_type_prefixes
                    .iter()
                    .any(|prefix| morpheme.conjugation_type.starts_with(prefix)))
            && (constraint.conjugation_forms.is_empty()
                || constraint
                    .conjugation_forms
                    .contains(&morpheme.conjugation_form))
    }
}

fn boundary_rule_matches(
    rule: &BoundaryRule,
    morphemes: &[Morpheme],
    span_start: usize,
    boundary: usize,
    features: &BoundaryFeatures,
    catalog: &BunsetsuCatalog,
) -> bool {
    let previous = &morphemes[boundary - 1];
    let current = &morphemes[boundary];
    match rule.kind.as_str() {
        "hard_symbol_boundary" => is_punctuation(previous) || is_punctuation(current),
        "formal_noun_after_predicate" => {
            catalog.formal_nouns.contains(&current.base_form)
                && features.has_predicate(span_start, boundary)
        }
        "relational_noun_after_no" => {
            previous.base_form == "の" && catalog.relational_nouns.contains(&current.base_form)
        }
        "nominal_predicate_with_suru" => {
            current.base_form == "する"
                && morphemes[span_start..boundary]
                    .iter()
                    .rev()
                    .find(|item| item.pos.major != "記号")
                    .is_some_and(|item| item.pos.major == "名詞")
        }
        "span_pattern" => rule.span_alternatives.iter().any(|alternative| {
            features.matches_constraint(span_start, &morphemes[span_start], &alternative.span_first)
                && features.matches_constraint(boundary - 1, previous, &alternative.span_last)
                && features.matches_constraint(boundary, current, &alternative.current)
        }),
        "formal_noun_negative_predicate" => {
            current.base_form == "ない"
                && features.has_local_formal_noun_chain(span_start, boundary)
        }
        "functional_attachment" => {
            matches!(current.pos.major.as_str(), "助詞" | "助動詞")
                || matches!(current.pos.sub1.as_str(), "非自立" | "接尾")
        }
        "prefix_attachment" => rule.alternatives.iter().any(|alternative| {
            features.matches_constraint(boundary - 1, previous, &alternative.left)
                && features.matches_constraint(boundary, current, &alternative.right)
        }),
        "new_independent_core" => is_jiritsugo(current),
        "default_attachment" => true,
        _ => false,
    }
}

fn boundary_options(
    morphemes: &[Morpheme],
    span_start: usize,
    boundary: usize,
    atomic_joins: &HashSet<usize>,
    features: &BoundaryFeatures,
    catalog: &BunsetsuCatalog,
) -> BoundaryOptions {
    if atomic_joins.contains(&boundary) {
        return BoundaryOptions {
            preferred: BoundaryDecisionOption {
                decision: "join",
                score: catalog.weights.hard,
                evidence: vec!["atomic_span".to_string()],
                hard_violation: 0,
            },
            alternative: BoundaryDecisionOption {
                decision: "split",
                score: -catalog.weights.hard,
                evidence: vec!["atomic_span_violation".to_string()],
                hard_violation: 1,
            },
        };
    }
    let rule = catalog
        .boundary_rules
        .iter()
        .find(|rule| {
            boundary_rule_matches(rule, morphemes, span_start, boundary, features, catalog)
        })
        .expect("文节目录必须包含默认规则");
    let score = catalog_weight(catalog, &rule.weight);
    let alternative_decision = if rule.decision == "join" {
        "split"
    } else {
        "join"
    };
    let hard = rule.weight == "hard";
    BoundaryOptions {
        preferred: BoundaryDecisionOption {
            decision: if rule.decision == "join" {
                "join"
            } else {
                "split"
            },
            score,
            evidence: vec![rule.id.clone()],
            hard_violation: 0,
        },
        alternative: BoundaryDecisionOption {
            decision: alternative_decision,
            score: if hard { -score } else { 0 },
            evidence: vec![format!("{}_counterevidence", rule.id)],
            hard_violation: usize::from(hard),
        },
    }
}

#[derive(Clone)]
struct BunsetsuPath {
    score: i32,
    hard_violations: usize,
    unresolved: usize,
    spans: Vec<(usize, usize)>,
}

fn path_is_better(candidate: &BunsetsuPath, current: &BunsetsuPath) -> bool {
    candidate.score > current.score
        || candidate.score == current.score
            && (candidate.hard_violations < current.hard_violations
                || candidate.hard_violations == current.hard_violations
                    && (candidate.unresolved < current.unresolved
                        || candidate.unresolved == current.unresolved
                            && (candidate.spans.len() < current.spans.len()
                                || candidate.spans.len() == current.spans.len()
                                    && candidate.spans < current.spans)))
}

fn choose_bunsetsu_path(
    morphemes: &[Morpheme],
    atomic_joins: &HashSet<usize>,
    features: &BoundaryFeatures,
    catalog: &BunsetsuCatalog,
) -> BunsetsuPath {
    let n = morphemes.len();
    let mut best: Vec<Option<BunsetsuPath>> = vec![None; n + 1];
    best[0] = Some(BunsetsuPath {
        score: 0,
        hard_violations: 0,
        unresolved: 0,
        spans: Vec::new(),
    });
    for start in 0..n {
        let Some(prefix) = best[start].clone() else {
            continue;
        };
        let mut join_score = 0;
        let mut join_hard = 0;
        let mut join_unresolved = 0;
        for end in start + 1..=n {
            if end > start + 1 {
                let options =
                    boundary_options(morphemes, start, end - 1, atomic_joins, features, catalog);
                let join = if options.preferred.decision == "join" {
                    &options.preferred
                } else {
                    &options.alternative
                };
                if join.hard_violation > 0 {
                    break;
                }
                join_score += join.score;
                join_hard += join.hard_violation;
                join_unresolved +=
                    usize::from(options.preferred.score == options.alternative.score);
            }
            let (split_score, split_hard, split_unresolved) = if end < n {
                let options =
                    boundary_options(morphemes, start, end, atomic_joins, features, catalog);
                let split = if options.preferred.decision == "split" {
                    &options.preferred
                } else {
                    &options.alternative
                };
                (
                    split.score,
                    split.hard_violation,
                    usize::from(options.preferred.score == options.alternative.score),
                )
            } else {
                (0, 0, 0)
            };
            if split_hard > 0 {
                continue;
            }
            let mut candidate = prefix.clone();
            candidate.score += join_score + split_score;
            candidate.hard_violations += join_hard + split_hard;
            candidate.unresolved += join_unresolved + split_unresolved;
            candidate.spans.push((start, end));
            if best[end]
                .as_ref()
                .is_none_or(|current| path_is_better(&candidate, current))
            {
                best[end] = Some(candidate);
            }
        }
    }
    best[n].take().expect("非空语素序列必须存在文节路径")
}

fn infer_function(bunsetsu: &Bunsetsu) -> BunsetsuFunctionAnnotation {
    let last = bunsetsu.morphemes.last();
    let head = &bunsetsu.head_word.pos;
    let (function, confidence, evidence) =
        if last.is_some_and(|item| item.pos.major == "助詞" && item.pos.sub1 == "格助詞") {
            (BunsetsuFunction::CasePhrase, 90, "ends_with_case_particle")
        } else if last.is_some_and(|item| item.base_form == "の" && item.pos.major == "助詞") {
            (BunsetsuFunction::Adnominal, 85, "ends_with_adnominal_no")
        } else if last.is_some_and(|item| item.pos.major == "助詞" && item.pos.sub1 == "接続助詞")
        {
            (
                BunsetsuFunction::Conjunctive,
                85,
                "ends_with_conjunctive_particle",
            )
        } else if matches!(head.major.as_str(), "動詞" | "形容詞")
            || bunsetsu
                .morphemes
                .iter()
                .any(|item| matches!(item.pos.major.as_str(), "動詞" | "形容詞" | "助動詞"))
        {
            (BunsetsuFunction::Predicate, 90, "predicate_core")
        } else if head.major == "副詞" {
            (BunsetsuFunction::Adverbial, 85, "adverbial_core")
        } else if head.major == "名詞" {
            (BunsetsuFunction::Nominal, 75, "nominal_core")
        } else {
            (BunsetsuFunction::Unknown, 40, "insufficient_local_evidence")
        };
    BunsetsuFunctionAnnotation {
        function,
        confidence,
        evidence: vec![evidence.to_string()],
        syntax_evidence: Vec::new(),
    }
}

/// 构造单个文节，并提取其核心自立语 (HeadWord) 与属性
pub(crate) fn build_bunsetsu(morphemes: Vec<Morpheme>) -> Bunsetsu {
    build_bunsetsu_with_annotations(morphemes, Vec::new(), Vec::new())
}

pub(crate) fn build_bunsetsu_with_formations(
    morphemes: Vec<Morpheme>,
    word_formations: Vec<WordFormationAnnotation>,
) -> Bunsetsu {
    build_bunsetsu_with_annotations(morphemes, word_formations, Vec::new())
}

pub(crate) fn build_bunsetsu_with_annotations(
    morphemes: Vec<Morpheme>,
    word_formations: Vec<WordFormationAnnotation>,
    lexical_units: Vec<DictionaryLexicalUnitAnnotation>,
) -> Bunsetsu {
    // 拼接表层形
    let surface: String = morphemes.iter().map(|m| m.surface.as_str()).collect();

    // 确定字符偏移区间
    let start = morphemes.first().map(|m| m.char_range.0).unwrap_or(0);
    let end = morphemes.last().map(|m| m.char_range.1).unwrap_or(0);

    // 提取核心自立语
    // 优先选择文节中第一个满足 is_jiritsugo 且不是 "接頭詞" 的形态素
    // 如果找不到，则回退选择第一个自立语，或者整个文节的第一个形态素
    let head_index = morphemes
        .iter()
        .position(|m| is_jiritsugo(m) && m.pos.major != "接頭詞")
        .or_else(|| morphemes.iter().position(is_jiritsugo))
        .unwrap_or(0);
    let head_morpheme = &morphemes[head_index];

    // Keep the candidate available; the dictionary-aware resolver decides whether
    // nominal suffixes belong to the lexical head.
    let mut head_surface = head_morpheme.surface.clone();
    let mut head_base_form = head_morpheme.base_form.clone();
    let mut head_reading = head_morpheme.reading.clone();
    if head_morpheme.pos.major == "名詞" {
        for suffix in morphemes.iter().skip(head_index + 1) {
            if suffix.pos.major != "名詞" || suffix.pos.sub1 != "接尾" {
                break;
            }
            head_surface.push_str(&suffix.surface);
            head_base_form.push_str(&suffix.base_form);
            if suffix.reading != "*" {
                if head_reading == "*" {
                    head_reading.clear();
                }
                head_reading.push_str(&suffix.reading);
            }
        }
    }

    let head_word = HeadWord {
        surface: head_surface,
        base_form: head_base_form,
        reading: head_reading,
        pos: head_morpheme.pos.clone(),
    };

    let mut bunsetsu = Bunsetsu {
        morphemes,
        surface,
        head_word,
        grammar_tags: Vec::new(), // 在后续的语法匹配阶段填充
        word_formations,
        lexical_units,
        function: None,
        char_range: (start, end),
    };

    bunsetsu.head_word.base_form = super::restore::restore_base_form(&bunsetsu);
    if let Some(formation) = bunsetsu.word_formations.iter().find(|formation| {
        formation.morpheme_range.0 <= head_index && head_index < formation.morpheme_range.1
    }) {
        bunsetsu.head_word.surface = formation.surface.clone();
        bunsetsu.head_word.base_form = formation.base_form.clone();
        bunsetsu.head_word.reading = formation.reading.clone();
        bunsetsu.head_word.pos = formation.output_pos.clone();
    }
    if let Some(lexical_unit) = bunsetsu
        .lexical_units
        .iter()
        .find(|lexical_unit| {
            lexical_unit.morpheme_range.0 <= head_index
                && head_index < lexical_unit.morpheme_range.1
        })
        .or_else(|| bunsetsu.lexical_units.first())
    {
        bunsetsu.head_word.surface = lexical_unit.surface.clone();
        bunsetsu.head_word.base_form = lexical_unit.base_form.clone();
        bunsetsu.head_word.reading = lexical_unit.reading.clone();
        bunsetsu.head_word.pos = lexical_unit.output_pos.clone();
    }
    bunsetsu
}

#[cfg(test)]
mod tests {
    #[test]
    fn candidate_path_combines_prefix_with_atomic_formation() {
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
        let reports = pipeline.inspect_bunsetsu("お二人で行く。");
        let report = reports.first().expect("文节审计应返回内容段");
        assert_eq!(
            report
                .bunsetsus
                .iter()
                .map(|bunsetsu| bunsetsu.surface.as_str())
                .collect::<Vec<_>>(),
            vec!["お二人で", "行く"]
        );
        assert!(report.reconstruction_ok && report.range_integrity_ok);
        assert_eq!(report.unresolved_boundaries, 0);
        assert!(report.boundaries.iter().any(|boundary| {
            boundary.decision == "join"
                && boundary
                    .evidence
                    .iter()
                    .any(|evidence| evidence == "prefix_attachment")
        }));

        for (text, expected) in [
            ("超速い。", "超速い"),
            ("くそだるい。", "くそだるい"),
            ("少しお借りします。", "お借りします"),
        ] {
            let reports = pipeline.inspect_bunsetsu(text);
            assert!(
                reports
                    .iter()
                    .flat_map(|report| &report.bunsetsus)
                    .any(|bunsetsu| { bunsetsu.surface == expected }),
                "{text} 未通过接续分支形成 {expected}"
            );
        }

        let compatibility = pipeline.inspect_bunsetsu("クソだっせえ。");
        assert!(compatibility
            .iter()
            .flat_map(|report| &report.bunsetsus)
            .any(|bunsetsu| bunsetsu.surface == "クソだっせえ"));
        assert!(compatibility
            .iter()
            .flat_map(|report| &report.bunsetsus)
            .flat_map(|bunsetsu| &bunsetsu.morphemes)
            .any(|morpheme| morpheme.surface == "だっせえ"
                && morpheme.base_form == "ダサい"
                && morpheme.pos.major == "形容詞"));
    }
}
