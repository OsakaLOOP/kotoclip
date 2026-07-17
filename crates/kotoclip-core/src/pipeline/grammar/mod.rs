pub mod catalog;
pub mod resolve;

use self::catalog::{GrammarCatalog, GrammarRule, GrammarRuleAtom};
use self::resolve::{realization_sense_candidates, sense_candidates, GrammarExplanationResolver};
use crate::models::{
    AnnotatedToken, Bunsetsu, FunctionalResidual, GrammarCapture, GrammarOccurrence,
    GrammarOccurrenceKind, GrammarOccurrenceStatus, GrammarTag, Morpheme, MorphologyArtifact,
    MorphologyChainRole,
};
use crate::pipeline::morphology;
use std::collections::HashMap;

pub const ANALYZER_VERSION: &str = "grammar-2";

/// 将分段分析产生的局部 Token／语素坐标重建为文档级规范坐标。
///
/// 字符范围始终是规范事实，因此这里从字符范围重新推导坐标，而不是在已有
/// 数值上叠加 offset。该函数可重复调用，适用于全量分析、渐进批次和暖缓存恢复。
pub fn canonicalize_document_coordinates(tokens: &mut [AnnotatedToken]) {
    let token_ranges = tokens
        .iter()
        .map(|token| token.bunsetsu.char_range)
        .collect::<Vec<_>>();
    let morpheme_ranges = tokens
        .iter()
        .flat_map(|token| token.bunsetsu.morphemes.iter().map(|morpheme| morpheme.char_range))
        .collect::<Vec<_>>();

    let morpheme_range_for = |char_range: (usize, usize)| {
        range_bounds(&morpheme_ranges, char_range).unwrap_or((0, 0))
    };
    let token_range_for = |ranges: &[(usize, usize)]| {
        let mut first = None;
        let mut last = None;
        for range in ranges {
            if let Some((start, end)) = range_bounds(&token_ranges, *range) {
                first = Some(first.map_or(start, |value: usize| value.min(start)));
                last = Some(last.map_or(end, |value: usize| value.max(end)));
            }
        }
        first.zip(last).unwrap_or((0, 0))
    };

    for token in tokens.iter_mut() {
        for tag in &mut token.bunsetsu.grammar_tags {
            tag.morpheme_range = morpheme_range_for(tag.char_range);
            if let Some(explanation) = &mut tag.explanation {
                for capture in &mut explanation.bound_captures {
                    capture.morpheme_range = morpheme_range_for(capture.char_range);
                }
            }
        }
        for chain in &mut token.bunsetsu.morphology.chains {
            chain.anchor_morpheme = morpheme_range_for(chain.anchor_range).0;
            chain.morpheme_range = morpheme_range_for(chain.char_range);
            chain.chain_id = format!("morph:{}:{}", chain.char_range.0, chain.char_range.1);
            for operator in &mut chain.operators {
                operator.source_morpheme_range = morpheme_range_for(operator.char_range);
                operator.operator_id =
                    format!("{}:{}", operator.concept_id, operator.char_range.0);
            }
        }
        let mut occurrence_id_map = HashMap::new();
        for occurrence in &mut token.bunsetsu.grammar_occurrences {
            occurrence.covered_token_range = token_range_for(&occurrence.matched_ranges);
            for capture in &mut occurrence.captures {
                capture.morpheme_range = morpheme_range_for(capture.char_range);
            }
            if occurrence.kind == GrammarOccurrenceKind::MorphologyFeature
                && occurrence.concept_id != "morphology.chain"
            {
                occurrence.rule_id =
                    format!("{}:{}", occurrence.concept_id, occurrence.anchor_range.0);
            }
            let previous_id = occurrence.occurrence_id.clone();
            occurrence.occurrence_id = occurrence_id(
                &occurrence.concept_id,
                &occurrence.rule_id,
                &occurrence.matched_ranges,
            );
            occurrence_id_map.insert(previous_id, occurrence.occurrence_id.clone());
        }
        for occurrence in &mut token.bunsetsu.grammar_occurrences {
            for component_id in &mut occurrence.component_occurrence_ids {
                if let Some(canonical_id) = occurrence_id_map.get(component_id) {
                    *component_id = canonical_id.clone();
                }
            }
        }
        for tag in &mut token.bunsetsu.grammar_tags {
            tag.occurrence_id =
                occurrence_id(&tag.concept_id, &tag.pattern_id, &tag.display_ranges);
        }
    }
}

/// 顺序追加文档批次时，分析结果已经在批次局部坐标中完成规范化。这里仅将
/// token／语素下标平移到文档坐标，避免为每个新批次重新扫描已完成全文。
pub fn offset_document_coordinates(
    tokens: &mut [AnnotatedToken],
    token_offset: usize,
    morpheme_offset: usize,
) {
    let offset_range = |range: &mut (usize, usize), offset: usize| {
        if range.0 != 0 || range.1 != 0 {
            range.0 += offset;
            range.1 += offset;
        }
    };
    for token in tokens {
        for tag in &mut token.bunsetsu.grammar_tags {
            offset_range(&mut tag.morpheme_range, morpheme_offset);
            if let Some(explanation) = &mut tag.explanation {
                for capture in &mut explanation.bound_captures {
                    offset_range(&mut capture.morpheme_range, morpheme_offset);
                }
            }
        }
        for chain in &mut token.bunsetsu.morphology.chains {
            chain.anchor_morpheme += morpheme_offset;
            offset_range(&mut chain.morpheme_range, morpheme_offset);
            for operator in &mut chain.operators {
                offset_range(&mut operator.source_morpheme_range, morpheme_offset);
            }
        }
        for occurrence in &mut token.bunsetsu.grammar_occurrences {
            offset_range(&mut occurrence.covered_token_range, token_offset);
            for capture in &mut occurrence.captures {
                offset_range(&mut capture.morpheme_range, morpheme_offset);
            }
        }
    }
}

/// `canonicalize_document_coordinates` 在 Token 已按字符位置排序后调用；范围边界
/// 因而可用二分查找定位，避免每个语法字段重新扫描整篇文档。
fn range_bounds(ranges: &[(usize, usize)], target: (usize, usize)) -> Option<(usize, usize)> {
    if target.0 >= target.1 || ranges.is_empty() {
        return None;
    }
    let mut low = 0;
    let mut high = ranges.len();
    while low < high {
        let middle = (low + high) / 2;
        if ranges[middle].1 <= target.0 {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    let start = low;

    low = start;
    high = ranges.len();
    while low < high {
        let middle = (low + high) / 2;
        if ranges[middle].0 < target.1 {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    (start < low).then_some((start, low))
}

#[derive(Debug, Clone)]
struct FlatMorpheme {
    morpheme: Morpheme,
    bunsetsu_index: usize,
    global_index: usize,
    morphology_features: Vec<String>,
}

#[derive(Debug)]
struct Candidate {
    occurrence: GrammarOccurrence,
    priority: i32,
    refines_rule_ids: Vec<String>,
    conflict_group: Option<String>,
}

pub struct GrammarMatcher {
    catalog: GrammarCatalog,
}

impl GrammarMatcher {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            catalog: GrammarCatalog::load_embedded()?,
        })
    }

    pub fn catalog(&self) -> &GrammarCatalog {
        &self.catalog
    }

    pub fn match_patterns(&self, bunsetsus: &mut [Bunsetsu]) {
        if bunsetsus.is_empty() {
            return;
        }
        for bunsetsu in bunsetsus.iter_mut() {
            bunsetsu.grammar_tags.clear();
            bunsetsu.grammar_occurrences.clear();
            bunsetsu.functional_residuals.clear();
        }

        let mut global_offset = 0;
        for bunsetsu in bunsetsus.iter_mut() {
            bunsetsu.morphology = morphology::analyze_bunsetsu(bunsetsu, global_offset);
            self.enrich_morphology(&mut bunsetsu.morphology);
            morphology::apply_lexical_head(bunsetsu);
            global_offset += bunsetsu.morphemes.len();
        }
        let flat = flatten(bunsetsus);
        let mut candidates = self.morphology_candidates(bunsetsus, &flat);
        candidates.extend(self.rule_candidates(&flat));
        resolve_candidate_relations(&mut candidates);

        let occurrences = candidates
            .into_iter()
            .map(|candidate| candidate.occurrence)
            .collect::<Vec<_>>();
        self.assign_residuals(bunsetsus, &flat, &occurrences);
        self.attach_occurrences_and_tags(bunsetsus, &flat, occurrences);
    }

    fn enrich_morphology(&self, artifact: &mut MorphologyArtifact) {
        for operator in artifact
            .chains
            .iter_mut()
            .flat_map(|chain| &mut chain.operators)
        {
            let canonical_id = self.catalog.normalize_concept_id(&operator.concept_id);
            let Some(concept) = self.catalog.concept(canonical_id) else {
                continue;
            };
            operator.concept_id = canonical_id.to_string();
            operator.label = concept.canonical_label.clone();
            operator.description = self
                .catalog
                .explanation(&concept.default_explanation_id)
                .map(|explanation| explanation.compact_summary.clone())
                .unwrap_or_default();
        }
    }

    fn morphology_candidates(&self, bunsetsus: &[Bunsetsu], flat: &[FlatMorpheme]) -> Vec<Candidate> {
        let mut result = Vec::new();
        for (bunsetsu_index, bunsetsu) in bunsetsus.iter().enumerate() {
            for chain in &bunsetsu.morphology.chains {
                let mut component_ids = Vec::new();
                for operator in &chain.operators {
                    let canonical_concept_id = self.catalog.normalize_concept_id(&operator.concept_id).to_string();
                    let Some(concept) = self.catalog.concept(&canonical_concept_id) else {
                        continue;
                    };
                    let candidates = sense_candidates(&self.catalog, &canonical_concept_id, operator.confidence);
                    let selected = (candidates.len() == 1).then(|| candidates[0].sense_id.clone());
                    let explanation_ref = selected
                        .as_deref()
                        .and_then(|sense_id| self.catalog.sense(sense_id))
                        .map(|sense| sense.explanation_id.clone())
                        .unwrap_or_else(|| concept.default_explanation_id.clone());
                    let status = published_status(&self.catalog, concept, &explanation_ref);
                    let occurrence_id = occurrence_id(&canonical_concept_id, &operator.operator_id, &[operator.char_range]);
                    component_ids.push(occurrence_id.clone());
                    result.push(Candidate {
                        occurrence: GrammarOccurrence {
                            occurrence_id,
                            concept_id: canonical_concept_id.clone(),
                            rule_id: operator.operator_id.clone(),
                            kind: GrammarOccurrenceKind::MorphologyFeature,
                            status,
                            matched_ranges: vec![operator.char_range],
                            covered_token_range: (bunsetsu_index, bunsetsu_index + 1),
                            display_ranges: vec![operator.char_range],
                            anchor_range: operator.char_range,
                            component_occurrence_ids: Vec::new(),
                            captures: capture_from_range("operator", flat, operator.source_morpheme_range),
                            selected_sense_id: selected,
                            sense_candidates: candidates,
                            confidence: operator.confidence,
                            evidence: operator.evidence.clone(),
                            counter_evidence: Vec::new(),
                            explanation_ref,
                            analyzer_version: ANALYZER_VERSION.to_string(),
                            catalog_version: self.catalog.catalog_version.clone(),
                            knowledge_item_id: canonical_concept_id,
                            show_badge: false,
                        },
                        priority: 120,
                        refines_rule_ids: Vec::new(),
                        conflict_group: None,
                    });
                }
                if chain.operators.is_empty() {
                    continue;
                }
                let matched_ranges = merge_ranges(
                    chain
                        .operators
                        .iter()
                        .map(|operator| operator.char_range)
                        .collect(),
                );
                let display_ranges = merge_ranges(
                    chain
                        .operators
                        .iter()
                        .filter(|operator| operator.source_morpheme_range.0 != chain.anchor_morpheme)
                        .map(|operator| operator.char_range)
                        .collect(),
                );
                let mut captures = chain
                    .operators
                    .iter()
                    .flat_map(|operator| capture_from_range("operator", flat, operator.source_morpheme_range))
                    .collect::<Vec<_>>();
                captures.sort_by_key(|capture| capture.char_range);
                captures.dedup_by_key(|capture| capture.char_range);
                let confidence = chain
                    .operators
                    .iter()
                    .map(|operator| operator.confidence)
                    .min()
                    .unwrap_or(90);
                if let Some(concept) = self.catalog.concept("morphology.chain") {
                    let explanation_ref = concept.default_explanation_id.clone();
                    let status = published_status(&self.catalog, concept, &explanation_ref);
                    let anchor_range = display_ranges
                        .first()
                        .zip(display_ranges.last())
                        .map(|(first, last)| (first.0, last.1))
                        .or_else(|| {
                            flat.iter()
                                .find(|item| item.global_index == chain.anchor_morpheme)
                                .map(|item| item.morpheme.char_range)
                        })
                        .unwrap_or(bunsetsu.char_range);
                    result.push(Candidate {
                        occurrence: GrammarOccurrence {
                            occurrence_id: occurrence_id("morphology.chain", &chain.chain_id, &display_ranges),
                            concept_id: "morphology.chain".to_string(),
                            rule_id: "morphology.chain.aggregate".to_string(),
                            kind: GrammarOccurrenceKind::MorphologyFeature,
                            status,
                            matched_ranges,
                            covered_token_range: (bunsetsu_index, bunsetsu_index + 1),
                            display_ranges: display_ranges.clone(),
                            anchor_range,
                            component_occurrence_ids: component_ids,
                            captures,
                            selected_sense_id: None,
                            sense_candidates: Vec::new(),
                            confidence,
                            evidence: chain.evidence.clone(),
                            counter_evidence: Vec::new(),
                            explanation_ref,
                            analyzer_version: ANALYZER_VERSION.to_string(),
                            catalog_version: self.catalog.catalog_version.clone(),
                            knowledge_item_id: "morphology.chain".to_string(),
                            show_badge: true,
                        },
                        priority: 125,
                        refines_rule_ids: Vec::new(),
                        conflict_group: None,
                    });
                }
            }
        }
        result
    }

    fn rule_candidates(&self, flat: &[FlatMorpheme]) -> Vec<Candidate> {
        let mut result = Vec::new();
        for rule in self
            .catalog
            .rules
            .iter()
            .filter(|rule| rule.enabled && rule.kind != "morphology_feature")
        {
            for start in 0..flat.len() {
                let Some(matched) = match_rule_at(rule, flat, start) else {
                    continue;
                };
                let indices = matched.iter().flatten().copied().collect::<Vec<_>>();
                if indices.is_empty() {
                    continue;
                }
                let matched_ranges = merge_ranges(indices.iter().map(|index| flat[*index].morpheme.char_range).collect());
                let display_end = rule.display_to.unwrap_or(rule.atoms.len()).min(rule.atoms.len());
                let display_ranges = merge_ranges(
                    matched
                        .iter()
                        .enumerate()
                        .filter(|(atom_index, _)| *atom_index >= rule.display_from && *atom_index < display_end)
                        .filter_map(|(_, index)| index.map(|value| flat[value].morpheme.char_range))
                        .collect(),
                );
                if display_ranges.is_empty() {
                    continue;
                }
                let canonical_concept_id = self.catalog.normalize_concept_id(&rule.concept_id).to_string();
                let Some(concept) = self.catalog.concept(&canonical_concept_id) else {
                    continue;
                };
                let mut captures = Vec::new();
                let mut provider_evidence = Vec::new();
                for (atom_index, flat_index) in matched.iter().enumerate() {
                    let Some(flat_index) = *flat_index else {
                        continue;
                    };
                    let atom = &rule.atoms[atom_index];
                    let item = &flat[flat_index];
                    if atom.provider_components.is_empty() {
                        let Some(capture) = atom.capture.as_deref() else {
                            continue;
                        };
                        captures.push(GrammarCapture {
                            name: capture.to_string(),
                            surface: item.morpheme.surface.clone(),
                            base_form: item.morpheme.base_form.clone(),
                            morpheme_range: (item.global_index, item.global_index + 1),
                            char_range: item.morpheme.char_range,
                        });
                        continue;
                    }
                    let mut offset = item.morpheme.char_range.0;
                    for component in &atom.provider_components {
                        let end = offset + component.surface.chars().count();
                        captures.push(GrammarCapture {
                            name: component.role.clone(),
                            surface: component.surface.clone(),
                            base_form: component.base_form.clone(),
                            morpheme_range: (item.global_index, item.global_index + 1),
                            char_range: (offset, end),
                        });
                        offset = end;
                    }
                    provider_evidence.push(format!(
                        "provider_decomposition:{}=>{}",
                        item.morpheme.surface,
                        atom.provider_components
                            .iter()
                            .map(|component| component.base_form.as_str())
                            .collect::<Vec<_>>()
                            .join("+")
                    ));
                }
                let senses = self
                    .catalog
                    .realization(&rule.realization_id)
                    .map(|realization| {
                        realization_sense_candidates(
                            &self.catalog,
                            &canonical_concept_id,
                            &realization.possible_sense_ids,
                            96,
                        )
                    })
                    .unwrap_or_else(|| sense_candidates(&self.catalog, &canonical_concept_id, 96));
                let selected_sense_id = (senses.len() == 1).then(|| senses[0].sense_id.clone());
                let explanation_ref = selected_sense_id
                    .as_deref()
                    .and_then(|sense_id| self.catalog.sense(sense_id))
                    .map(|sense| sense.explanation_id.clone())
                    .unwrap_or_else(|| concept.default_explanation_id.clone());
                let first = indices[0];
                let last = *indices.last().unwrap();
                let explanation_verified = self
                    .catalog
                    .explanation(&explanation_ref)
                    .is_some_and(|explanation| explanation.authoring_status == "verified");
                let status = if rule.audit_status == "verified"
                    && concept.audit_status == "verified"
                    && explanation_verified
                {
                    GrammarOccurrenceStatus::Accepted
                } else {
                    GrammarOccurrenceStatus::Pending
                };
                let occurrence = GrammarOccurrence {
                    occurrence_id: occurrence_id(&canonical_concept_id, &rule.rule_id, &matched_ranges),
                    concept_id: canonical_concept_id.clone(),
                    rule_id: rule.rule_id.clone(),
                    kind: parse_kind(&rule.kind),
                    status,
                    matched_ranges,
                    covered_token_range: (flat[first].bunsetsu_index, flat[last].bunsetsu_index + 1),
                    display_ranges: display_ranges.clone(),
                    anchor_range: (display_ranges[0].0, display_ranges.last().unwrap().1),
                    component_occurrence_ids: Vec::new(),
                    captures,
                    selected_sense_id,
                    sense_candidates: senses,
                    confidence: 96,
                    evidence: vec![
                        format!("rule:{}", rule.rule_id),
                        format!("realization:{}", rule.realization_id),
                    ]
                    .into_iter()
                    .chain(provider_evidence)
                    .collect(),
                    counter_evidence: Vec::new(),
                    explanation_ref,
                    analyzer_version: ANALYZER_VERSION.to_string(),
                    catalog_version: self.catalog.catalog_version.clone(),
                    knowledge_item_id: canonical_concept_id,
                    show_badge: rule.show_badge,
                };
                result.push(Candidate {
                    occurrence,
                    priority: rule.priority,
                    refines_rule_ids: rule.refines_rule_ids.clone(),
                    conflict_group: rule.conflict_group.clone(),
                });
            }
        }
        deduplicate_candidates(&self.catalog, result)
    }

    fn assign_residuals(
        &self,
        bunsetsus: &mut [Bunsetsu],
        flat: &[FlatMorpheme],
        occurrences: &[GrammarOccurrence],
    ) {
        let covered = occurrences
            .iter()
            .filter(|occurrence| {
                occurrence.status == GrammarOccurrenceStatus::Accepted
                    && matches!(
                        occurrence.kind,
                        GrammarOccurrenceKind::FunctionalMorpheme
                            | GrammarOccurrenceKind::MorphologyFeature
                            | GrammarOccurrenceKind::GrammarConstruction
                    )
            })
            .flat_map(|occurrence| occurrence.matched_ranges.iter().copied())
            .collect::<Vec<_>>();
        for item in flat {
            let morpheme = &item.morpheme;
            if morpheme.pos.major == "記号" || morpheme.surface.trim().is_empty() {
                continue;
            }
            let formal_noun = morpheme.pos.major == "名詞"
                && morpheme.pos.sub1 == "非自立"
                && matches!(morpheme.base_form.as_str(), "こと" | "もの" | "ところ" | "わけ" | "はず" | "つもり" | "ため" | "まま" | "の" | "ん");
            let previous = item
                .global_index
                .checked_sub(1)
                .and_then(|index| flat.get(index))
                .map(|item| &item.morpheme);
            let functional_verb_context = if morpheme.pos.major == "動詞" && morpheme.pos.sub1 == "非自立" {
                if morpheme.base_form == "てる" {
                    previous.is_some_and(|item| item.pos.major == "動詞")
                } else if matches!(
                    morpheme.base_form.as_str(),
                    "いる" | "ある" | "おく" | "みる" | "見る" | "しまう" | "いく" | "行く" | "くる" | "来る" | "やる" | "あげる" | "くれる" | "くださる" | "もらう"
                ) {
                    previous.is_some_and(|item| matches!(item.base_form.as_str(), "て" | "で"))
                } else {
                    false
                }
            } else {
                false
            };
            let grammatical_suffix = morpheme.pos.major == "動詞"
                && morpheme.pos.sub1 == "接尾"
                && matches!(
                    morpheme.base_form.as_str(),
                    "せる" | "させる" | "す" | "れる" | "られる" | "がる" | "やす"
                );
            let lexical = match morpheme.pos.major.as_str() {
                "名詞" => !formal_noun,
                "動詞" => !functional_verb_context && !grammatical_suffix,
                "形容詞" | "副詞" | "連体詞" | "接続詞" | "感動詞" | "接頭詞" => true,
                "フィラー" => true,
                _ => false,
            };
            let classified = lexical
                || covered
                    .iter()
                    .any(|range| contains_range(*range, morpheme.char_range));
            if !classified {
                bunsetsus[item.bunsetsu_index]
                    .functional_residuals
                    .push(FunctionalResidual {
                        surface: morpheme.surface.clone(),
                        base_form: morpheme.base_form.clone(),
                        pos: morpheme.pos.clone(),
                        conjugation_type: morpheme.conjugation_type.clone(),
                        conjugation_form: morpheme.conjugation_form.clone(),
                        char_range: morpheme.char_range,
                        reason: if morpheme.conjugation_type.starts_with("文語")
                            || matches!(morpheme.base_form.as_str(), "り" | "つ" | "き" | "たり")
                        {
                            "provider_or_language_profile_review".to_string()
                        } else {
                            "catalog_missing_or_unresolved".to_string()
                        },
                    });
            }
        }
    }

    fn attach_occurrences_and_tags(
        &self,
        bunsetsus: &mut [Bunsetsu],
        flat: &[FlatMorpheme],
        mut occurrences: Vec<GrammarOccurrence>,
    ) {
        for occurrence in &mut occurrences {
            expand_functional_inflection(occurrence, bunsetsus);
        }
        let resolver = GrammarExplanationResolver::new(&self.catalog);
        let global_to_bunsetsu = flat
            .iter()
            .map(|item| (item.global_index, item.bunsetsu_index))
            .collect::<HashMap<_, _>>();
        for occurrence in occurrences {
            let anchor_bunsetsu = occurrence.covered_token_range.0.min(bunsetsus.len() - 1);
            let morphology = bunsetsus.get(anchor_bunsetsu).map(|item| &item.morphology);
            let explanation = resolver.resolve(&occurrence, morphology);
            if occurrence.status == GrammarOccurrenceStatus::Accepted
                && occurrence.kind != GrammarOccurrenceKind::MorphologyFeature
                && !absorbed_by_lexical_inflection(&occurrence, bunsetsus)
            {
                let label = self.catalog
                    .concept(&occurrence.concept_id)
                    .map(|item| item.canonical_label.clone())
                    .unwrap_or_else(|| occurrence.concept_id.clone());
                let description = explanation
                    .as_ref()
                    .map(|item| item.compact_summary.clone())
                    .unwrap_or_default();
                let morpheme_range = occurrence
                    .captures
                    .iter()
                    .fold((usize::MAX, 0), |range, capture| {
                        (range.0.min(capture.morpheme_range.0), range.1.max(capture.morpheme_range.1))
                    });
                let morpheme_range = if morpheme_range.0 == usize::MAX {
                    let first = flat.iter().find(|item| contains_range(occurrence.anchor_range, item.morpheme.char_range));
                    first.map(|item| (item.global_index, item.global_index + 1)).unwrap_or((0, 0))
                } else {
                    morpheme_range
                };
                let tag = GrammarTag {
                    pattern_id: occurrence.rule_id.clone(),
                    name_ja: label,
                    name_en: occurrence.concept_id.clone(),
                    jlpt_level: self.catalog.concept(&occurrence.concept_id).and_then(|item| item.jlpt_level),
                    description,
                    morpheme_range,
                    char_range: occurrence.anchor_range,
                    occurrence_id: occurrence.occurrence_id.clone(),
                    concept_id: occurrence.concept_id.clone(),
                    occurrence_kind: occurrence.kind.clone(),
                    status: occurrence.status.clone(),
                    show_badge: occurrence.show_badge,
                    display_ranges: occurrence.display_ranges.clone(),
                    selected_sense_id: occurrence.selected_sense_id.clone(),
                    sense_candidates: occurrence.sense_candidates.clone(),
                    explanation,
                };
                let mut touched = bunsetsus
                    .iter()
                    .enumerate()
                    .filter(|(_, bunsetsu)| {
                        occurrence
                            .display_ranges
                            .iter()
                            .any(|range| ranges_overlap(*range, bunsetsu.char_range))
                    })
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>();
                if touched.is_empty() {
                    touched.push(anchor_bunsetsu);
                }
                for bunsetsu_index in touched {
                    let mut projected = tag.clone();
                    projected.show_badge &= bunsetsu_index == anchor_bunsetsu;
                    if !bunsetsus[bunsetsu_index]
                        .grammar_tags
                        .iter()
                        .any(|existing| existing.occurrence_id == projected.occurrence_id)
                    {
                        bunsetsus[bunsetsu_index].grammar_tags.push(projected);
                    }
                }
            }
            let target = global_to_bunsetsu
                .iter()
                .find(|(_, bunsetsu_index)| **bunsetsu_index == anchor_bunsetsu)
                .map(|(_, bunsetsu_index)| *bunsetsu_index)
                .unwrap_or(anchor_bunsetsu);
            bunsetsus[target].grammar_occurrences.push(occurrence);
        }
        for bunsetsu in bunsetsus {
            bunsetsu.grammar_tags.sort_by_key(|tag| (tag.char_range.0, !tag.show_badge, tag.char_range.1));
            bunsetsu.grammar_occurrences.sort_by_key(|item| (item.anchor_range.0, item.anchor_range.1));
        }
    }
}

fn expand_functional_inflection(
    occurrence: &mut GrammarOccurrence,
    bunsetsus: &[Bunsetsu],
) {
    if occurrence.kind == GrammarOccurrenceKind::MorphologyFeature {
        return;
    }
    for chain in bunsetsus
        .iter()
        .flat_map(|bunsetsu| &bunsetsu.morphology.chains)
        .filter(|chain| chain.role == MorphologyChainRole::Functional)
    {
        let touches_anchor = occurrence
            .display_ranges
            .iter()
            .any(|range| ranges_overlap(*range, chain.anchor_range));
        if !touches_anchor {
            continue;
        }
        occurrence
            .matched_ranges
            .extend(chain.source_ranges.iter().copied());
        occurrence
            .display_ranges
            .extend(chain.source_ranges.iter().copied());
        for capture in &mut occurrence.captures {
            if ranges_overlap(capture.char_range, chain.anchor_range)
                && (matches!(
                    capture.name.as_str(),
                    "functional_verb" | "support_verb" | "auxiliary"
                ) || capture.base_form == chain.base_lexeme)
            {
                capture.surface = chain.surface_form.clone();
                capture.char_range = chain.char_range;
                capture.morpheme_range = chain.morpheme_range;
            }
        }
    }
    occurrence.matched_ranges = merge_ranges(std::mem::take(&mut occurrence.matched_ranges));
    occurrence.display_ranges = merge_ranges(std::mem::take(&mut occurrence.display_ranges));
}

fn absorbed_by_lexical_inflection(
    occurrence: &GrammarOccurrence,
    bunsetsus: &[Bunsetsu],
) -> bool {
    if occurrence.kind != GrammarOccurrenceKind::FunctionalMorpheme
        || occurrence.display_ranges.is_empty()
    {
        return false;
    }
    bunsetsus
        .iter()
        .flat_map(|bunsetsu| &bunsetsu.morphology.chains)
        .filter(|chain| chain.role == MorphologyChainRole::Lexical)
        .flat_map(|chain| &chain.operators)
        .filter(|operator| operator.kind != "connection_form")
        .any(|operator| {
            occurrence
                .display_ranges
                .iter()
                .all(|range| contains_range(operator.char_range, *range))
        })
}

fn flatten(bunsetsus: &[Bunsetsu]) -> Vec<FlatMorpheme> {
    let mut result = Vec::new();
    let mut global_index = 0;
    for (bunsetsu_index, bunsetsu) in bunsetsus.iter().enumerate() {
        for morpheme in &bunsetsu.morphemes {
            let mut morphology_features = bunsetsu
                .morphology
                .chains
                .iter()
                .flat_map(|chain| &chain.operators)
                .filter(|operator| {
                    global_index >= operator.source_morpheme_range.0
                        && global_index < operator.source_morpheme_range.1
                })
                .flat_map(|operator| {
                    [operator.output_state.clone(), operator.concept_id.clone()]
                })
                .collect::<Vec<_>>();
            morphology_features.sort();
            morphology_features.dedup();
            result.push(FlatMorpheme {
                morpheme: morpheme.clone(),
                bunsetsu_index,
                global_index,
                morphology_features,
            });
            global_index += 1;
        }
    }
    result
}

fn match_rule_at(rule: &GrammarRule, flat: &[FlatMorpheme], start: usize) -> Option<Vec<Option<usize>>> {
    let mut cursor = start;
    let mut matched = Vec::with_capacity(rule.atoms.len());
    for atom in &rule.atoms {
        if cursor < flat.len() && atom_matches(atom, &flat[cursor]) {
            if cursor > start && crosses_hard_boundary(&flat[cursor - 1].morpheme, &flat[cursor].morpheme) {
                return None;
            }
            matched.push(Some(cursor));
            cursor += 1;
        } else if atom.optional {
            matched.push(None);
        } else {
            return None;
        }
    }
    Some(matched)
}

fn atom_matches(atom: &GrammarRuleAtom, item: &FlatMorpheme) -> bool {
    let morpheme = &item.morpheme;
    matches_filter(&atom.surfaces, &morpheme.surface)
        && matches_filter(&atom.base_forms, &morpheme.base_form)
        && matches_filter(&atom.pos_major, &morpheme.pos.major)
        && matches_filter(&atom.pos_sub1, &morpheme.pos.sub1)
        && matches_prefix_filter(&atom.conjugation_types, &morpheme.conjugation_type)
        && matches_filter(&atom.conjugation_forms, &morpheme.conjugation_form)
        && matches_any_filter(&atom.morphology_features, &item.morphology_features)
        && provider_components_match(atom, morpheme)
}

fn provider_components_match(atom: &GrammarRuleAtom, morpheme: &Morpheme) -> bool {
    atom.provider_components.is_empty()
        || atom
            .provider_components
            .iter()
            .map(|component| component.surface.as_str())
            .collect::<String>()
            == morpheme.surface
}

fn matches_filter(values: &[String], actual: &str) -> bool {
    values.is_empty() || values.iter().any(|value| value == actual)
}

fn matches_prefix_filter(values: &[String], actual: &str) -> bool {
    values.is_empty() || values.iter().any(|value| actual.starts_with(value))
}

fn matches_any_filter(expected: &[String], actual: &[String]) -> bool {
    expected.is_empty() || expected.iter().any(|value| actual.contains(value))
}

fn crosses_hard_boundary(previous: &Morpheme, current: &Morpheme) -> bool {
    previous.pos.major == "記号"
        || current.pos.major == "記号"
        || previous.surface.contains('\n')
        || current.surface.contains('\n')
}

fn resolve_candidate_relations(candidates: &mut [Candidate]) {
    for index in 0..candidates.len() {
        if candidates[index].occurrence.status != GrammarOccurrenceStatus::Accepted {
            continue;
        }
        for other_index in 0..candidates.len() {
            if index == other_index
                || candidates[other_index].occurrence.status != GrammarOccurrenceStatus::Accepted
                || !ranges_overlap(
                    candidates[index].occurrence.anchor_range,
                    candidates[other_index].occurrence.anchor_range,
                )
            {
                continue;
            }
            if candidates[other_index]
                .refines_rule_ids
                .contains(&candidates[index].occurrence.rule_id)
                && candidates[other_index].priority > candidates[index].priority
            {
                candidates[index].occurrence.status = GrammarOccurrenceStatus::Rejected;
                candidates[index]
                    .occurrence
                    .counter_evidence
                    .push(format!("refined_by:{}", candidates[other_index].occurrence.rule_id));
                break;
            }
            if candidates[index].conflict_group.is_some()
                && candidates[index].conflict_group == candidates[other_index].conflict_group
                && candidates[other_index].priority > candidates[index].priority
            {
                candidates[index].occurrence.status = GrammarOccurrenceStatus::Pending;
                candidates[index]
                    .occurrence
                    .counter_evidence
                    .push(format!("conflicts_with:{}", candidates[other_index].occurrence.rule_id));
                break;
            }
        }
    }
}

fn deduplicate_candidates(catalog: &GrammarCatalog, candidates: Vec<Candidate>) -> Vec<Candidate> {
    let mut candidates = candidates;
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.priority));
    let mut indices = HashMap::new();
    let mut merged: Vec<Candidate> = Vec::new();

    for candidate in candidates {
        let key = (
            candidate.occurrence.concept_id.clone(),
            candidate.occurrence.anchor_range,
            occurrence_kind_key(&candidate.occurrence.kind),
            occurrence_status_key(&candidate.occurrence.status),
        );
        let Some(&index) = indices.get(&key) else {
            indices.insert(key, merged.len());
            merged.push(candidate);
            continue;
        };

        let target = &mut merged[index];
        let alternate_rule_id = candidate.occurrence.rule_id.clone();
        merge_unique(
            &mut target.occurrence.component_occurrence_ids,
            candidate.occurrence.component_occurrence_ids,
        );
        merge_unique(&mut target.occurrence.captures, candidate.occurrence.captures);
        merge_unique(
            &mut target.occurrence.evidence,
            candidate.occurrence.evidence,
        );
        merge_unique(
            &mut target.occurrence.counter_evidence,
            candidate.occurrence.counter_evidence,
        );
        merge_unique(
            &mut target.refines_rule_ids,
            candidate.refines_rule_ids,
        );
        for sense in candidate.occurrence.sense_candidates {
            if let Some(existing) = target
                .occurrence
                .sense_candidates
                .iter_mut()
                .find(|existing| existing.sense_id == sense.sense_id)
            {
                existing.confidence = existing.confidence.max(sense.confidence);
                merge_unique(&mut existing.evidence, sense.evidence);
            } else {
                target.occurrence.sense_candidates.push(sense);
            }
        }
        target.occurrence.sense_candidates.sort_by_key(|sense| {
            (std::cmp::Reverse(sense.confidence), sense.sense_id.clone())
        });
        target.occurrence.selected_sense_id =
            (target.occurrence.sense_candidates.len() == 1)
                .then(|| target.occurrence.sense_candidates[0].sense_id.clone());
        target.occurrence.explanation_ref = target
            .occurrence
            .selected_sense_id
            .as_deref()
            .and_then(|sense_id| catalog.sense(sense_id))
            .map(|sense| sense.explanation_id.clone())
            .or_else(|| {
                catalog
                    .concept(&target.occurrence.concept_id)
                    .map(|concept| concept.default_explanation_id.clone())
            })
            .unwrap_or_else(|| target.occurrence.explanation_ref.clone());
        target.occurrence.confidence = target
            .occurrence
            .confidence
            .max(candidate.occurrence.confidence);
        target.occurrence.show_badge |= candidate.occurrence.show_badge;
        if alternate_rule_id != target.occurrence.rule_id {
            let evidence = format!("alternate_rule:{alternate_rule_id}");
            if !target.occurrence.evidence.contains(&evidence) {
                target.occurrence.evidence.push(evidence);
            }
        }
    }

    merged
}

fn occurrence_kind_key(kind: &GrammarOccurrenceKind) -> u8 {
    match kind {
        GrammarOccurrenceKind::MorphologyFeature => 0,
        GrammarOccurrenceKind::FunctionalMorpheme => 1,
        GrammarOccurrenceKind::GrammarConstruction => 2,
        GrammarOccurrenceKind::BunsetsuFunction => 3,
        GrammarOccurrenceKind::CorrelativeGrammar => 4,
        GrammarOccurrenceKind::Unknown => 5,
    }
}

fn occurrence_status_key(status: &GrammarOccurrenceStatus) -> u8 {
    match status {
        GrammarOccurrenceStatus::Accepted => 0,
        GrammarOccurrenceStatus::Pending => 1,
        GrammarOccurrenceStatus::Rejected => 2,
        GrammarOccurrenceStatus::Unknown => 3,
    }
}

fn merge_unique<T: PartialEq>(target: &mut Vec<T>, incoming: Vec<T>) {
    for item in incoming {
        if !target.contains(&item) {
            target.push(item);
        }
    }
}

fn parse_kind(kind: &str) -> GrammarOccurrenceKind {
    match kind {
        "morphology_feature" => GrammarOccurrenceKind::MorphologyFeature,
        "functional_morpheme" => GrammarOccurrenceKind::FunctionalMorpheme,
        "grammar_construction" => GrammarOccurrenceKind::GrammarConstruction,
        "correlative_grammar" => GrammarOccurrenceKind::CorrelativeGrammar,
        _ => GrammarOccurrenceKind::Unknown,
    }
}

fn published_status(
    catalog: &GrammarCatalog,
    concept: &catalog::GrammarConcept,
    explanation_ref: &str,
) -> GrammarOccurrenceStatus {
    if concept.audit_status == "verified"
        && catalog
            .explanation(explanation_ref)
            .is_some_and(|explanation| explanation.authoring_status == "verified")
    {
        GrammarOccurrenceStatus::Accepted
    } else {
        GrammarOccurrenceStatus::Pending
    }
}

fn capture_from_range(
    name: &str,
    flat: &[FlatMorpheme],
    morpheme_range: (usize, usize),
) -> Vec<GrammarCapture> {
    flat.iter()
        .filter(|item| item.global_index >= morpheme_range.0 && item.global_index < morpheme_range.1)
        .map(|item| GrammarCapture {
            name: name.to_string(),
            surface: item.morpheme.surface.clone(),
            base_form: item.morpheme.base_form.clone(),
            morpheme_range: (item.global_index, item.global_index + 1),
            char_range: item.morpheme.char_range,
        })
        .collect()
}

fn occurrence_id(concept_id: &str, rule_id: &str, ranges: &[(usize, usize)]) -> String {
    let coordinates = ranges
        .iter()
        .map(|range| format!("{}-{}", range.0, range.1))
        .collect::<Vec<_>>()
        .join("+");
    format!("{}@{}#{}", concept_id, coordinates, rule_id)
}

fn merge_ranges(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    ranges.sort_by_key(|range| (range.0, range.1));
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.0 <= last.1 {
                last.1 = last.1.max(range.1);
                continue;
            }
        }
        merged.push(range);
    }
    merged
}

fn contains_range(container: (usize, usize), inner: (usize, usize)) -> bool {
    inner.0 >= container.0 && inner.1 <= container.1
}

fn ranges_overlap(left: (usize, usize), right: (usize, usize)) -> bool {
    left.0 < right.1 && right.0 < left.1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{HeadWord, PosTag};

    fn m(surface: &str, base: &str, major: &str, sub1: &str, form: &str, range: (usize, usize)) -> Morpheme {
        Morpheme {
            surface: surface.to_string(), base_form: base.to_string(), reading: String::new(),
            pos: PosTag { major: major.to_string(), sub1: sub1.to_string(), sub2: "*".to_string(), sub3: "*".to_string() },
            conjugation_type: String::new(), conjugation_form: form.to_string(), char_range: range,
        }
    }

    fn bunsetsu(morphemes: Vec<Morpheme>, range: (usize, usize)) -> Bunsetsu {
        let head = morphemes[0].clone();
        Bunsetsu {
            surface: morphemes.iter().map(|item| item.surface.as_str()).collect(),
            head_word: HeadWord { surface: head.surface, base_form: head.base_form, reading: head.reading, pos: head.pos },
            morphemes, grammar_tags: Vec::new(), morphology: MorphologyArtifact::default(), grammar_occurrences: Vec::new(), functional_residuals: Vec::new(),
            word_formations: Vec::new(), lexical_units: Vec::new(), function: None, char_range: range,
        }
    }

    #[test]
    fn request_refines_generic_benefactive() {
        let matcher = GrammarMatcher::new().unwrap();
        let mut bunsetsus = vec![bunsetsu(vec![
            m("使っ", "使う", "動詞", "自立", "連用タ接続", (0, 2)),
            m("て", "て", "助詞", "接続助詞", "*", (2, 3)),
            m("ください", "くださる", "動詞", "非自立", "命令ｉ", (3, 7)),
        ], (0, 7))];
        matcher.match_patterns(&mut bunsetsus);
        assert!(bunsetsus[0].grammar_occurrences.iter().any(|item| item.concept_id == "grammar.request.te_kudasai" && item.status == GrammarOccurrenceStatus::Accepted));
        assert!(bunsetsus[0].grammar_occurrences.iter().any(|item| item.concept_id == "grammar.benefactive.te_kudasaru" && item.status == GrammarOccurrenceStatus::Rejected));
        assert!(bunsetsus[0].grammar_tags.iter().any(|item| item.concept_id == "grammar.request.te_kudasai"));
    }
}
