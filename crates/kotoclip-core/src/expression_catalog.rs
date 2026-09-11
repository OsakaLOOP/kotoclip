//! 内置表达目录的只读候选扫描器。
use kotoclip_nlp::{expression::{ExpressionArtifact, ExpressionOccurrence, ExpressionStatus}, model::MorphemeToken};
use serde::Deserialize;

const PATTERNS_JSON: &str = include_str!("../resources/expression_patterns.json");

#[derive(Debug, Deserialize)]
struct PatternFile { patterns: Vec<Pattern> }

#[derive(Debug, Deserialize)]
struct Pattern {
    id: String,
    #[serde(default)] expression_type: Option<String>,
    #[serde(default)] label: String,
    #[serde(default)] description: String,
    #[serde(default)] category: String,
    atoms: Vec<Atom>,
}

#[derive(Debug, Deserialize)]
struct Atom {
    #[serde(default)] lemmas: Vec<String>,
    #[serde(default)] surfaces: Vec<String>,
    #[serde(default)] pos: Option<String>,
}

fn load() -> Result<PatternFile, String> {
    serde_json::from_str(PATTERNS_JSON).map_err(|error| format!("表达目录解析失败：{error}"))
}

fn atom_matches(atom: &Atom, token: &MorphemeToken) -> bool {
    let lemma_match = atom.lemmas.is_empty() || atom.lemmas.iter().any(|value| value == token.lemma.as_deref().unwrap_or("") || value == &token.surface);
    let surface_match = atom.surfaces.is_empty() || atom.surfaces.iter().any(|value| value == &token.surface);
    let pos_match = atom.pos.as_deref().map(|value| token.pos.iter().flatten().any(|item| item == value)).unwrap_or(true);
    lemma_match && surface_match && pos_match
}

/// 在连续 UniDic token 上生成表达候选；不跨越 gap，也不修改原始 token。
pub fn collect(text: &str, morphemes: &[MorphemeToken]) -> Result<ExpressionArtifact, String> {
    let patterns = load()?.patterns;
    let mut occurrences = Vec::new();
    for pattern in patterns {
        if pattern.atoms.is_empty() || pattern.atoms.len() > morphemes.len() { continue; }
        for start in 0..=morphemes.len() - pattern.atoms.len() {
            let end = start + pattern.atoms.len();
            if !pattern.atoms.iter().zip(&morphemes[start..end]).all(|(atom, token)| atom_matches(atom, token)) { continue; }
            let char_range = [morphemes[start].char_range[0], morphemes[end - 1].char_range[1]];
            if char_range[1] > text.chars().count() { return Err(format!("表达 {} 范围超出文本", pattern.id)); }
            let expression_type = pattern.expression_type.clone().unwrap_or_else(|| "grammar_construction".into());
            let mut labels = Vec::new();
            if !pattern.label.is_empty() { labels.push(pattern.label.clone()); }
            if !pattern.category.is_empty() { labels.push(format!("category:{}", pattern.category)); }
            occurrences.push(ExpressionOccurrence {
                id: format!("expression:{}:{}", pattern.id, start), rule_id: Some(pattern.id.clone()),
                char_range, morpheme_indices: (start..end).collect(), status: ExpressionStatus::Candidate,
                provider: "builtin-expression-catalog".into(), source_id: Some(pattern.id.clone()), expression_type,
                labels, evidence: vec!["catalog:expression_patterns.json".into(), format!("description:{}", pattern.description)],
            });
        }
    }
    Ok(ExpressionArtifact { schema: kotoclip_nlp::expression::SCHEMA.into(), occurrences })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_loads_and_exposes_candidates() {
        let morphemes = [
            MorphemeToken { id: "m0".into(), source_index: 0, surface: "耳".into(), char_range: [0, 1], pos: [Some("名詞".into()), None, None, None], lemma: Some("耳".into()), reading: None, query_forms: Vec::new() },
            MorphemeToken { id: "m1".into(), source_index: 1, surface: "を".into(), char_range: [1, 2], pos: [Some("助詞".into()), None, None, None], lemma: Some("を".into()), reading: None, query_forms: Vec::new() },
            MorphemeToken { id: "m2".into(), source_index: 2, surface: "傾ける".into(), char_range: [2, 5], pos: [Some("動詞".into()), None, None, None], lemma: Some("傾ける".into()), reading: None, query_forms: Vec::new() },
        ];
        let artifact = collect("耳を傾ける", &morphemes).unwrap();
        assert!(artifact.occurrences.iter().any(|item| item.rule_id.as_deref() == Some("idiom_mimi_wo_katamukeru")));
    }
}
