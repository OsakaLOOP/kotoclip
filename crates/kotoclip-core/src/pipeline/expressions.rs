use crate::models::{AnnotatedToken, ExpressionAnnotation};
use serde::Deserialize;
use std::collections::HashSet;

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
}

#[derive(Debug)]
struct FlatMorpheme {
    token_index: usize,
    surface: String,
    lemma: String,
    pos: String,
    char_range: (usize, usize),
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
            token.bunsetsu.morphemes.iter().filter_map(move |morpheme| {
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
                    char_range: morpheme.char_range,
                })
            })
        })
        .collect()
}

fn atom_matches(atom: &ExpressionAtom, morpheme: &FlatMorpheme) -> bool {
    (atom.lemmas.is_empty() || atom.lemmas.iter().any(|lemma| lemma == &morpheme.lemma))
        && atom.pos.as_ref().map_or(true, |pos| pos == &morpheme.pos)
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
            if !pattern.atoms.iter().zip(window).all(|(atom, morpheme)| atom_matches(atom, morpheme)) {
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
            let surface: String = window.iter().map(|morpheme| morpheme.surface.as_str()).collect();
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
}

/// 基于本地词典自动匹配的跨文节固定表达扫描
pub fn apply_dictionary_expressions(
    tokens: &mut [AnnotatedToken],
    dictionary: &crate::dictionary::lookup::DictionaryEngine,
) -> usize {
    let mut count = 0;
    let mut matched_char_ranges: Vec<(usize, usize)> = Vec::new();
    let mut matches_found: Vec<MatchInfo> = Vec::new();

    // 限制跨越的文节数 N 从 4 递减到 2，优先匹配更长范围
    for n in (2..=4).rev() {
        if n > tokens.len() {
            continue;
        }
        'window_loop: for s in 0..=tokens.len() - n {
            let e = s + n - 1; // 终点文节索引
            let ts = &tokens[s].bunsetsu;
            let te = &tokens[e].bunsetsu;

            // 穷举起点文节中的语素偏移 p 和终点文节中的语素偏移 q
            for p in 0..ts.morphemes.len() {
                for q in 0..te.morphemes.len() {
                    let mut sub_morphemes = Vec::new();

                    // 起点文节的后半部分
                    for m in &ts.morphemes[p..] {
                        sub_morphemes.push(m);
                    }

                    // 中间文节的所有语素
                    for mid in s + 1..e {
                        for m in &tokens[mid].bunsetsu.morphemes {
                            sub_morphemes.push(m);
                        }
                    }

                    // 终点文节的前半部分
                    for m in &te.morphemes[..=q] {
                        sub_morphemes.push(m);
                    }

                    // 1. 过滤包含无意义字符（如标点或空白）的组合
                    if sub_morphemes.iter().any(|m| m.surface.trim().is_empty() || m.pos.major == "記号") {
                        continue;
                    }

                    // 2. 检查最后一个语素的词性是否合规
                    let last_m = sub_morphemes.last().unwrap();
                    let valid_last_pos = matches!(
                        last_m.pos.major.as_str(),
                        "動詞" | "形容詞" | "助動詞" | "名詞"
                    );
                    if !valid_last_pos {
                        continue;
                    }

                    // 3. 计算本组合精确的 char_range，并执行包含性排重
                    let char_range = (
                        sub_morphemes.first().unwrap().char_range.0,
                        sub_morphemes.last().unwrap().char_range.1,
                    );

                    let is_sub_range = matched_char_ranges.iter().any(|&(existing_start, existing_end)| {
                        existing_start <= char_range.0 && char_range.1 <= existing_end
                    });
                    if is_sub_range {
                        continue;
                    }

                    // 4. 启发式拼接查询词 (前 len - 1 个语素取 surface，最后一个语素取 base_form)
                    let mut query = String::new();
                    let len = sub_morphemes.len();
                    for (idx, m) in sub_morphemes.iter().enumerate() {
                        if idx + 1 < len {
                            query.push_str(&m.surface);
                        } else {
                            let base = if m.base_form.is_empty() || m.base_form == "*" {
                                &m.surface
                            } else {
                                &m.base_form
                            };
                            query.push_str(base);
                        }
                    }

                    // 5. 词典精确匹配
                    if dictionary.contains_exact(&query) {
                        // 记录匹配成功的字符区间，供后续更短组合的排重判定
                        matched_char_ranges.push(char_range);

                        matches_found.push(MatchInfo {
                            query: query.clone(),
                            s,
                            e,
                            char_range,
                            surface: sub_morphemes.iter().map(|m| m.surface.as_str()).collect(),
                        });
                        continue 'window_loop;
                    }
                }
            }
        }
    }

    // 第二阶段：应用匹配到的跨文节表达
    for info in matches_found {
        let match_id = format!("dict:{}:{}:{}", info.query, info.s, info.e + 1);
        let width = info.e + 1 - info.s;

        // 排重：如果本 token 范围已被其他同等范围的表达标注过，则跳过
        let has_duplicate = tokens[info.s..=info.e].iter().any(|token| {
            token.expressions.iter().any(|exp| exp.char_range == info.char_range)
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
    morpheme.lemma == "。"
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
            if !pattern.head_atoms.iter().zip(h_window).all(|(atom, m)| atom_matches(atom, m)) {
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
                    let t_window = &morphemes[t_start..t_start + tail_len];
                    if !tail_atoms.iter().zip(t_window).all(|(atom, m)| atom_matches(atom, m)) {
                        continue;
                    }

                    let tail_start_token = t_window.first().unwrap().token_index;
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

                    let head_surface: String = h_window.iter().map(|m| m.surface.as_str()).collect();
                    let tail_surface: String = t_window.iter().map(|m| m.surface.as_str()).collect();
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
                        if cand.end_token > current_best.end_token {
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
            if i == j { continue; }
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
    use crate::pipeline::Pipeline;
    use crate::dictionary::lookup::DictionaryEngine;

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
        let candidates = vec![
            "../../data/dicts",
            "../data/dicts",
            "data/dicts",
        ];
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
            println!("Text: '{}', Matched: {}, Expressions: {:?}", text, count, tokens.iter().flat_map(|t| t.expressions.clone()).collect::<Vec<_>>());
            
            let found = tokens.iter().any(|t| {
                t.expressions.iter().any(|exp| exp.label == expected_label && exp.rule_id == -9999)
            });
            assert!(found, "无法在文本 '{}' 中匹配到预期的跨文节表达 '{}'", text, expected_label);
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
            ("まるで、自ら首を差し出しているかのようじゃないか。", "まるで〜ようだ"),
            ("一体どうやってあの体を支えているのか。", "一体〜か"),
            ("どんなに固定や拘束を解こうとしても、扉が開くことはなかった。", "どんなに〜ても"),
            ("たとえ俺が斬らなくったって、あいつはいずれ退治される運命だよ。", "たとえ〜ても"),
            ("絶対に许さない。", "絶対に〜ない"), // 注意：絶対に……ない
            ("せめてその怒りに触れぬよう", "せめて〜よう"),
        ];

        for (text, expected_label) in cases {
            let mut tokens = pipeline.process(text, &[]);
            println!("=== Text: '{}' morphemes ===", text);
            for (ti, token) in tokens.iter().enumerate() {
                println!("  Token [{}] surface='{}'", ti, token.bunsetsu.surface);
                for m in &token.bunsetsu.morphemes {
                    println!("    Morpheme: surface='{}', base_form='{}', pos='{:?}'", m.surface, m.base_form, m.pos);
                }
            }
            let count = apply_correlative_expressions(&mut tokens);
            println!("Matched: {}, Expressions: {:?}", count, tokens.iter().flat_map(|t| t.expressions.clone()).collect::<Vec<_>>());

            let found = tokens.iter().any(|t| {
                t.expressions.iter().any(|exp| exp.label == expected_label && exp.origin == "correlative")
            });
            assert!(found, "无法在文本 '{}' 中匹配到预期的非连续呼应表达 '{}'", text, expected_label);
        }
    }
}

