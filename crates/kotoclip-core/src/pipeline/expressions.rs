use crate::models::{AnnotatedToken, ExpressionAnnotation};
use serde::Deserialize;
use std::collections::HashSet;

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

/// 应用最高优先级的词汇单位边界，同时保留合并范围内的底层语素。
pub fn apply_expression_boundaries(tokens: Vec<AnnotatedToken>) -> Vec<AnnotatedToken> {
    let mut lexical_ranges: Vec<_> = tokens
        .iter()
        .flat_map(|token| &token.expressions)
        .filter(|item| item.boundary_effect == "merge_lexical_unit")
        .map(|item| (item.token_range, item.priority, item.confidence, item.match_id.clone()))
        .collect();
    lexical_ranges.sort_by(|left, right| {
        right.1.cmp(&left.1)
            .then_with(|| right.2.total_cmp(&left.2))
            .then_with(|| right.0.1.saturating_sub(right.0.0).cmp(&left.0.1.saturating_sub(left.0.0)))
    });
    lexical_ranges.dedup_by(|left, right| left.3 == right.3);

    let mut claimed = vec![false; tokens.len()];
    let mut accepted = Vec::new();
    for (range, _, _, _) in lexical_ranges {
        if range.0 >= range.1
            || range.1 > tokens.len()
            || claimed[range.0..range.1].iter().any(|value| *value)
            || tokens[range.0..range.1]
                .iter()
                .any(|token| token.display_class != "content")
        {
            continue;
        }
        claimed[range.0..range.1].fill(true);
        accepted.push(range);
    }
    accepted.sort_by_key(|range| range.0);

    let mut old_to_new = vec![0usize; tokens.len()];
    let mut result = Vec::new();
    let mut index = 0;
    let mut range_index = 0;
    while index < tokens.len() {
        if accepted.get(range_index).is_some_and(|range| range.0 == index) {
            let range = accepted[range_index];
            let mut merged = tokens[index].clone();
            merged.bunsetsu.morphemes = tokens[range.0..range.1]
                .iter()
                .flat_map(|token| token.bunsetsu.morphemes.clone())
                .collect();
            merged.bunsetsu.surface = tokens[range.0..range.1]
                .iter()
                .map(|token| token.bunsetsu.surface.as_str())
                .collect();
            merged.bunsetsu.char_range = (
                tokens[range.0].bunsetsu.char_range.0,
                tokens[range.1 - 1].bunsetsu.char_range.1,
            );
            merged.bunsetsu.grammar_tags = tokens[range.0..range.1]
                .iter()
                .flat_map(|token| token.bunsetsu.grammar_tags.clone())
                .collect();
            let mut word_formations = Vec::new();
            let mut morpheme_offset = 0;
            for token in &tokens[range.0..range.1] {
                for formation in &token.bunsetsu.word_formations {
                    let mut formation = formation.clone();
                    formation.morpheme_range.0 += morpheme_offset;
                    formation.morpheme_range.1 += morpheme_offset;
                    formation.head_morpheme += morpheme_offset;
                    for capture in &mut formation.captures {
                        capture.morpheme_range.0 += morpheme_offset;
                        capture.morpheme_range.1 += morpheme_offset;
                    }
                    word_formations.push(formation);
                }
                morpheme_offset += token.bunsetsu.morphemes.len();
            }
            merged.bunsetsu.word_formations = word_formations;
            if let Some(expression) = tokens[range.0..range.1]
                .iter()
                .flat_map(|token| &token.expressions)
                .find(|item| item.boundary_effect == "merge_lexical_unit" && item.token_range == range)
            {
                merged.bunsetsu.head_word.surface = expression.surface.clone();
                merged.bunsetsu.head_word.base_form = expression.label.clone();
                merged.bunsetsu.head_word.reading = merged
                    .bunsetsu
                    .morphemes
                    .iter()
                    .map(|morpheme| morpheme.reading.as_str())
                    .collect();
            }
            merged.novelty_score = tokens[range.0..range.1]
                .iter()
                .map(|token| token.novelty_score)
                .fold(0.0, f32::max);
            merged.is_selected = tokens[range.0..range.1].iter().any(|token| token.is_selected);
            merged.is_known = tokens[range.0..range.1].iter().all(|token| token.is_known);
            merged.expressions = tokens[range.0..range.1]
                .iter()
                .flat_map(|token| token.expressions.clone())
                .collect();
            let new_index = result.len();
            old_to_new[range.0..range.1].fill(new_index);
            result.push(merged);
            index = range.1;
            range_index += 1;
        } else {
            old_to_new[index] = result.len();
            result.push(tokens[index].clone());
            index += 1;
        }
    }

    for (new_index, token) in result.iter_mut().enumerate() {
        let mut seen = HashSet::new();
        token.expressions.retain_mut(|item| {
            let old_range = item.token_range;
            if old_range.0 >= old_range.1 || old_range.1 > old_to_new.len() {
                return false;
            }
            let start = old_to_new[old_range.0];
            let end = old_to_new[old_range.1 - 1] + 1;
            item.token_range = (start, end);
            item.position = if end - start == 1 {
                "single"
            } else if new_index == start {
                "start"
            } else if new_index + 1 == end {
                "end"
            } else {
                "middle"
            }
            .to_string();
            seen.insert(item.match_id.clone())
        });
    }
    result
}

#[derive(Debug, Deserialize)]
struct ExpressionCatalog {
    patterns: Vec<BuiltinExpressionPattern>,
    #[serde(default)]
    correlative_patterns: Vec<CorrelativePattern>,
}

#[derive(Debug, Deserialize)]
struct CorrelativePattern {
    id: String,
    label: String,
    description: String,
    category: String,
    head_atoms: Vec<ExpressionAtom>,
    tail_variants: Vec<Vec<ExpressionAtom>>,
    #[serde(default = "default_gap_bunsetsu")]
    gap_bunsetsu: (usize, usize),
}

fn default_gap_bunsetsu() -> (usize, usize) {
    (0, 10)
}

#[derive(Debug, Deserialize)]
struct BuiltinExpressionPattern {
    id: String,
    label: String,
    description: String,
    category: String,
    atoms: Vec<ExpressionAtom>,
}

#[derive(Debug, Deserialize)]
struct ExpressionAtom {
    #[serde(default)]
    lemmas: Vec<String>,
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
    conjugation_forms: Vec<String>,
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
    is_boundary: bool,
}

fn catalog() -> ExpressionCatalog {
    serde_json::from_str(include_str!("../../resources/expression_patterns.json"))
        .expect("内置跨文节表达目录格式无效")
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
                    is_boundary: true,
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
                        is_boundary: false,
                    })
                })
                .collect()
        })
        .collect()
}

fn atom_matches(atom: &ExpressionAtom, morpheme: &FlatMorpheme) -> bool {
    (atom.lemmas.is_empty() || atom.lemmas.iter().any(|lemma| lemma == &morpheme.lemma))
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
        && (atom.conjugation_forms.is_empty()
            || atom
                .conjugation_forms
                .iter()
                .any(|value| value == &morpheme.conjugation_form))
}

/// 在跨文节的完整语素流上运行确定性状态机。匹配范围从第一个锚点语素开始，
/// 因此 `兄に｜負けるところが｜多い` 的参数“兄”不会被误标为表达本体。
pub fn apply_builtin_expressions(tokens: &mut [AnnotatedToken]) -> usize {
    let morphemes = flatten(tokens);
    let mut count = 0;
    let mut seen = HashSet::new();

    for (pattern_index, pattern) in catalog().patterns.into_iter().enumerate() {
        if pattern.atoms.is_empty() || pattern.atoms.len() > morphemes.len() {
            continue;
        }
        for start in 0..=morphemes.len() - pattern.atoms.len() {
            let window = &morphemes[start..start + pattern.atoms.len()];
            if !pattern
                .atoms
                .iter()
                .zip(window)
                .all(|(atom, morpheme)| atom_matches(atom, morpheme))
            {
                continue;
            }
            let start_token = window.first().unwrap().token_index;
            let end_token = window.last().unwrap().token_index + 1;
            if end_token - start_token < 2 {
                continue;
            }
            let match_id = format!("builtin:{}:{}:{}", pattern.id, start_token, end_token);
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
            let width = end_token - start_token;
            for (offset, token) in tokens[start_token..end_token].iter_mut().enumerate() {
                let position = if offset == 0 {
                    "start"
                } else if offset + 1 == width {
                    "end"
                } else {
                    "middle"
                };
                token.expressions.push(ExpressionAnnotation {
                    match_id: match_id.clone(),
                    rule_id: -((pattern_index as i64) + 1),
                    label: pattern.label.clone(),
                    description: format!("{}｜{}", pattern.category, pattern.description),
                    origin: "builtin".to_string(),
                    expression_type: "grammar_construction".to_string(),
                    priority: 60,
                    boundary_effect: "annotate_only".to_string(),
                    confidence: 0.95,
                    position: position.to_string(),
                    token_range: (start_token, end_token),
                    char_range,
                    surface: surface.clone(),
                });
            }
            count += 1;
        }
    }
    count
}

struct MatchInfo {
    query: String,
    s: usize,
    e: usize,
    char_range: (usize, usize),
    surface: String,
    expression_type: String,
    boundary_effect: String,
    priority: i32,
    confidence: f32,
}

/// 词典存在性只提供候选证据；组合方式决定候选能否成为正式结果。
/// 以助词起始的「に＋つく」「と＋する」等普通句法不进入自动表达层。
fn classify_dictionary_composition(
    morphemes: &[&crate::models::Morpheme],
) -> Option<(&'static str, &'static str, i32, f32)> {
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
            return Some(("idiom", "annotate_only", 70, 0.82));
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
    lexical_shape.then_some(("lexical_unit", "merge_lexical_unit", 90, 0.9))
}

/// 基于本地词典自动匹配的跨文节固定表达扫描
pub fn apply_dictionary_expressions(
    tokens: &mut [AnnotatedToken],
    dictionary: &crate::dictionary::lookup::DictionaryEngine,
) -> usize {
    let mut count = 0;
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
                    let Some((expression_type, boundary_effect, priority, confidence)) =
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
                        boundary_effect: boundary_effect.to_string(),
                        priority,
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

    // 第三阶段：应用匹配到的跨文节表达
    for info in matches_found {
        let match_id = format!("dict:{}:{}:{}", info.query, info.s, info.e + 1);
        let width = info.e + 1 - info.s;

        // 排重：如果本 token 范围已被其他同等范围的表达标注过，则跳过
        let has_duplicate = tokens[info.s..=info.e].iter().any(|token| {
            token
                .expressions
                .iter()
                .any(|exp| exp.char_range == info.char_range)
        });
        if has_duplicate {
            continue;
        }

        for (offset, token) in tokens[info.s..=info.e].iter_mut().enumerate() {
            let position = if offset == 0 {
                "start"
            } else if offset + 1 == width {
                "end"
            } else {
                "middle"
            };
            token.expressions.push(ExpressionAnnotation {
                match_id: match_id.clone(),
                rule_id: -9999,
                label: info.query.clone(),
                description: "词典惯用语｜在本地词典中匹配到的固定跨文节表达。".to_string(),
                origin: "dictionary".to_string(),
                expression_type: info.expression_type.clone(),
                priority: info.priority,
                boundary_effect: info.boundary_effect.clone(),
                confidence: info.confidence,
                position: position.to_string(),
                token_range: (info.s, info.e + 1),
                char_range: info.char_range,
                surface: info.surface.clone(),
            });
        }
        count += 1;
    }

    count
}

fn is_sentence_boundary(morpheme: &FlatMorpheme) -> bool {
    morpheme.is_boundary
        || morpheme.lemma == "。"
        || morpheme.lemma == "！"
        || morpheme.lemma == "？"
        || morpheme.surface.contains('。')
        || morpheme.surface.contains('！')
        || morpheme.surface.contains('？')
        || morpheme.surface.contains('\n')
        || morpheme.surface.contains('?')
        || morpheme.surface.contains('!')
}

struct CorrelativeMatchInfo {
    id: String,
    label: String,
    category: String,
    description: String,
    rule_idx: usize,
    start_token: usize,
    end_token: usize,
    char_range: (usize, usize),
    surface: String,
}

/// 非连续呼应表达匹配层
pub fn apply_correlative_expressions(tokens: &mut [AnnotatedToken]) -> usize {
    let morphemes = flatten(tokens);
    let mut matches: Vec<CorrelativeMatchInfo> = Vec::new();
    let catalog = catalog();

    for (pattern_index, pattern) in catalog.correlative_patterns.iter().enumerate() {
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
                        rule_idx: pattern_index,
                        start_token: head_start_token,
                        end_token: tail_end_token,
                        char_range,
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
            final_matches.push(m_i);
        }
    }

    let mut count = 0;
    for m in final_matches {
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
                surface: m.surface.clone(),
            });
        }
        count += 1;
    }

    count
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
    fn test_dictionary_expressions() {
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
            let mut tokens = pipeline.process(text, &[]);
            let count = apply_dictionary_expressions(&mut tokens, &dictionary);
            println!(
                "Text: '{}', Matched: {}, Expressions: {:?}",
                text,
                count,
                tokens
                    .iter()
                    .flat_map(|t| t.expressions.clone())
                    .collect::<Vec<_>>()
            );

            let found = tokens.iter().any(|t| {
                t.expressions
                    .iter()
                    .any(|exp| exp.label == expected_label && exp.rule_id == -9999)
            });
            assert!(
                found,
                "无法在文本 '{}' 中匹配到预期的跨文节表达 '{}'",
                text, expected_label
            );
        }
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
        let cases: Vec<TestCase> = serde_json::from_str(json_str).expect("解析 test_cases JSON 失败");

        let mut printed_placeholders = false;

        for case in &cases {
            let mut tokens = pipeline.process(&case.text, &[]);
            for token in &mut tokens {
                if token.display_class == "content" {
                    crate::pipeline::bunsetsu::resolve_lexical_boundaries(
                        std::slice::from_mut(&mut token.bunsetsu),
                        |word| dictionary.contains_exact(word),
                    );
                }
            }
            apply_builtin_expressions(&mut tokens);
            apply_dictionary_expressions(&mut tokens, &dictionary);
            apply_correlative_expressions(&mut tokens);
            resolve_expression_conflicts(&mut tokens);
            let tokens = apply_expression_boundaries(tokens);

            let actual_bunsetsu: Vec<String> = tokens.iter().map(|t| t.bunsetsu.surface.clone()).collect();
            let mut actual_word_formations: Vec<String> = tokens
                .iter()
                .flat_map(|t| t.bunsetsu.word_formations.iter().map(|item| item.rule_id.clone()))
                .collect();
            actual_word_formations.sort();
            let mut actual_expressions: Vec<String> = tokens
                .iter()
                .flat_map(|t| t.expressions.iter().map(|exp| exp.label.clone()))
                .collect();
            actual_expressions.sort();
            actual_expressions.dedup();

            if let Some(expected_stage_c) = &case.expected_stage_c {
                assert_eq!(actual_bunsetsu, expected_stage_c.bunsetsu, "用例 {} 的阶段 C 文节切分不一致", case.id);
                if !expected_stage_c.functions.is_empty() {
                    let actual_functions: Vec<String> = tokens.iter().filter(|token| token.display_class == "content").map(|token| {
                        token.bunsetsu.function.as_ref().map_or("unknown", |annotation| match annotation.function {
                            crate::models::BunsetsuFunction::Predicate => "predicate",
                            crate::models::BunsetsuFunction::CasePhrase => "case_phrase",
                            crate::models::BunsetsuFunction::Adnominal => "adnominal",
                            crate::models::BunsetsuFunction::Adverbial => "adverbial",
                            crate::models::BunsetsuFunction::Conjunctive => "conjunctive",
                            crate::models::BunsetsuFunction::Nominal => "nominal",
                            crate::models::BunsetsuFunction::Standalone => "standalone",
                            crate::models::BunsetsuFunction::Unknown => "unknown",
                        }).to_string()
                    }).collect();
                    assert_eq!(actual_functions, expected_stage_c.functions, "用例 {} 的阶段 C 功能标签不一致", case.id);
                }
                continue;
            }

            if let Some(expected_stage_b) = &case.expected_stage_b {
                if let Some(expected_bunsetsu) = &expected_stage_b.bunsetsu {
                    assert_eq!(actual_bunsetsu, *expected_bunsetsu, "用例 {} 的阶段 B 文节切分不一致", case.id);
                }
                assert_eq!(actual_word_formations, expected_stage_b.word_formations, "用例 {} 的阶段 B 构词规则不一致", case.id);
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
