//! 编译语法目录与讲解库的只读查询服务。
use serde::{Deserialize, Serialize};

const CATALOG_JSON: &str = include_str!("../resources/grammar/compiled/grammar_catalog.json");
const EXPLANATIONS_JSON: &str = include_str!("../resources/grammar/compiled/grammar_explanations.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarConcept {
    pub concept_id: String,
    pub kind: String,
    pub canonical_label: String,
    #[serde(default)] pub aliases: Vec<String>,
    #[serde(default)] pub semantic_domains: Vec<String>,
    #[serde(default)] pub function_tags: Vec<String>,
    pub jlpt_level: Option<u8>,
    #[serde(default)] pub register: Vec<String>,
    #[serde(default)] pub related_concept_ids: Vec<String>,
    #[serde(default)] pub contrast_concept_ids: Vec<String>,
    pub default_explanation_id: String,
    #[serde(default)] pub source_refs: Vec<String>,
    pub audit_status: String,
    pub concept_version: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarSense {
    pub sense_id: String,
    pub concept_id: String,
    pub label: String,
    pub function_summary: String,
    #[serde(default)] pub semantic_features: serde_json::Value,
    #[serde(default)] pub context_requirements: Vec<String>,
    #[serde(default)] pub exclusion_conditions: Vec<String>,
    #[serde(default)] pub related_sense_ids: Vec<String>,
    #[serde(default)] pub contrast_sense_ids: Vec<String>,
    pub explanation_id: String,
    pub sense_version: u32,
    pub audit_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarExplanationSourceBlock {
    pub kind: String,
    pub label: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarProvenance {
    pub origin: String,
    pub author: String,
    pub date: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarExplanationDocument {
    pub explanation_id: String,
    pub concept_id: String,
    pub sense_id: Option<String>,
    pub language: String,
    pub title: String,
    pub compact_summary: String,
    pub function_summary: String,
    pub connection: String,
    pub formation: String,
    #[serde(default)] pub usage_notes: Vec<String>,
    #[serde(default)] pub semantic_constraints: Vec<String>,
    #[serde(default)] pub pragmatic_notes: Vec<String>,
    #[serde(default)] pub examples: Vec<String>,
    #[serde(default)] pub counter_examples: Vec<String>,
    #[serde(default)] pub source_refs: Vec<String>,
    pub authoring_status: String,
    pub content_version: u32,
    #[serde(default)] pub provenance: Option<GrammarProvenance>,
    pub review_status: String,
    #[serde(default)] pub body_blocks: Vec<GrammarExplanationSourceBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarConceptBundle {
    pub concept: GrammarConcept,
    pub senses: Vec<GrammarSense>,
    pub explanation: GrammarExplanationDocument,
    pub explanations: Vec<GrammarExplanationDocument>,
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    concepts: Vec<GrammarConcept>,
    #[serde(default)] senses: Vec<GrammarSense>,
}

#[derive(Debug, Deserialize)]
struct ExplanationFile { explanations: Vec<GrammarExplanationDocument> }

fn files() -> Result<(CatalogFile, ExplanationFile), String> {
    let catalog = serde_json::from_str(CATALOG_JSON).map_err(|error| format!("语法目录解析失败：{error}"))?;
    let explanations = serde_json::from_str(EXPLANATIONS_JSON).map_err(|error| format!("语法讲解库解析失败：{error}"))?;
    Ok((catalog, explanations))
}

pub fn search(query: Option<&str>, family: Option<&str>, jlpt_level: Option<u8>, audit_status: Option<&str>, source_ref: Option<&str>) -> Result<Vec<GrammarConcept>, String> {
    let (catalog, _) = files()?;
    let query = query.unwrap_or("").trim().to_lowercase();
    let result = catalog.concepts.into_iter().filter(|concept| {
        if !concept.enabled { return false; }
        if let Some(family) = family.filter(|value| !value.trim().is_empty()) {
            let family = family.to_lowercase();
            if concept.kind.to_lowercase() != family && !concept.semantic_domains.iter().any(|item| item.to_lowercase() == family) { return false; }
        }
        if let Some(level) = jlpt_level { if concept.jlpt_level != Some(level) { return false; } }
        if let Some(status) = audit_status.filter(|value| !value.trim().is_empty()) { if concept.audit_status != status { return false; } }
        if let Some(source) = source_ref.filter(|value| !value.trim().is_empty()) { if !concept.source_refs.iter().any(|item| item == source) { return false; } }
        if query.is_empty() { return true; }
        let mut fields = std::iter::once(&concept.concept_id).chain(std::iter::once(&concept.canonical_label)).chain(concept.aliases.iter());
        fields.any(|field| field.to_lowercase().contains(&query))
    }).collect();
    Ok(result)
}

pub fn get(concept_id: &str) -> Result<GrammarConceptBundle, String> {
    let (catalog, explanations) = files()?;
    let concept = catalog.concepts.into_iter().find(|item| item.concept_id == concept_id).ok_or_else(|| format!("语法 concept 不存在：{concept_id}"))?;
    let senses = catalog.senses.into_iter().filter(|item| item.concept_id == concept_id).collect::<Vec<_>>();
    let related = explanations.explanations.into_iter().filter(|item| item.concept_id == concept_id).collect::<Vec<_>>();
    let explanation = related.iter().find(|item| item.explanation_id == concept.default_explanation_id).cloned().or_else(|| related.first().cloned()).ok_or_else(|| format!("语法 concept 缺少讲解：{concept_id}"))?;
    Ok(GrammarConceptBundle { concept, senses, explanation, explanations: related })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_compiled_catalog_and_resolves_explanation() {
        let concepts = search(Some("活用链"), None, None, None, None).unwrap();
        assert!(concepts.iter().any(|item| item.concept_id == "morphology.chain"));
        let bundle = get("morphology.chain").unwrap();
        assert_eq!(bundle.concept.concept_id, "morphology.chain");
        assert!(!bundle.explanation.title.is_empty());
    }
}
