use crate::models::{AnnotatedToken, ExpressionAnnotation};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
struct ExpressionCatalog {
    patterns: Vec<BuiltinExpressionPattern>,
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
