use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarConcept {
    pub concept_id: String,
    pub kind: String,
    pub canonical_label: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub semantic_domains: Vec<String>,
    #[serde(default)]
    pub function_tags: Vec<String>,
    pub jlpt_level: Option<u8>,
    #[serde(default)]
    pub register: Vec<String>,
    #[serde(default)]
    pub related_concept_ids: Vec<String>,
    #[serde(default)]
    pub contrast_concept_ids: Vec<String>,
    pub default_explanation_id: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    pub audit_status: String,
    pub concept_version: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarSense {
    pub sense_id: String,
    pub concept_id: String,
    pub label: String,
    pub function_summary: String,
    #[serde(default)]
    pub semantic_features: HashMap<String, String>,
    #[serde(default)]
    pub context_requirements: Vec<String>,
    #[serde(default)]
    pub exclusion_conditions: Vec<String>,
    #[serde(default)]
    pub related_sense_ids: Vec<String>,
    #[serde(default)]
    pub contrast_sense_ids: Vec<String>,
    pub explanation_id: String,
    pub sense_version: u32,
    pub audit_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarRealization {
    pub realization_id: String,
    pub concept_id: String,
    #[serde(default)]
    pub possible_sense_ids: Vec<String>,
    pub rule_id: String,
    #[serde(default)]
    pub connection_signature: String,
    #[serde(default)]
    pub morphology_requirements: Vec<String>,
    #[serde(default)]
    pub functional_requirements: Vec<String>,
    #[serde(default)]
    pub context_requirements: Vec<String>,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub counter_examples: Vec<String>,
    pub realization_version: u32,
    pub audit_status: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GrammarRuleAtom {
    #[serde(default)]
    pub surfaces: Vec<String>,
    #[serde(default)]
    pub base_forms: Vec<String>,
    #[serde(default)]
    pub pos_major: Vec<String>,
    #[serde(default)]
    pub pos_sub1: Vec<String>,
    #[serde(default)]
    pub conjugation_types: Vec<String>,
    #[serde(default)]
    pub conjugation_forms: Vec<String>,
    #[serde(default)]
    pub morphology_features: Vec<String>,
    #[serde(default)]
    pub provider_components: Vec<GrammarProviderComponent>,
    #[serde(default)]
    pub capture: Option<String>,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarProviderComponent {
    pub role: String,
    pub surface: String,
    pub base_form: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarRule {
    pub rule_id: String,
    pub realization_id: String,
    pub concept_id: String,
    pub kind: String,
    pub priority: i32,
    pub enabled: bool,
    pub audit_status: String,
    pub atoms: Vec<GrammarRuleAtom>,
    #[serde(default)]
    pub display_from: usize,
    pub display_to: Option<usize>,
    #[serde(default)]
    pub captures: Vec<String>,
    #[serde(default)]
    pub refines_rule_ids: Vec<String>,
    pub conflict_group: Option<String>,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub counter_examples: Vec<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
    pub rule_version: u32,
    #[serde(default)]
    pub show_badge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarRedirect {
    pub from_concept_id: String,
    pub to_concept_id: String,
    pub reason: String,
    pub redirect_version: u32,
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
    #[serde(default)]
    pub formation: String,
    #[serde(default)]
    pub usage_notes: Vec<String>,
    #[serde(default)]
    pub semantic_constraints: Vec<String>,
    #[serde(default)]
    pub pragmatic_notes: Vec<String>,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub counter_examples: Vec<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
    pub authoring_status: String,
    pub content_version: u32,
    #[serde(default)]
    pub body_blocks: Vec<GrammarSourceBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarSourceBlock {
    pub kind: String,
    pub label: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompiledCatalog {
    schema_version: u32,
    pub catalog_version: String,
    concepts: Vec<GrammarConcept>,
    senses: Vec<GrammarSense>,
    realizations: Vec<GrammarRealization>,
    rules: Vec<GrammarRule>,
    #[serde(default)]
    redirects: Vec<GrammarRedirect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompiledExplanations {
    schema_version: u32,
    content_version: String,
    explanations: Vec<GrammarExplanationDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarSearchEntry {
    pub concept_id: String,
    pub label: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub semantic_domains: Vec<String>,
    #[serde(default)]
    pub function_tags: Vec<String>,
    pub jlpt_level: Option<u8>,
    #[serde(default)]
    pub surface_hints: Vec<String>,
    #[serde(default)]
    pub register: Vec<String>,
    #[serde(default)]
    pub related_concept_ids: Vec<String>,
    #[serde(default)]
    pub contrast_concept_ids: Vec<String>,
    #[serde(default)]
    pub sense_hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompiledSearchIndex {
    entries: Vec<GrammarSearchEntry>,
}

#[derive(Debug, Clone)]
pub struct GrammarCatalog {
    pub catalog_version: String,
    pub concepts: Vec<GrammarConcept>,
    pub senses: Vec<GrammarSense>,
    pub realizations: Vec<GrammarRealization>,
    pub rules: Vec<GrammarRule>,
    pub explanations: Vec<GrammarExplanationDocument>,
    pub redirects: Vec<GrammarRedirect>,
    pub search_entries: Vec<GrammarSearchEntry>,
    concept_index: HashMap<String, usize>,
    explanation_index: HashMap<String, usize>,
    realization_index: HashMap<String, usize>,
    sense_index: HashMap<String, usize>,
    senses_by_concept: HashMap<String, Vec<usize>>,
    redirect_index: HashMap<String, String>,
    search_index: HashMap<String, usize>,
}

fn default_true() -> bool {
    true
}

impl GrammarCatalog {
    pub fn load_embedded() -> Result<Self, serde_json::Error> {
        let catalog: CompiledCatalog = serde_json::from_str(include_str!(
            "../../../resources/grammar/compiled/grammar_catalog.json"
        ))?;
        let explanations: CompiledExplanations = serde_json::from_str(include_str!(
            "../../../resources/grammar/compiled/grammar_explanations.json"
        ))?;
        let search: CompiledSearchIndex = serde_json::from_str(include_str!(
            "../../../resources/grammar/compiled/grammar_search_index.json"
        ))?;
        Ok(Self::from_parts(catalog, explanations, search.entries))
    }

    fn from_parts(
        catalog: CompiledCatalog,
        explanations: CompiledExplanations,
        search_entries: Vec<GrammarSearchEntry>,
    ) -> Self {
        let concept_index = catalog
            .concepts
            .iter()
            .enumerate()
            .map(|(index, item)| (item.concept_id.clone(), index))
            .collect();
        let explanation_index = explanations
            .explanations
            .iter()
            .enumerate()
            .map(|(index, item)| (item.explanation_id.clone(), index))
            .collect();
        let realization_index = catalog
            .realizations
            .iter()
            .enumerate()
            .map(|(index, item)| (item.realization_id.clone(), index))
            .collect();
        let sense_index = catalog
            .senses
            .iter()
            .enumerate()
            .map(|(index, item)| (item.sense_id.clone(), index))
            .collect();
        let mut senses_by_concept: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, sense) in catalog.senses.iter().enumerate() {
            senses_by_concept
                .entry(sense.concept_id.clone())
                .or_default()
                .push(index);
        }
        let redirect_index = catalog
            .redirects
            .iter()
            .map(|item| (item.from_concept_id.clone(), item.to_concept_id.clone()))
            .collect();
        let search_index = search_entries
            .iter()
            .enumerate()
            .map(|(index, item)| (item.concept_id.clone(), index))
            .collect();
        Self {
            catalog_version: catalog.catalog_version,
            concepts: catalog.concepts,
            senses: catalog.senses,
            realizations: catalog.realizations,
            rules: catalog.rules,
            explanations: explanations.explanations,
            redirects: catalog.redirects,
            search_entries,
            concept_index,
            explanation_index,
            realization_index,
            sense_index,
            senses_by_concept,
            redirect_index,
            search_index,
        }
    }

    pub fn concept(&self, concept_id: &str) -> Option<&GrammarConcept> {
        let concept_id = self.normalize_concept_id(concept_id);
        self.concept_index
            .get(concept_id)
            .and_then(|index| self.concepts.get(*index))
    }

    pub fn raw_concept(&self, concept_id: &str) -> Option<&GrammarConcept> {
        self.concept_index
            .get(concept_id)
            .and_then(|index| self.concepts.get(*index))
    }

    pub fn normalize_concept_id<'a>(&'a self, concept_id: &'a str) -> &'a str {
        let mut current = concept_id;
        let mut remaining = self.redirects.len() + 1;
        while remaining > 0 {
            let Some(next) = self.redirect_index.get(current) else {
                break;
            };
            current = next;
            remaining -= 1;
        }
        current
    }

    pub fn explanation(&self, explanation_id: &str) -> Option<&GrammarExplanationDocument> {
        self.explanation_index
            .get(explanation_id)
            .and_then(|index| self.explanations.get(*index))
    }

    pub fn realization(&self, realization_id: &str) -> Option<&GrammarRealization> {
        self.realization_index
            .get(realization_id)
            .and_then(|index| self.realizations.get(*index))
    }

    pub fn sense(&self, sense_id: &str) -> Option<&GrammarSense> {
        self.sense_index
            .get(sense_id)
            .and_then(|index| self.senses.get(*index))
    }

    pub fn senses_for(&self, concept_id: &str) -> Vec<&GrammarSense> {
        let concept_id = self.normalize_concept_id(concept_id);
        self.senses_by_concept
            .get(concept_id)
            .into_iter()
            .flatten()
            .filter_map(|index| self.senses.get(*index))
            .collect()
    }

    pub fn explanations_for(&self, concept_id: &str) -> Vec<&GrammarExplanationDocument> {
        let concept_id = self.normalize_concept_id(concept_id);
        self.explanations
            .iter()
            .filter(|explanation| explanation.concept_id == concept_id)
            .collect()
    }

    pub fn search(
        &self,
        query: Option<&str>,
        family: Option<&str>,
        jlpt_level: Option<u8>,
        audit_status: Option<&str>,
        source_ref: Option<&str>,
    ) -> Vec<&GrammarConcept> {
        let query = query.map(|value| value.to_lowercase());
        self.concepts
            .iter()
            .filter(|concept| concept.enabled)
            .filter(|concept| audit_status.is_none_or(|value| concept.audit_status == value))
            .filter(|concept| jlpt_level.is_none_or(|value| concept.jlpt_level == Some(value)))
            .filter(|concept| {
                source_ref.is_none_or(|value| {
                    concept.source_refs.iter().any(|source| source.contains(value))
                })
            })
            .filter(|concept| {
                let entry = self
                    .search_index
                    .get(&concept.concept_id)
                    .and_then(|index| self.search_entries.get(*index));
                family.is_none_or(|value| {
                    entry.is_some_and(|item| {
                        item.semantic_domains.iter().any(|domain| domain == value)
                            || item.function_tags.iter().any(|tag| tag == value)
                    }) || concept.kind == value
                })
            })
            .filter(|concept| {
                let Some(value) = &query else {
                    return true;
                };
                self.search_index
                    .get(&concept.concept_id)
                    .and_then(|index| self.search_entries.get(*index))
                    .is_some_and(|entry| {
                        entry.concept_id.to_lowercase().contains(value)
                            || entry.label.to_lowercase().contains(value)
                            || entry
                                .aliases
                                .iter()
                                .any(|item| item.to_lowercase().contains(value))
                            || entry
                                .surface_hints
                                .iter()
                                .any(|item| item.to_lowercase().contains(value))
                            || entry
                                .semantic_domains
                                .iter()
                                .any(|item| item.to_lowercase().contains(value))
                            || entry
                                .function_tags
                                .iter()
                                .any(|item| item.to_lowercase().contains(value))
                            || entry
                                .register
                                .iter()
                                .any(|item| item.to_lowercase().contains(value))
                            || entry
                                .related_concept_ids
                                .iter()
                                .any(|item| item.to_lowercase().contains(value))
                            || entry
                                .contrast_concept_ids
                                .iter()
                                .any(|item| item.to_lowercase().contains(value))
                            || entry
                                .sense_hints
                                .iter()
                                .any(|item| item.to_lowercase().contains(value))
                    })
            })
            .collect()
    }

    pub fn audit(&self) -> GrammarCatalogAudit {
        let missing_explanations = self
            .concepts
            .iter()
            .filter(|concept| self.explanation(&concept.default_explanation_id).is_none())
            .map(|concept| concept.concept_id.clone())
            .collect::<Vec<_>>();
        let dangling_rules = self
            .rules
            .iter()
            .filter(|rule| {
                self.concept(&rule.concept_id).is_none()
                    || self.realization(&rule.realization_id).is_none()
            })
            .map(|rule| rule.rule_id.clone())
            .collect::<Vec<_>>();
        let dangling_senses = self
            .senses
            .iter()
            .filter(|sense| {
                self.concept(&sense.concept_id).is_none()
                    || self.explanation(&sense.explanation_id).is_none()
            })
            .map(|sense| sense.sense_id.clone())
            .collect::<Vec<_>>();
        let dangling_realizations = self
            .realizations
            .iter()
            .filter(|realization| {
                self.concept(&realization.concept_id).is_none()
                    || !self.rules.iter().any(|rule| rule.rule_id == realization.rule_id)
                    || realization
                        .possible_sense_ids
                        .iter()
                        .any(|sense_id| {
                            self.sense(sense_id)
                                .is_none_or(|sense| sense.concept_id != realization.concept_id)
                        })
            })
            .map(|realization| realization.realization_id.clone())
            .collect::<Vec<_>>();
        let invalid_redirects = self
            .redirects
            .iter()
            .filter(|redirect| {
                self.raw_concept(&redirect.from_concept_id).is_none()
                    || self.raw_concept(&redirect.to_concept_id).is_none()
            })
            .map(|redirect| redirect.from_concept_id.clone())
            .collect::<Vec<_>>();
        let unpublishable_rules = self
            .rules
            .iter()
            .filter(|rule| rule.enabled && rule.audit_status == "verified")
            .filter(|rule| {
                let Some(concept) = self.concept(&rule.concept_id) else {
                    return true;
                };
                concept.audit_status != "verified"
                    || self
                        .explanation(&concept.default_explanation_id)
                        .is_none_or(|explanation| explanation.authoring_status != "verified")
            })
            .map(|rule| rule.rule_id.clone())
            .collect::<Vec<_>>();
        let missing_source_refs = self
            .concepts
            .iter()
            .filter(|concept| concept.audit_status == "verified" && concept.source_refs.is_empty())
            .map(|concept| format!("concept:{}", concept.concept_id))
            .chain(
                self.realizations
                    .iter()
                    .filter(|item| item.audit_status == "verified" && item.source_refs.is_empty())
                    .map(|item| format!("realization:{}", item.realization_id)),
            )
            .chain(
                self.rules
                    .iter()
                    .filter(|item| item.audit_status == "verified" && item.source_refs.is_empty())
                    .map(|item| format!("rule:{}", item.rule_id)),
            )
            .chain(
                self.explanations
                    .iter()
                    .filter(|item| item.authoring_status == "verified" && item.source_refs.is_empty())
                    .map(|item| format!("explanation:{}", item.explanation_id)),
            )
            .collect::<Vec<_>>();
        let missing_search_entries = self
            .concepts
            .iter()
            .filter(|concept| concept.enabled && !self.search_index.contains_key(&concept.concept_id))
            .map(|concept| concept.concept_id.clone())
            .collect::<Vec<_>>();
        GrammarCatalogAudit {
            catalog_version: self.catalog_version.clone(),
            concepts: self.concepts.len(),
            senses: self.senses.len(),
            realizations: self.realizations.len(),
            rules: self.rules.len(),
            explanations: self.explanations.len(),
            redirects: self.redirects.len(),
            missing_explanations,
            dangling_rules,
            dangling_senses,
            dangling_realizations,
            invalid_redirects,
            unpublishable_rules,
            missing_source_refs,
            missing_search_entries,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GrammarCatalogAudit {
    pub catalog_version: String,
    pub concepts: usize,
    pub senses: usize,
    pub realizations: usize,
    pub rules: usize,
    pub explanations: usize,
    pub redirects: usize,
    pub missing_explanations: Vec<String>,
    pub dangling_rules: Vec<String>,
    pub dangling_senses: Vec<String>,
    pub dangling_realizations: Vec<String>,
    pub invalid_redirects: Vec<String>,
    pub unpublishable_rules: Vec<String>,
    pub missing_source_refs: Vec<String>,
    pub missing_search_entries: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn embedded_catalog_has_no_dangling_core_references() {
        let catalog = GrammarCatalog::load_embedded().expect("目录应可加载");
        let audit = catalog.audit();
        assert!(audit.missing_explanations.is_empty());
        assert!(audit.dangling_rules.is_empty());
        assert!(audit.concepts >= 60);
    }

    #[test]
    fn redirects_resolve_stable_concept_identity() {
        let compiled: CompiledCatalog = serde_json::from_value(json!({
            "schema_version": 1,
            "catalog_version": "redirect-test",
            "concepts": [{
                "concept_id": "grammar.canonical",
                "kind": "functional_morpheme",
                "canonical_label": "规范概念",
                "jlpt_level": null,
                "default_explanation_id": "explanation.grammar.canonical",
                "audit_status": "verified",
                "concept_version": 1
            }],
            "senses": [],
            "realizations": [],
            "rules": [],
            "redirects": [
                {
                    "from_concept_id": "grammar.legacy",
                    "to_concept_id": "grammar.intermediate",
                    "reason": "旧版标识迁移",
                    "redirect_version": 1
                },
                {
                    "from_concept_id": "grammar.intermediate",
                    "to_concept_id": "grammar.canonical",
                    "reason": "稳定知识身份合并",
                    "redirect_version": 1
                }
            ]
        }))
        .expect("测试目录结构应有效");
        let explanations: CompiledExplanations = serde_json::from_value(json!({
            "schema_version": 1,
            "content_version": "redirect-test",
            "explanations": []
        }))
        .expect("测试讲解结构应有效");
        let catalog = GrammarCatalog::from_parts(compiled, explanations, Vec::new());

        assert_eq!(
            catalog.normalize_concept_id("grammar.legacy"),
            "grammar.canonical"
        );
        assert_eq!(
            catalog
                .concept("grammar.intermediate")
                .map(|concept| concept.concept_id.as_str()),
            Some("grammar.canonical")
        );
        assert_eq!(
            catalog.normalize_concept_id("grammar.unknown"),
            "grammar.unknown"
        );
    }
}
