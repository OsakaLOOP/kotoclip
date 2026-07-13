use crate::models::{
    AnnotatedToken, ExpressionAnnotation, ExpressionCandidate, ExpressionCandidateCapture,
    ExpressionCandidateStatus, RuleCatalogAudit,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::OnceLock;

/// 对所有候选来源执行统一的优先级排序与同类范围排重。
pub fn resolve_expression_conflicts(tokens: &mut [AnnotatedToken]) {
    for token in tokens {
        token.expressions.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| right.confidence.total_cmp(&left.confidence))
                .then_with(|| {
                    let left_width = left.char_range.1.saturating_sub(left.char_range.0);
                    let right_width = right.char_range.1.saturating_sub(right.char_range.0);
                    right_width.cmp(&left_width)
                })
        });
        let mut seen = HashSet::new();
        token
            .expressions
            .retain(|item| seen.insert((item.expression_type.clone(), item.char_range)));
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpressionCatalog {
    schema_version: u32,
    catalog_version: u32,
    source: String,
    patterns: Vec<BuiltinExpressionPattern>,
    #[serde(default)]
    correlative_patterns: Vec<CorrelativePattern>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorrelativePattern {
    id: String,
    #[serde(default = "default_rule_version")]
    rule_version: u32,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
    label: String,
    description: String,
    category: String,
    head_atoms: Vec<ExpressionAtom>,
    tail_variants: Vec<Vec<ExpressionAtom>>,
    #[serde(default = "default_gap_bunsetsu")]
    gap_bunsetsu: (usize, usize),
    #[serde(default)]
    examples: Vec<String>,
    #[serde(default)]
    counter_examples: Vec<String>,
}

fn default_gap_bunsetsu() -> (usize, usize) {
    (0, 10)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuiltinExpressionPattern {
    id: String,
    label: String,
    description: String,
    category: String,
    #[serde(default = "default_builtin_expression_type")]
    expression_type: String,
    #[serde(default = "default_rule_version")]
    rule_version: u32,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
    #[serde(default)]
    priority: Option<i32>,
    #[serde(default = "default_expression_confidence")]
    confidence: u8,
    atoms: Vec<ExpressionAtom>,
    #[serde(default)]
    examples: Vec<String>,
    #[serde(default)]
    counter_examples: Vec<String>,
    #[serde(default)]
    requires_following_content: bool,
}

fn default_builtin_expression_type() -> String {
    "grammar_construction".to_string()
}
fn default_rule_version() -> u32 {
    1
}
fn enabled_by_default() -> bool {
    true
}
fn default_expression_confidence() -> u8 {
    95
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpressionAtom {
    #[serde(default)]
    lemmas: Vec<String>,
    #[serde(default)]
    surfaces: Vec<String>,
    #[serde(default)]
    pos: Option<String>,
    #[serde(default)]
    pos_sub1: Option<String>,
    #[serde(default)]
    pos_sub2: Option<String>,
    #[serde(default)]
    pos_sub3: Option<String>,
    #[serde(default)]
    conjugation_type: Option<String>,
    #[serde(default)]
    conjugation_types: Vec<String>,
    #[serde(default)]
    conjugation_type_prefixes: Vec<String>,
    #[serde(default)]
    conjugation_forms: Vec<String>,
    #[serde(default)]
    capture: Option<String>,
    #[serde(default = "one_atom")]
    min: usize,
    #[serde(default = "one_atom")]
    max: usize,
}

fn one_atom() -> usize {
    1
}

#[derive(Debug)]
struct FlatMorpheme {
    token_index: usize,
    surface: String,
    lemma: String,
    pos: String,
    pos_sub1: String,
    pos_sub2: String,
    pos_sub3: String,
    conjugation_type: String,
    conjugation_form: String,
    char_range: (usize, usize),
}

fn catalog() -> &'static ExpressionCatalog {
    static CATALOG: OnceLock<ExpressionCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog: ExpressionCatalog =
            serde_json::from_str(include_str!("../../resources/expression_patterns.json"))
                .expect("内置跨文节表达目录格式无效");
        validate_catalog(&catalog).expect("内置表达目录语义校验失败");
        catalog
    })
}

fn validate_catalog(catalog: &ExpressionCatalog) -> Result<(), String> {
    if catalog.schema_version != 2
        || catalog.catalog_version == 0
        || catalog.source.trim().is_empty()
    {
        return Err("表达目录版本或来源非法".to_string());
    }
    let mut ids = HashSet::new();
    for pattern in &catalog.patterns {
        if pattern.id.trim().is_empty()
            || !ids.insert(pattern.id.clone())
            || pattern.rule_version == 0
            || pattern.confidence > 100
            || pattern.atoms.is_empty()
            || pattern.examples.is_empty()
            || pattern.counter_examples.is_empty()
            || !matches!(
                pattern.expression_type.as_str(),
                "idiom" | "grammar_construction"
            )
        {
            return Err(format!("连续表达规则非法：{}", pattern.id));
        }
        for atom in &pattern.atoms {
            validate_expression_atom(atom).map_err(|reason| format!("{}：{reason}", pattern.id))?;
        }
    }
    for pattern in &catalog.correlative_patterns {
        if pattern.id.trim().is_empty()
            || !ids.insert(pattern.id.clone())
            || pattern.rule_version == 0
            || pattern.head_atoms.is_empty()
            || pattern.tail_variants.is_empty()
            || pattern.examples.is_empty()
            || pattern.counter_examples.is_empty()
            || pattern.gap_bunsetsu.0 > pattern.gap_bunsetsu.1
            || pattern.gap_bunsetsu.1 > 64
        {
            return Err(format!("呼应表达规则非法：{}", pattern.id));
        }
        for atom in pattern
            .head_atoms
            .iter()
            .chain(pattern.tail_variants.iter().flatten())
        {
            validate_expression_atom(atom).map_err(|reason| format!("{}：{reason}", pattern.id))?;
            if atom.min != 1 || atom.max != 1 || atom.capture.is_some() {
                return Err(format!(
                    "呼应锚点当前只允许单次原子且不允许捕获：{}",
                    pattern.id
                ));
            }
        }
    }
    Ok(())
}

fn validate_expression_atom(atom: &ExpressionAtom) -> Result<(), &'static str> {
    if atom.min == 0 || atom.max < atom.min || atom.max > 8 {
        return Err("重复范围非法");
    }
    if atom.lemmas.is_empty()
        && atom.surfaces.is_empty()
        && atom.pos.is_none()
        && atom.pos_sub1.is_none()
        && atom.pos_sub2.is_none()
        && atom.pos_sub3.is_none()
        && atom.conjugation_type.is_none()
        && atom.conjugation_types.is_empty()
        && atom.conjugation_type_prefixes.is_empty()
        && atom.conjugation_forms.is_empty()
    {
        return Err("原子缺少约束");
    }
    if atom.pos_sub2.is_some() && atom.pos_sub1.is_none()
        || atom.pos_sub3.is_some() && atom.pos_sub2.is_none()
    {
        return Err("四级词性层级非法");
    }
    Ok(())
}

pub fn catalog_audit() -> RuleCatalogAudit {
    let catalog = catalog();
    RuleCatalogAudit {
        layer: "expression".to_string(),
        schema_version: catalog.schema_version,
        catalog_version: catalog.catalog_version,
        rule_count: catalog.patterns.len() + catalog.correlative_patterns.len(),
        enabled_rule_count: catalog.patterns.iter().filter(|rule| rule.enabled).count()
            + catalog
                .correlative_patterns
                .iter()
                .filter(|rule| rule.enabled)
                .count(),
        capabilities: vec![
            "surface_set".to_string(),
            "base_form_set".to_string(),
            "four_level_pos".to_string(),
            "conjugation_type_exact".to_string(),
            "conjugation_type_prefix".to_string(),
            "conjugation_form_set".to_string(),
            "bounded_repeat_continuous".to_string(),
            "named_capture_continuous".to_string(),
            "finite_bunsetsu_gap".to_string(),
            "nearest_tail_same_domain".to_string(),
            "accepted_pending_rejected".to_string(),
            "discontinuous_matched_ranges".to_string(),
            "strict_unknown_field_rejection".to_string(),
        ],
    }
}

fn flatten(tokens: &[AnnotatedToken]) -> Vec<FlatMorpheme> {
    tokens
        .iter()
        .enumerate()
        .flat_map(|(token_index, token)| {
            if token.display_class != "content" {
                return vec![FlatMorpheme {
                    token_index,
                    surface: token.bunsetsu.surface.clone(),
                    lemma: token.bunsetsu.surface.clone(),
                    pos: "記号".to_string(),
                    pos_sub1: String::new(),
                    pos_sub2: String::new(),
                    pos_sub3: String::new(),
                    conjugation_type: String::new(),
                    conjugation_form: String::new(),
                    char_range: token.bunsetsu.char_range,
                }];
            }
            token
                .bunsetsu
                .morphemes
                .iter()
                .filter_map(move |morpheme| {
                    if morpheme.surface.trim().is_empty() {
                        return None;
                    }
                    Some(FlatMorpheme {
                        token_index,
                        surface: morpheme.surface.clone(),
                        lemma: if morpheme.base_form.is_empty() || morpheme.base_form == "*" {
                            morpheme.surface.clone()
                        } else {
                            morpheme.base_form.clone()
                        },
                        pos: morpheme.pos.major.clone(),
                        pos_sub1: morpheme.pos.sub1.clone(),
                        pos_sub2: morpheme.pos.sub2.clone(),
                        pos_sub3: morpheme.pos.sub3.clone(),
                        conjugation_type: morpheme.conjugation_type.clone(),
                        conjugation_form: morpheme.conjugation_form.clone(),
                        char_range: morpheme.char_range,
                    })
                })
                .collect()
        })
        .collect()
}

fn atom_matches(atom: &ExpressionAtom, morpheme: &FlatMorpheme) -> bool {
    (atom.lemmas.is_empty() || atom.lemmas.iter().any(|lemma| lemma == &morpheme.lemma))
        && (atom.surfaces.is_empty()
            || atom
                .surfaces
                .iter()
                .any(|surface| surface == &morpheme.surface))
        && atom.pos.as_ref().map_or(true, |pos| pos == &morpheme.pos)
        && atom
            .pos_sub1
            .as_ref()
            .map_or(true, |pos| pos == &morpheme.pos_sub1)
        && atom
            .pos_sub2
            .as_ref()
            .map_or(true, |pos| pos == &morpheme.pos_sub2)
        && atom
            .pos_sub3
            .as_ref()
            .map_or(true, |pos| pos == &morpheme.pos_sub3)
        && atom
            .conjugation_type
            .as_ref()
            .map_or(true, |value| value == &morpheme.conjugation_type)
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
struct ExpressionAtomSpan {
    atom_index: usize,
    start: usize,
    end: usize,
}

struct ExpressionSequenceMatch {
    end: usize,
    atoms: Vec<ExpressionAtomSpan>,
}

fn match_expression_atoms(
    atoms: &[ExpressionAtom],
    morphemes: &[FlatMorpheme],
    atom_index: usize,
    cursor: usize,
    matched: &mut Vec<ExpressionAtomSpan>,
    output: &mut Vec<ExpressionSequenceMatch>,
) {
    if atom_index == atoms.len() {
        output.push(ExpressionSequenceMatch {
            end: cursor,
            atoms: matched.clone(),
        });
        return;
    }
    let atom = &atoms[atom_index];
    let mut maximum = 0;
    while maximum < atom.max
        && cursor + maximum < morphemes.len()
        && atom_matches(atom, &morphemes[cursor + maximum])
    {
        maximum += 1;
    }
    if maximum < atom.min {
        return;
    }
    for count in (atom.min..=maximum).rev() {
        matched.push(ExpressionAtomSpan {
            atom_index,
            start: cursor,
            end: cursor + count,
        });
        match_expression_atoms(
            atoms,
            morphemes,
            atom_index + 1,
            cursor + count,
            matched,
            output,
        );
        matched.pop();
    }
}

/// 在跨文节的完整语素流上运行确定性状态机。匹配范围从第一个锚点语素开始，
/// 因此 `兄に｜負けるところが｜多い` 的参数“兄”不会被误标为表达本体。
struct AcceptedBuiltinCandidate {
    candidate: ExpressionCandidate,
    legacy_rule_id: i64,
    priority: i32,
}

fn accepted_builtin_candidates(tokens: &[AnnotatedToken]) -> Vec<AcceptedBuiltinCandidate> {
    let morphemes = flatten(tokens);
    let mut seen = HashSet::new();
    let mut output = Vec::new();

    for (pattern_index, pattern) in catalog()
        .patterns
        .iter()
        .enumerate()
        .filter(|(_, pattern)| pattern.enabled)
    {
        for start in 0..morphemes.len() {
            let mut matches = Vec::new();
            match_expression_atoms(
                &pattern.atoms,
                &morphemes,
                0,
                start,
                &mut Vec::new(),
                &mut matches,
            );
            for matched in matches {
                if pattern.requires_following_content
                    && !morphemes[matched.end..]
                        .iter()
                        .take_while(|morpheme| !is_sentence_boundary(morpheme))
                        .any(|morpheme| {
                            !morpheme.surface.trim().is_empty() && morpheme.pos != "記号"
                        })
                {
                    continue;
                }
                let window = &morphemes[start..matched.end];
                let start_token = window.first().unwrap().token_index;
                let end_token = window.last().unwrap().token_index + 1;
                let match_id = format!(
                    "builtin:{}:v{}:{}:{}",
                    pattern.id, pattern.rule_version, start_token, end_token
                );
                if !seen.insert(match_id.clone()) {
                    continue;
                }
                let char_range = (
                    window.first().unwrap().char_range.0,
                    window.last().unwrap().char_range.1,
                );
                let surface: String = window
                    .iter()
                    .map(|morpheme| morpheme.surface.as_str())
                    .collect();
                let captures = matched
                    .atoms
                    .iter()
                    .filter_map(|span| {
                        let name = pattern.atoms[span.atom_index].capture.as_ref()?;
                        Some(ExpressionCandidateCapture {
                            name: name.clone(),
                            surface: morphemes[span.start..span.end]
                                .iter()
                                .map(|morpheme| morpheme.surface.as_str())
                                .collect(),
                            morpheme_range: (span.start, span.end),
                            char_range: (
                                morphemes[span.start].char_range.0,
                                morphemes[span.end - 1].char_range.1,
                            ),
                        })
                    })
                    .collect();
                output.push(AcceptedBuiltinCandidate {
                    legacy_rule_id: -((pattern_index as i64) + 1),
                    priority: pattern
                        .priority
                        .unwrap_or(if pattern.expression_type == "idiom" {
                            70
                        } else {
                            60
                        }),
                    candidate: ExpressionCandidate {
                        candidate_id: match_id,
                        rule_id: pattern.id.clone(),
                        rule_version: pattern.rule_version,
                        origin: "builtin".to_string(),
                        expression_type: pattern.expression_type.clone(),
                        status: ExpressionCandidateStatus::Accepted,
                        confidence: pattern.confidence,
                        label: pattern.label.clone(),
                        description: format!("{}｜{}", pattern.category, pattern.description),
                        matched_ranges: vec![char_range],
                        covered_token_range: (start_token, end_token),
                        char_range,
                        surface,
                        captures,
                        evidence: vec!["all_atoms_matched".to_string()],
                        counter_evidence: Vec::new(),
                        rejection_reason: None,
                        entry_key: None,
                    },
                });
            }
        }
    }
    output
}

pub fn builtin_expression_candidates(tokens: &[AnnotatedToken]) -> Vec<ExpressionCandidate> {
    accepted_builtin_candidates(tokens)
        .into_iter()
        .map(|item| item.candidate)
        .collect()
}

pub fn apply_builtin_expressions(tokens: &mut [AnnotatedToken]) -> usize {
    let candidates = accepted_builtin_candidates(tokens);
    for item in &candidates {
        let candidate = &item.candidate;
        let (start_token, end_token) = candidate.covered_token_range;
        let width = end_token - start_token;
        for (offset, token) in tokens[start_token..end_token].iter_mut().enumerate() {
            let position = if width == 1 {
                "single"
            } else if offset == 0 {
                "start"
            } else if offset + 1 == width {
                "end"
            } else {
                "middle"
            };
            token.expressions.push(ExpressionAnnotation {
                match_id: candidate.candidate_id.clone(),
                rule_id: item.legacy_rule_id,
                label: candidate.label.clone(),
                description: candidate.description.clone(),
                origin: candidate.origin.clone(),
                expression_type: candidate.expression_type.clone(),
                priority: item.priority,
                boundary_effect: "annotate_only".to_string(),
                confidence: f32::from(candidate.confidence) / 100.0,
                position: position.to_string(),
                token_range: candidate.covered_token_range,
                char_range: candidate.char_range,
                matched_ranges: candidate.matched_ranges.clone(),
                surface: candidate.surface.clone(),
            });
        }
    }
    candidates.len()
}

struct MatchInfo {
    query: String,
    s: usize,
    e: usize,
    char_range: (usize, usize),
    surface: String,
    expression_type: String,
    confidence: f32,
}

/// 词典存在性只提供候选证据；组合方式决定候选能否成为正式结果。
/// 以助词起始的「に＋つく」「と＋する」等普通句法不进入自动表达层。
fn classify_dictionary_composition(
    morphemes: &[&crate::models::Morpheme],
) -> Option<(&'static str, f32)> {
    let first = morphemes.first()?;
    if first.pos.major == "助詞" || first.pos.major == "助動詞" {
        return None;
    }
    let particle_count = morphemes.iter().filter(|m| m.pos.major == "助詞").count();
    if particle_count > 0 {
        let has_content_head = matches!(first.pos.major.as_str(), "名詞" | "動詞" | "形容詞");
        let has_predicate = morphemes
            .iter()
            .skip(1)
            .any(|m| matches!(m.pos.major.as_str(), "動詞" | "形容詞"));
        if morphemes.len() >= 3 && has_content_head && has_predicate {
            return Some(("idiom", 0.82));
        }
        return None;
    }
    if first.pos.sub1 == "代名詞" {
        return None;
    }
    let lexical_shape = morphemes.iter().all(|m| {
        matches!(m.pos.major.as_str(), "名詞" | "接頭詞")
            || (m.pos.major == "動詞" && m.pos.sub1 != "自立")
    }) || (morphemes.len() >= 2
        && first.pos.major == "名詞"
        && morphemes.last().is_some_and(|m| m.pos.major == "動詞"));
    lexical_shape.then_some(("unclassified_dictionary_phrase", 0.7))
}

/// 基于本地词典自动匹配的跨文节固定表达扫描
pub fn dictionary_expression_candidates(
    tokens: &[AnnotatedToken],
    dictionary: &crate::dictionary::lookup::DictionaryEngine,
) -> Vec<ExpressionCandidate> {
    let mut matched_char_ranges: Vec<(usize, usize)> = Vec::new();
    let mut matches_found: Vec<MatchInfo> = Vec::new();
    let mut candidates = Vec::new();
    let mut unique_queries = HashSet::new();

    // 第一阶段只生成合法候选。顺序保持为长窗口优先、起点优先，
    // 以便批量查询后复用原有的最长范围选择语义。
    for n in (2..=4).rev() {
        if n > tokens.len() {
            continue;
        }
        for s in 0..=tokens.len() - n {
            let e = s + n - 1;
            if tokens[s..=e]
                .iter()
                .any(|token| token.display_class != "content")
            {
                continue;
            }
            let ts = &tokens[s].bunsetsu;
            let te = &tokens[e].bunsetsu;

            for p in 0..ts.morphemes.len() {
                for q in 0..te.morphemes.len() {
                    let last_m = &te.morphemes[q];
                    let valid_last_pos = matches!(
                        last_m.pos.major.as_str(),
                        "動詞" | "形容詞" | "助動詞" | "名詞"
                    );
                    if !valid_last_pos {
                        continue;
                    }

                    let char_range = (ts.morphemes[p].char_range.0, te.morphemes[q].char_range.1);
                    let mut query = String::new();
                    let mut surface = String::new();
                    let mut invalid = false;
                    let mut selected_morphemes = Vec::new();
                    for token_index in s..=e {
                        let morphemes = &tokens[token_index].bunsetsu.morphemes;
                        let start = if token_index == s { p } else { 0 };
                        let end = if token_index == e {
                            q + 1
                        } else {
                            morphemes.len()
                        };
                        for (index, morpheme) in morphemes[start..end].iter().enumerate() {
                            if morpheme.surface.trim().is_empty() || morpheme.pos.major == "記号"
                            {
                                invalid = true;
                                break;
                            }
                            selected_morphemes.push(morpheme);
                            surface.push_str(&morpheme.surface);
                            let is_last = token_index == e && start + index == q;
                            if is_last
                                && !morpheme.base_form.is_empty()
                                && morpheme.base_form != "*"
                            {
                                query.push_str(&morpheme.base_form);
                            } else {
                                query.push_str(&morpheme.surface);
                            }
                        }
                        if invalid {
                            break;
                        }
                    }
                    if invalid {
                        continue;
                    }
                    let Some((expression_type, confidence)) =
                        classify_dictionary_composition(&selected_morphemes)
                    else {
                        continue;
                    };
                    unique_queries.insert(query.clone());
                    candidates.push(MatchInfo {
                        query,
                        s,
                        e,
                        char_range,
                        surface,
                        expression_type: expression_type.to_string(),
                        confidence,
                    });
                }
            }
        }
    }

    // 第二阶段一次批量查询所有唯一候选，再按原顺序选择每个窗口首个最长命中。
    let dictionary_matches = dictionary.contains_exact_batch(&unique_queries);
    let mut claimed_windows = HashSet::new();
    for candidate in candidates {
        if claimed_windows.contains(&(candidate.s, candidate.e))
            || !dictionary_matches.contains(&candidate.query)
            || matched_char_ranges.iter().any(|&(start, end)| {
                start <= candidate.char_range.0 && candidate.char_range.1 <= end
            })
        {
            continue;
        }
        claimed_windows.insert((candidate.s, candidate.e));
        matched_char_ranges.push(candidate.char_range);
        matches_found.push(candidate);
    }

    // 第三阶段：词典存在性不足以确认惯用义，统一输出 pending 候选。
    let mut result = Vec::new();
    for info in matches_found {
        if tokens[info.s..=info.e].iter().any(|token| {
            token
                .expressions
                .iter()
                .any(|expression| expression.char_range == info.char_range)
        }) {
            continue;
        }
        let match_id = format!("dict:{}:{}:{}", info.query, info.s, info.e + 1);
        result.push(ExpressionCandidate {
            candidate_id: match_id,
            rule_id: "dictionary_exact_window".to_string(),
            rule_version: 1,
            origin: "dictionary".to_string(),
            expression_type: info.expression_type,
            status: ExpressionCandidateStatus::Pending,
            confidence: (info.confidence * 100.0) as u8,
            label: info.query,
            description: "词典存在性候选；尚无结构化惯用语证据。".to_string(),
            matched_ranges: vec![info.char_range],
            covered_token_range: (info.s, info.e + 1),
            char_range: info.char_range,
            surface: info.surface,
            captures: Vec::new(),
            evidence: vec![
                "dictionary_exact_headword".to_string(),
                "composition_shape_compatible".to_string(),
            ],
            counter_evidence: vec!["missing_structured_idiom_evidence".to_string()],
            rejection_reason: None,
            entry_key: None,
        });
    }
    result
}

pub fn rejected_builtin_candidates(tokens: &[AnnotatedToken]) -> Vec<ExpressionCandidate> {
    let morphemes = flatten(tokens);
    let mut result = Vec::new();
    for window in morphemes.windows(2) {
        if window[0].lemma == "こと"
            && window[1].lemma == "ない"
            && (window[1].surface != "なく" || window[1].conjugation_form != "連用テ接続")
        {
            let char_range = (window[0].char_range.0, window[1].char_range.1);
            result.push(ExpressionCandidate {
                candidate_id: format!("rejected:negative_koto_naku:{}", char_range.0),
                rule_id: "builtin_negative_koto_naku".to_string(),
                rule_version: 2,
                origin: "builtin".to_string(),
                expression_type: "grammar_construction".to_string(),
                status: ExpressionCandidateStatus::Rejected,
                confidence: 100,
                label: "〜ことなく".to_string(),
                description: "否定成分不是连接形「なく」。".to_string(),
                matched_ranges: vec![char_range],
                covered_token_range: (window[0].token_index, window[1].token_index + 1),
                char_range,
                surface: format!("{}{}", window[0].surface, window[1].surface),
                captures: Vec::new(),
                evidence: vec!["koto_followed_by_nai".to_string()],
                counter_evidence: vec!["negative_not_connective_form".to_string()],
                rejection_reason: Some("conjugation_form_mismatch".to_string()),
                entry_key: None,
            });
        }
    }
    for (index, window) in morphemes.windows(2).enumerate() {
        if window[0].lemma == "こと"
            && window[1].lemma == "ない"
            && window[1].surface == "なく"
            && window[1].conjugation_form == "連用テ接続"
            && !morphemes[index + 2..]
                .iter()
                .take_while(|morpheme| !is_sentence_boundary(morpheme))
                .any(|morpheme| !morpheme.surface.trim().is_empty() && morpheme.pos != "記号")
        {
            let char_range = (window[0].char_range.0, window[1].char_range.1);
            result.push(ExpressionCandidate {
                candidate_id: format!("rejected:negative_koto_naku_following:{}", char_range.0),
                rule_id: "negative_koto_naku".to_string(),
                rule_version: 2,
                origin: "builtin".to_string(),
                expression_type: "grammar_construction".to_string(),
                status: ExpressionCandidateStatus::Rejected,
                confidence: 100,
                label: "〜ことなく".to_string(),
                description: "连接形「なく」之后缺少被连接的内容。".to_string(),
                matched_ranges: vec![char_range],
                covered_token_range: (window[0].token_index, window[1].token_index + 1),
                char_range,
                surface: format!("{}{}", window[0].surface, window[1].surface),
                captures: Vec::new(),
                evidence: vec!["connective_koto_naku".to_string()],
                counter_evidence: vec!["missing_following_clause".to_string()],
                rejection_reason: Some("missing_following_clause".to_string()),
                entry_key: None,
            });
        }
    }
    result
}

fn is_sentence_boundary(morpheme: &FlatMorpheme) -> bool {
    morpheme.lemma == "。"
        || morpheme.lemma == "！"
        || morpheme.lemma == "？"
        || morpheme.surface.contains('。')
        || morpheme.surface.contains('！')
        || morpheme.surface.contains('？')
        || morpheme.surface.contains('\n')
        || morpheme.surface.contains('?')
        || morpheme.surface.contains('!')
        || (morpheme.lemma == "と" && morpheme.pos == "助詞" && morpheme.pos_sub2 == "引用")
}

#[derive(Clone)]
struct CorrelativeMatchInfo {
    id: String,
    label: String,
    category: String,
    description: String,
    rule_version: u32,
    rule_idx: usize,
    start_token: usize,
    end_token: usize,
    char_range: (usize, usize),
    matched_ranges: Vec<(usize, usize)>,
    surface: String,
}

fn find_correlative_matches(tokens: &[AnnotatedToken]) -> Vec<CorrelativeMatchInfo> {
    let morphemes = flatten(tokens);
    let mut matches: Vec<CorrelativeMatchInfo> = Vec::new();
    let catalog = catalog();

    for (pattern_index, pattern) in catalog
        .correlative_patterns
        .iter()
        .enumerate()
        .filter(|(_, pattern)| pattern.enabled)
    {
        let head_len = pattern.head_atoms.len();
        if head_len == 0 || head_len > morphemes.len() {
            continue;
        }

        for h_start in 0..=morphemes.len() - head_len {
            let h_window = &morphemes[h_start..h_start + head_len];
            if !pattern
                .head_atoms
                .iter()
                .zip(h_window)
                .all(|(atom, m)| atom_matches(atom, m))
            {
                continue;
            }

            let head_start_token = h_window.first().unwrap().token_index;
            let head_end_token = h_window.last().unwrap().token_index;
            let h_last_morpheme_idx = h_start + head_len - 1;

            let mut best_match: Option<CorrelativeMatchInfo> = None;

            for tail_atoms in &pattern.tail_variants {
                let tail_len = tail_atoms.len();
                if tail_len == 0 || h_last_morpheme_idx + 1 + tail_len > morphemes.len() {
                    continue;
                }

                for t_start in (h_last_morpheme_idx + 1)..=morphemes.len() - tail_len {
                    let tail_start_token = morphemes[t_start].token_index;
                    if tail_start_token > head_end_token + 1 + pattern.gap_bunsetsu.1 {
                        break;
                    }
                    if tail_start_token < head_end_token + 1 + pattern.gap_bunsetsu.0 {
                        continue;
                    }
                    let t_window = &morphemes[t_start..t_start + tail_len];
                    if !tail_atoms
                        .iter()
                        .zip(t_window)
                        .all(|(atom, m)| atom_matches(atom, m))
                    {
                        continue;
                    }

                    let tail_end_token = t_window.last().unwrap().token_index + 1;

                    if tail_start_token <= head_end_token {
                        continue;
                    }

                    let gap_count = tail_start_token.saturating_sub(head_end_token + 1);
                    if gap_count < pattern.gap_bunsetsu.0 || gap_count > pattern.gap_bunsetsu.1 {
                        continue;
                    }

                    let mut has_boundary = false;
                    for m in &morphemes[h_last_morpheme_idx + 1..t_start] {
                        if is_sentence_boundary(m) {
                            has_boundary = true;
                            break;
                        }
                    }
                    if has_boundary {
                        continue;
                    }

                    let head_surface: String =
                        h_window.iter().map(|m| m.surface.as_str()).collect();
                    let tail_surface: String =
                        t_window.iter().map(|m| m.surface.as_str()).collect();
                    let surface = format!("{}……{}", head_surface, tail_surface);

                    let char_range = (
                        h_window.first().unwrap().char_range.0,
                        t_window.last().unwrap().char_range.1,
                    );

                    let cand = CorrelativeMatchInfo {
                        id: pattern.id.clone(),
                        label: pattern.label.clone(),
                        category: pattern.category.clone(),
                        description: pattern.description.clone(),
                        rule_version: pattern.rule_version,
                        rule_idx: pattern_index,
                        start_token: head_start_token,
                        end_token: tail_end_token,
                        char_range,
                        matched_ranges: vec![
                            (
                                h_window.first().unwrap().char_range.0,
                                h_window.last().unwrap().char_range.1,
                            ),
                            (
                                t_window.first().unwrap().char_range.0,
                                t_window.last().unwrap().char_range.1,
                            ),
                        ],
                        surface,
                    };

                    if let Some(ref current_best) = best_match {
                        // 呼应结构优先连接同一局部句法域内最近的相容尾项。
                        if cand.end_token < current_best.end_token {
                            best_match = Some(cand);
                        }
                    } else {
                        best_match = Some(cand);
                    }
                }
            }

            if let Some(m) = best_match {
                matches.push(m);
            }
        }
    }

    let mut final_matches = Vec::new();
    for i in 0..matches.len() {
        let m_i = &matches[i];
        let mut is_sub = false;
        for j in 0..matches.len() {
            if i == j {
                continue;
            }
            let m_j = &matches[j];
            if m_j.char_range.0 <= m_i.char_range.0 && m_i.char_range.1 <= m_j.char_range.1 {
                if m_j.char_range == m_i.char_range {
                    if j < i {
                        is_sub = true;
                        break;
                    }
                } else {
                    is_sub = true;
                    break;
                }
            }
        }
        if !is_sub {
            final_matches.push(m_i.clone());
        }
    }
    final_matches
}

pub fn correlative_expression_candidates(tokens: &[AnnotatedToken]) -> Vec<ExpressionCandidate> {
    find_correlative_matches(tokens)
        .into_iter()
        .map(|m| ExpressionCandidate {
            candidate_id: format!("correlative:{}:{}:{}", m.id, m.start_token, m.end_token),
            rule_id: m.id,
            rule_version: m.rule_version,
            origin: "correlative".to_string(),
            expression_type: "correlative".to_string(),
            status: ExpressionCandidateStatus::Accepted,
            confidence: 90,
            label: m.label,
            description: format!("{}｜{}", m.category, m.description),
            matched_ranges: m.matched_ranges,
            covered_token_range: (m.start_token, m.end_token),
            char_range: m.char_range,
            surface: m.surface,
            captures: Vec::new(),
            evidence: vec![
                "nearest_compatible_tail".to_string(),
                "same_boundary_domain".to_string(),
            ],
            counter_evidence: Vec::new(),
            rejection_reason: None,
            entry_key: None,
        })
        .collect()
}

/// 非连续呼应表达匹配层
pub fn apply_correlative_expressions(tokens: &mut [AnnotatedToken]) -> usize {
    let matches = find_correlative_matches(tokens);
    for m in &matches {
        let match_id = format!("correlative:{}:{}:{}", m.id, m.start_token, m.end_token);
        let width = m.end_token - m.start_token;

        for (offset, token) in tokens[m.start_token..m.end_token].iter_mut().enumerate() {
            let position = if offset == 0 {
                "start"
            } else if offset + 1 == width {
                "end"
            } else {
                "middle"
            };

            token.expressions.push(ExpressionAnnotation {
                match_id: match_id.clone(),
                rule_id: -((m.rule_idx as i64) + 10000),
                label: m.label.clone(),
                description: format!("{}｜{}", m.category, m.description),
                origin: "correlative".to_string(),
                expression_type: "correlative".to_string(),
                priority: 40,
                boundary_effect: "annotate_only".to_string(),
                confidence: 0.9,
                position: position.to_string(),
                token_range: (m.start_token, m.end_token),
                char_range: m.char_range,
                matched_ranges: m.matched_ranges.clone(),
                surface: m.surface.clone(),
            });
        }
    }
    matches.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::lookup::DictionaryEngine;
    use crate::pipeline::Pipeline;

    fn get_test_ipadic_path() -> Option<String> {
        if let Ok(env_path) = std::env::var("KOTOCLIP_TEST_IPADIC") {
            if std::path::Path::new(&env_path).exists() {
                return Some(env_path);
            }
        }
        let candidates = vec![
            "../../ipadic/system.dic",
            "../ipadic/system.dic",
            "ipadic/system.dic",
        ];
        for c in candidates {
            if std::path::Path::new(c).exists() {
                return Some(c.to_string());
            }
        }
        None
    }

    fn get_test_dict_dir() -> Option<String> {
        let candidates = vec!["../../data/dicts", "../data/dicts", "data/dicts"];
        for c in candidates {
            if std::path::Path::new(c).exists() {
                return Some(c.to_string());
            }
        }
        None
    }

    #[test]
    fn test_dictionary_expressions_are_pending_without_structured_evidence() {
        let ipadic_path = match get_test_ipadic_path() {
            Some(p) => p,
            None => {
                println!("跳过测试：未找到 IPADIC system.dic");
                return;
            }
        };
        let dict_dir = match get_test_dict_dir() {
            Some(d) => d,
            None => {
                println!("跳过测试：未找到 data/dicts 目录");
                return;
            }
        };

        let pipeline = Pipeline::new(&ipadic_path).expect("初始化 pipeline 失败");
        let dictionary = DictionaryEngine::new(&dict_dir).expect("初始化 dictionary 失败");

        let cases = vec![
            ("足を動かして、足を取られる。", "足を取られる"),
            ("彼はそっと口を開いた。", "口を開く"),
            ("ついに手に入れた。", "手に入れる"),
            ("胸を張って歩く。", "胸を張る"),
            ("身を潜めている。", "身を潜める"),
        ];

        for (text, expected_label) in cases {
            let tokens = pipeline.process(text, &[]);
            let candidates = dictionary_expression_candidates(&tokens, &dictionary);
            let found = candidates.iter().any(|candidate| {
                candidate.label == expected_label
                    && candidate.status == ExpressionCandidateStatus::Pending
            });
            assert!(
                found,
                "无法在文本 '{}' 中生成预期待定词典候选 '{}'",
                text, expected_label
            );
            assert!(tokens.iter().all(|token| token.expressions.is_empty()));
        }
    }

    #[test]
    fn builtin_candidates_preserve_captures_and_following_clause_requirement() {
        let Some(ipadic_path) = get_test_ipadic_path() else {
            return;
        };
        let pipeline = Pipeline::new(&ipadic_path).unwrap();

        let tokens = pipeline.process("彼は口を開くたびに嘘をつく。", &[]);
        let candidate = builtin_expression_candidates(&tokens)
            .into_iter()
            .find(|candidate| candidate.rule_id == "idiom_kuchi_wo_hiraku")
            .expect("口を開く应生成结构化候选");
        assert_eq!(candidate.status, ExpressionCandidateStatus::Accepted);
        assert_eq!(
            candidate
                .captures
                .iter()
                .map(|capture| (capture.name.as_str(), capture.surface.as_str()))
                .collect::<Vec<_>>(),
            vec![("object", "口"), ("predicate", "開く")]
        );

        let terminal = pipeline.process("何もすることなく。", &[]);
        assert!(builtin_expression_candidates(&terminal)
            .iter()
            .all(|candidate| candidate.rule_id != "negative_koto_naku"));
        assert!(rejected_builtin_candidates(&terminal)
            .iter()
            .any(|candidate| candidate.rejection_reason.as_deref()
                == Some("missing_following_clause")));
    }

    #[test]
    fn test_correlative_expressions() {
        let ipadic_path = match get_test_ipadic_path() {
            Some(p) => p,
            None => {
                println!("跳过测试：未找到 IPADIC system.dic");
                return;
            }
        };

        let pipeline = Pipeline::new(&ipadic_path).expect("初始化 pipeline 失败");

        let cases = vec![
            (
                "まるで、自ら首を差し出しているかのようじゃないか。",
                "まるで〜ようだ",
            ),
            ("一体どうやってあの体を支えているのか。", "一体〜か"),
            (
                "どんなに固定や拘束を解こうとしても、扉が開くことはなかった。",
                "どんなに〜ても",
            ),
            (
                "たとえ俺が斬らなくったって、あいつはいずれ退治される運命だよ。",
                "たとえ〜ても",
            ),
            ("絶対に许さない。", "絶対に〜ない"), // 注意：絶対に……ない
            ("せめてその怒りに触れぬよう", "せめて〜よう"),
        ];

        for (text, expected_label) in cases {
            let mut tokens = pipeline.process(text, &[]);
            println!("=== Text: '{}' morphemes ===", text);
            for (ti, token) in tokens.iter().enumerate() {
                println!("  Token [{}] surface='{}'", ti, token.bunsetsu.surface);
                for m in &token.bunsetsu.morphemes {
                    println!(
                        "    Morpheme: surface='{}', base_form='{}', pos='{:?}'",
                        m.surface, m.base_form, m.pos
                    );
                }
            }
            let count = apply_correlative_expressions(&mut tokens);
            println!(
                "Matched: {}, Expressions: {:?}",
                count,
                tokens
                    .iter()
                    .flat_map(|t| t.expressions.clone())
                    .collect::<Vec<_>>()
            );

            let found = tokens.iter().any(|t| {
                t.expressions
                    .iter()
                    .any(|exp| exp.label == expected_label && exp.origin == "correlative")
            });
            assert!(
                found,
                "无法在文本 '{}' 中匹配到预期的非连续呼应表达 '{}'",
                text, expected_label
            );
        }
    }

    #[derive(Deserialize)]
    struct ExpectedObserved {
        bunsetsu: Vec<String>,
        expressions: Vec<String>,
    }

    #[derive(Deserialize)]
    struct TestCase {
        id: String,
        text: String,
        expected_observed: ExpectedObserved,
        #[serde(default)]
        expected_stage_b: Option<ExpectedStageB>,
        #[serde(default)]
        expected_stage_c: Option<ExpectedStageC>,
        #[serde(default)]
        expected_stage_d: Option<ExpectedStageD>,
    }

    #[derive(Deserialize)]
    struct ExpectedStageB {
        #[serde(default)]
        bunsetsu: Option<Vec<String>>,
        word_formations: Vec<String>,
    }

    #[derive(Deserialize)]
    struct ExpectedStageC {
        bunsetsu: Vec<String>,
        #[serde(default)]
        functions: Vec<String>,
    }

    #[derive(Deserialize)]
    struct ExpectedStageD {
        #[serde(default)]
        accepted: Vec<String>,
        #[serde(default)]
        pending: Vec<String>,
        #[serde(default)]
        rejected: Vec<String>,
        #[serde(default)]
        matched_surfaces: Vec<String>,
    }

    #[test]
    fn test_representative_cases() {
        let ipadic_path = match get_test_ipadic_path() {
            Some(p) => p,
            None => {
                println!("跳过测试：未找到 IPADIC system.dic");
                return;
            }
        };
        let dict_dir = match get_test_dict_dir() {
            Some(d) => d,
            None => {
                println!("跳过测试：未找到 data/dicts 目录");
                return;
            }
        };

        let pipeline = Pipeline::new(&ipadic_path).expect("初始化 pipeline 失败");
        let dictionary = DictionaryEngine::new(&dict_dir).expect("初始化 dictionary 失败");

        let json_str = include_str!("../../tests/fixtures/representative_cases.json");
        let cases: Vec<TestCase> =
            serde_json::from_str(json_str).expect("解析 test_cases JSON 失败");

        let mut printed_placeholders = false;

        for case in &cases {
            let mut tokens = pipeline.process_with_dictionary(&case.text, &[], &dictionary);
            apply_builtin_expressions(&mut tokens);
            let dictionary_candidates = dictionary_expression_candidates(&tokens, &dictionary);
            apply_correlative_expressions(&mut tokens);
            resolve_expression_conflicts(&mut tokens);
            let builtin_rejected = rejected_builtin_candidates(&tokens);

            let actual_bunsetsu: Vec<String> =
                tokens.iter().map(|t| t.bunsetsu.surface.clone()).collect();
            let mut actual_word_formations: Vec<String> = tokens
                .iter()
                .flat_map(|t| {
                    t.bunsetsu
                        .word_formations
                        .iter()
                        .map(|item| item.rule_id.clone())
                })
                .collect();
            actual_word_formations.sort();
            let mut actual_expressions: Vec<String> = tokens
                .iter()
                .flat_map(|t| t.expressions.iter().map(|exp| exp.label.clone()))
                .collect();
            actual_expressions.sort();
            actual_expressions.dedup();

            if let Some(expected_stage_c) = &case.expected_stage_c {
                assert_eq!(
                    actual_bunsetsu, expected_stage_c.bunsetsu,
                    "用例 {} 的阶段 C 文节切分不一致",
                    case.id
                );
                if !expected_stage_c.functions.is_empty() {
                    let actual_functions: Vec<String> = tokens
                        .iter()
                        .filter(|token| token.display_class == "content")
                        .map(|token| {
                            token
                                .bunsetsu
                                .function
                                .as_ref()
                                .map_or("unknown", |annotation| match annotation.function {
                                    crate::models::BunsetsuFunction::Predicate => "predicate",
                                    crate::models::BunsetsuFunction::CasePhrase => "case_phrase",
                                    crate::models::BunsetsuFunction::Adnominal => "adnominal",
                                    crate::models::BunsetsuFunction::Adverbial => "adverbial",
                                    crate::models::BunsetsuFunction::Conjunctive => "conjunctive",
                                    crate::models::BunsetsuFunction::Nominal => "nominal",
                                    crate::models::BunsetsuFunction::Standalone => "standalone",
                                    crate::models::BunsetsuFunction::Unknown => "unknown",
                                })
                                .to_string()
                        })
                        .collect();
                    assert_eq!(
                        actual_functions, expected_stage_c.functions,
                        "用例 {} 的阶段 C 功能标签不一致",
                        case.id
                    );
                }
            }

            if let Some(expected_stage_d) = &case.expected_stage_d {
                assert_eq!(
                    actual_expressions, expected_stage_d.accepted,
                    "用例 {} 的阶段 D accepted 不一致",
                    case.id
                );
                let mut pending: Vec<String> = dictionary_candidates
                    .iter()
                    .filter(|candidate| candidate.status == ExpressionCandidateStatus::Pending)
                    .map(|candidate| candidate.label.clone())
                    .collect();
                pending.sort();
                pending.dedup();
                assert_eq!(
                    pending, expected_stage_d.pending,
                    "用例 {} 的阶段 D pending 不一致",
                    case.id
                );
                let mut rejected: Vec<String> = builtin_rejected
                    .iter()
                    .map(|candidate| candidate.rule_id.clone())
                    .collect();
                rejected.sort();
                rejected.dedup();
                assert_eq!(
                    rejected, expected_stage_d.rejected,
                    "用例 {} 的阶段 D rejected 不一致",
                    case.id
                );
                if !expected_stage_d.matched_surfaces.is_empty() {
                    let chars: Vec<char> = case.text.chars().collect();
                    let mut surfaces: Vec<String> = tokens
                        .iter()
                        .flat_map(|token| token.expressions.iter())
                        .filter(|expression| {
                            matches!(expression.position.as_str(), "start" | "single")
                        })
                        .map(|expression| {
                            expression
                                .matched_ranges
                                .iter()
                                .map(|range| chars[range.0..range.1].iter().collect::<String>())
                                .collect::<String>()
                        })
                        .collect();
                    surfaces.sort();
                    surfaces.dedup();
                    assert_eq!(
                        surfaces, expected_stage_d.matched_surfaces,
                        "用例 {} 的阶段 D 精确表层不一致",
                        case.id
                    );
                }
                continue;
            }
            if case.expected_stage_c.is_some() {
                continue;
            }

            if let Some(expected_stage_b) = &case.expected_stage_b {
                if let Some(expected_bunsetsu) = &expected_stage_b.bunsetsu {
                    assert_eq!(
                        actual_bunsetsu, *expected_bunsetsu,
                        "用例 {} 的阶段 B 文节切分不一致",
                        case.id
                    );
                }
                assert_eq!(
                    actual_word_formations, expected_stage_b.word_formations,
                    "用例 {} 的阶段 B 构词规则不一致",
                    case.id
                );
            } else if case.expected_observed.bunsetsu.is_empty() {
                if !printed_placeholders {
                    println!("\n=== 代表性案例当前运行 Observed 输出 ===");
                    printed_placeholders = true;
                }
                println!(
                    "ID: {} ->\n  \"expected_observed\": {{\n    \"bunsetsu\": {:?},\n    \"expressions\": {:?}\n  }},",
                    case.id, actual_bunsetsu, actual_expressions
                );
            } else {
                assert_eq!(
                    actual_bunsetsu, case.expected_observed.bunsetsu,
                    "用例 {} 的文节切分不一致",
                    case.id
                );
                assert_eq!(
                    actual_expressions, case.expected_observed.expressions,
                    "用例 {} 的表达式匹配不一致",
                    case.id
                );
            }
        }

        if printed_placeholders {
            panic!("发现 Observed 占位为空的测试例，请将上述打印的 actual 输出反填至 representative_cases.json");
        }
    }
}
