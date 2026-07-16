use super::catalog::GrammarCatalog;
use crate::models::{
    GrammarContentBlock, GrammarDictionaryTarget, GrammarOccurrence, GrammarSenseCandidate,
    MorphologyArtifact, ResolvedGrammarExplanation,
};

pub struct GrammarExplanationResolver<'a> {
    catalog: &'a GrammarCatalog,
}

impl<'a> GrammarExplanationResolver<'a> {
    pub fn new(catalog: &'a GrammarCatalog) -> Self {
        Self { catalog }
    }

    pub fn resolve(
        &self,
        occurrence: &GrammarOccurrence,
        morphology: Option<&MorphologyArtifact>,
    ) -> Option<ResolvedGrammarExplanation> {
        let canonical_concept_id = self.catalog.normalize_concept_id(&occurrence.concept_id);
        let concept = self.catalog.concept(canonical_concept_id)?;
        let actual_form = join_surfaces(occurrence);
        let selected_sense = occurrence
            .selected_sense_id
            .as_deref()
            .and_then(|sense_id| occurrence.sense_candidates.iter().find(|item| item.sense_id == sense_id))
            .cloned();
        let alternative_senses = occurrence
            .sense_candidates
            .iter()
            .filter(|candidate| Some(candidate.sense_id.as_str()) != occurrence.selected_sense_id.as_deref())
            .cloned()
            .collect::<Vec<_>>();
        let explanation = self
            .catalog
            .explanation(&occurrence.explanation_ref)
            .or_else(|| {
                selected_sense
                    .as_ref()
                    .and_then(|candidate| self.catalog.sense(&candidate.sense_id))
                    .and_then(|sense| self.catalog.explanation(&sense.explanation_id))
            })
            .or_else(|| self.catalog.explanation(&concept.default_explanation_id))?;
        let content_blocks = explanation
            .body_blocks
            .iter()
            .filter_map(|block| {
                bind_template(&block.text, occurrence, &actual_form).map(|text| {
                    GrammarContentBlock {
                        kind: block.kind.clone(),
                        label: block.label.clone(),
                        text,
                    }
                })
            })
            .collect();
        let morphology_chain = morphology
            .into_iter()
            .flat_map(|artifact| &artifact.chains)
            .flat_map(|chain| &chain.operators)
            .filter(|operator| {
                occurrence
                    .matched_ranges
                    .iter()
                    .any(|range| ranges_overlap(operator.char_range, *range))
            })
            .map(|operator| {
                self.catalog
                    .concept(&operator.concept_id)
                    .map(|item| item.canonical_label.clone())
                    .unwrap_or_else(|| operator.output_state.clone())
            })
            .collect::<Vec<_>>();
        let dictionary_targets = occurrence
            .captures
            .iter()
            .filter(|capture| {
                matches!(
                    capture.name.as_str(),
                    "functional_verb" | "support_verb" | "predicate"
                ) && !capture.base_form.is_empty()
            })
            .map(|capture| GrammarDictionaryTarget {
                label: format!("查看「{}」的词典", capture.base_form),
                base_form: capture.base_form.clone(),
                reading: String::new(),
                char_range: capture.char_range,
            })
            .collect();
        Some(ResolvedGrammarExplanation {
            status: if occurrence.selected_sense_id.is_some() || occurrence.sense_candidates.len() <= 1 {
                "resolved".to_string()
            } else {
                "partial".to_string()
            },
            occurrence_summary: format!("{}：{}", concept.canonical_label, actual_form),
            concept_id: concept.concept_id.clone(),
            title: concept.canonical_label.clone(),
            compact_summary: bind_template(
                &explanation.compact_summary,
                occurrence,
                &actual_form,
            )
            .unwrap_or_default(),
            function_summary: selected_sense
                .as_ref()
                .map(|sense| sense.label.clone())
                .unwrap_or_else(|| {
                    bind_template(&explanation.function_summary, occurrence, &actual_form)
                        .unwrap_or_default()
                }),
            connection: bind_template(&explanation.connection, occurrence, &actual_form)
                .unwrap_or_default(),
            actual_form,
            selected_sense,
            alternative_senses,
            bound_captures: occurrence.captures.clone(),
            morphology_chain,
            content_blocks,
            evidence: occurrence.evidence.clone(),
            related_concept_ids: concept.related_concept_ids.clone(),
            contrast_concept_ids: concept.contrast_concept_ids.clone(),
            dictionary_targets,
            source_refs: explanation.source_refs.clone(),
            provenance: explanation.provenance.clone(),
            review_status: explanation.review_status.clone(),
            available_depths: vec!["compact".to_string(), "standard".to_string(), "deep".to_string()],
            content_version: explanation.content_version,
            audit_status: concept.audit_status.clone(),
        })
    }
}

fn join_surfaces(occurrence: &GrammarOccurrence) -> String {
    if occurrence.captures.is_empty() {
        return occurrence
            .display_ranges
            .iter()
            .map(|range| format!("{}..{}", range.0, range.1))
            .collect::<Vec<_>>()
            .join("、");
    }
    occurrence
        .captures
        .iter()
        .map(|capture| capture.surface.as_str())
        .collect::<Vec<_>>()
        .join("")
}

fn bind_template(
    template: &str,
    occurrence: &GrammarOccurrence,
    actual_form: &str,
) -> Option<String> {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let placeholder = &rest[start + 2..];
        let end = placeholder.find("}}")?;
        let key = placeholder[..end].trim();
        if key == "actual_form" {
            output.push_str(actual_form);
        } else {
            let mut captures = occurrence
                .captures
                .iter()
                .filter(|capture| capture.name == key)
                .collect::<Vec<_>>();
            captures.sort_by_key(|capture| capture.char_range);
            if captures.is_empty() {
                return None;
            }
            for capture in captures {
                output.push_str(&capture.surface);
            }
        }
        rest = &placeholder[end + 2..];
    }
    output.push_str(rest);
    Some(output)
}

fn ranges_overlap(left: (usize, usize), right: (usize, usize)) -> bool {
    left.0 < right.1 && right.0 < left.1
}

pub fn sense_candidates(
    catalog: &GrammarCatalog,
    concept_id: &str,
    confidence: u8,
) -> Vec<GrammarSenseCandidate> {
    let senses = catalog.senses_for(concept_id);
    let count = senses.len().max(1) as u8;
    senses
        .into_iter()
        .map(|sense| GrammarSenseCandidate {
            sense_id: sense.sense_id.clone(),
            label: if sense.function_summary.trim().is_empty() {
                sense.label.clone()
            } else {
                sense.function_summary.clone()
            },
            confidence: if count == 1 { confidence } else { confidence.saturating_sub((count - 1) * 4) },
            evidence: sense.context_requirements.clone(),
        })
        .collect()
}

pub fn realization_sense_candidates(
    catalog: &GrammarCatalog,
    concept_id: &str,
    possible_sense_ids: &[String],
    confidence: u8,
) -> Vec<GrammarSenseCandidate> {
    let mut candidates = sense_candidates(catalog, concept_id, confidence);
    if possible_sense_ids.is_empty() {
        return candidates;
    }
    candidates.retain(|candidate| possible_sense_ids.contains(&candidate.sense_id));
    let count = candidates.len().max(1) as u8;
    for candidate in &mut candidates {
        candidate.confidence = if count == 1 {
            confidence
        } else {
            confidence.saturating_sub((count - 1) * 4)
        };
    }
    candidates
}
