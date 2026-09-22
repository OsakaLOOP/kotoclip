//! 统一词元、结构证据和语言规则的应用层分析。
use kotoclip_nlp::{
    application::{ApplicationArtifact, Explanation, LexicalDecision, ReadingUnit},
    expression::{ExpressionOccurrence, ExpressionStatus},
    grammar::{GrammarOccurrence, GrammarStatus},
    model::UnifiedDocument,
    rules::{self, Rule, RuleMatch},
};
use std::collections::{BTreeMap, BTreeSet};

pub const VERSION: &str = "kotoclip.language-application.v1";

pub fn apply(document: &mut UnifiedDocument, user_rules: &[Rule], rules_version: u64) -> Result<(), String> {
    let lexical = lexical_decisions(document);
    let mut explanations = morphology_explanations(document);
    explanations.extend(structure_explanations(document));

    let mut grammar_rules = crate::language_rules::grammar()?.to_vec();
    let mut expression_rules = crate::language_rules::expressions()?;
    for rule in user_rules.iter().filter(|rule| rule.document_id.as_deref().is_none_or(|id| id == document.id)) {
        if matches!(rule.kind.as_str(), "functional_morpheme" | "morphology_feature") || rule.concept_id.is_some() {
            grammar_rules.push(rule.clone());
        } else {
            expression_rules.push(rule.clone());
        }
    }
    grammar_rules.sort_by_key(|rule| std::cmp::Reverse(rule.priority));
    expression_rules.sort_by_key(|rule| std::cmp::Reverse(rule.priority));

    for rule in &grammar_rules {
        for found in find(document, rule)? {
            let status = if rule.sense_ids.len() <= 1 { GrammarStatus::Observed } else { GrammarStatus::Candidate };
            let id = occurrence_id("grammar", rule, &found);
            document.grammar.occurrences.push(GrammarOccurrence { id: id.clone(), concept_id: rule.concept_id.clone(),
                char_range: found.char_range, morpheme_indices: found.members.clone(), status,
                provider: if rule.id.starts_with("user.") { "user-rule".into() } else { "compiled-grammar-catalog".into() },
                source_id: Some(rule.id.clone()), labels: vec![rule.label.clone()], evidence: rule_evidence(document, rule, &found) });
            explanations.push(rule_explanation(document, "grammar", rule, &found, &id));
        }
    }
    for rule in &expression_rules {
        for found in find(document, rule)? {
            let id = occurrence_id("expression", rule, &found);
            document.expression.occurrences.push(ExpressionOccurrence { id: id.clone(), rule_id: Some(rule.id.clone()),
                char_range: found.char_range, morpheme_indices: found.members.clone(), status: ExpressionStatus::Observed,
                provider: if rule.id.starts_with("user.") { "user-rule".into() } else { "builtin-expression-catalog".into() },
                source_id: Some(rule.id.clone()), expression_type: rule.kind.clone(), labels: vec![rule.label.clone()],
                evidence: rule_evidence(document, rule, &found) });
            explanations.push(rule_explanation(document, "expression", rule, &found, &id));
        }
    }
    deduplicate_language_layers(document);
    let reading_units = reading_units(document, &lexical);
    document.application = ApplicationArtifact { version: VERSION.into(), rules_version, lexical, reading_units, explanations };
    document.projection = kotoclip_nlp::projection::from_layers(&document.grammar, &document.expression);
    for explanation in &document.application.explanations {
        document.projection.targets.push(kotoclip_nlp::projection::ProjectionTarget { id: format!("explanation:{}", explanation.id),
            char_range: explanation.char_range, layer: explanation.layer.clone(), source_id: explanation.source_id.clone(), status: explanation.status.clone() });
    }
    Ok(())
}

pub fn bind_lexical(document: &mut UnifiedDocument, dictionary: &crate::dictionary::lookup::DictionaryEngine) {
    for decision in &mut document.application.lexical {
        let Some(candidate) = document.dictionary_candidates.candidates.iter().find(|candidate| decision.candidate_ids.contains(&candidate.source_id)) else { continue; };
        let reading = candidate.query_forms.iter().find_map(|form| form.reading.as_deref());
        decision.bindings = lookup_bindings(dictionary, &candidate.surface, reading);
    }
    let bound_candidates = document.application.lexical.iter().flat_map(|decision| decision.candidate_ids.iter().map(move |id| (id.clone(), !decision.bindings.is_empty())))
        .collect::<BTreeMap<_, _>>();
    for decision in &mut document.application.lexical {
        let competing_bound = decision.competing_ids.iter().any(|candidate_id| bound_candidates.get(candidate_id) == Some(&true));
        match (!decision.bindings.is_empty(), competing_bound) {
            (true, false) => { decision.status = "observed".into(); decision.reason = "词典整体记录已绑定".into(); }
            (true, true) => { decision.status = "pending".into(); decision.reason = "重叠候选均有词典记录，等待竞争决定".into(); }
            (false, _) => { decision.status = "candidate".into(); decision.reason = "来源构词范围完整，词典没有对应整体记录".into(); }
        }
    }
}

pub fn lookup_bindings(dictionary: &crate::dictionary::lookup::DictionaryEngine, surface: &str, reading: Option<&str>) -> Vec<kotoclip_nlp::application::DictionaryBinding> {
    let mut seen = BTreeSet::new();
    dictionary.lookup(surface, reading).into_iter().filter(|entry| seen.insert((entry.dict_name.clone(), entry.occurrence_id.clone()))).map(|entry| {
        kotoclip_nlp::application::DictionaryBinding { dictionary: entry.dict_name, entry_key: entry.entry_key, occurrence_id: entry.occurrence_id,
            headword: entry.headword, reading: entry.reading.unwrap_or_default() }
    }).collect()
}

pub fn preview(document: &UnifiedDocument, rule: &Rule) -> Result<Vec<RuleMatch>, String> {
    find(document, rule)
}

fn find(document: &UnifiedDocument, rule: &Rule) -> Result<Vec<RuleMatch>, String> {
    let found = rules::matches(rule, &document.text, &document.morphemes, &document.source.tokens)?;
    Ok(found.into_iter().filter(|item| rules::respects_bunsetsu_gap(rule, item, &document.bunsetsu)).collect())
}

fn occurrence_id(layer: &str, rule: &Rule, found: &RuleMatch) -> String {
    format!("{layer}:{}:{}:{}", rule.id, found.char_range[0], found.char_range[1])
}

fn lexical_decisions(document: &UnifiedDocument) -> Vec<LexicalDecision> {
    let mut grouped = BTreeMap::<[usize; 2], Vec<_>>::new();
    for node in &document.formation.nodes { grouped.entry(node.char_range).or_default().push(node); }
    grouped.into_iter().map(|(char_range, nodes)| {
        let candidate_ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
        let members = nodes.iter().flat_map(|node| node.morpheme_indices.iter().copied()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
        let competing_ids = document.formation.conflicts.iter().filter(|conflict| conflict.node_ids.iter().any(|id| candidate_ids.contains(id)))
            .flat_map(|conflict| conflict.node_ids.iter().filter(|id| !candidate_ids.contains(id)).cloned()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
        let status = if competing_ids.is_empty() { "candidate" } else { "pending" };
        let reason = if competing_ids.is_empty() { "来源构词范围完整，等待词典整体绑定" } else { "构词范围与其他候选重叠，等待竞争决定" };
        LexicalDecision { id: format!("lexical:{}:{}", char_range[0], char_range[1]), char_range, members, candidate_ids,
            status: status.into(), reason: reason.into(), bindings: Vec::new(), competing_ids }
    }).collect()
}

fn reading_units(document: &UnifiedDocument, lexical: &[LexicalDecision]) -> Vec<ReadingUnit> {
    let mut consumed = BTreeSet::new();
    let mut units = Vec::new();
    for index in 0..document.morphemes.len() {
        if consumed.contains(&index) { continue; }
        let lexical_item = lexical.iter().filter(|item| item.members.first() == Some(&index))
            .max_by_key(|item| item.members.len());
        let members = lexical_item.map(|item| item.members.clone()).unwrap_or_else(|| vec![index]);
        consumed.extend(members.iter().copied());
        let char_range = [document.morphemes[*members.first().unwrap()].char_range[0], document.morphemes[*members.last().unwrap()].char_range[1]];
        let chain_ids = document.morphology.chains.iter().filter(|chain| members.iter().any(|index| *index >= chain.morpheme_range[0] && *index < chain.morpheme_range[1]))
            .map(|chain| chain.chain_id.clone()).collect();
        let bunsetsu_ids = document.bunsetsu.nodes.iter().filter(|node| node.morpheme_indices.iter().any(|index| members.contains(index))).map(|node| node.id.clone()).collect();
        let query_target_ids = document.dictionary_candidates.candidates.iter().filter(|candidate| candidate.morpheme_indices.iter().all(|index| members.contains(index)))
            .map(|candidate| candidate.id.clone()).collect();
        units.push(ReadingUnit { id: format!("reading:{}:{}", char_range[0], char_range[1]), char_range, members,
            lexical_id: lexical_item.map(|item| item.id.clone()), chain_ids, bunsetsu_ids, query_target_ids });
    }
    units
}

fn morphology_explanations(document: &UnifiedDocument) -> Vec<Explanation> {
    document.morphology.chains.iter().map(|chain| Explanation { id: format!("explanation:{}", chain.chain_id), layer: "morphology".into(),
        source_id: chain.chain_id.clone(), char_range: chain.char_range, hit_ranges: chain.source_ranges.clone(),
        members: (chain.morpheme_range[0]..chain.morpheme_range[1]).collect(), captures: BTreeMap::new(), chain_ids: vec![chain.chain_id.clone()],
        concept_id: Some("morphology.chain".into()), sense_id: None, sense_candidates: Vec::new(), status: "observed".into(),
        reason: "UniDic 活用字段与连接形".into(), title: chain.display_form.clone(),
        summary: if chain.operators.is_empty() { format!("辞书形：{}", chain.dictionary_form) } else { chain.operators.iter().map(|item| item.label.as_str()).collect::<Vec<_>>().join("、") },
        evidence: chain.evidence.clone() }).collect()
}

fn structure_explanations(document: &UnifiedDocument) -> Vec<Explanation> {
    document.bunsetsu.nodes.iter().map(|node| Explanation { id: format!("explanation:{}", node.id), layer: "structure".into(), source_id: node.id.clone(),
        char_range: node.char_range, hit_ranges: vec![node.char_range], members: node.morpheme_indices.clone(), captures: BTreeMap::new(), chain_ids: Vec::new(),
        concept_id: None, sense_id: None, sense_candidates: Vec::new(), status: format!("{:?}", node.status).to_lowercase(), reason: format!("{} 文节范围", node.provider),
        title: "文节".into(), summary: node.labels.join("、"), evidence: node.source_id.iter().cloned().collect() }).collect()
}

fn rule_explanation(document: &UnifiedDocument, layer: &str, rule: &Rule, found: &RuleMatch, source_id: &str) -> Explanation {
    let chain_ids = document.morphology.chains.iter().filter(|chain| found.members.iter().any(|member| *member >= chain.morpheme_range[0] && *member < chain.morpheme_range[1]))
        .map(|chain| chain.chain_id.clone()).collect();
    let (title, summary) = rule.concept_id.as_deref().and_then(|id| crate::grammar_catalog::get(id).ok())
        .map(|bundle| (bundle.explanation.title, bundle.explanation.compact_summary))
        .unwrap_or_else(|| (rule.label.clone(), rule.description.clone()));
    Explanation { id: format!("explanation:{source_id}"), layer: layer.into(), source_id: source_id.into(), char_range: found.char_range,
        hit_ranges: found.hit_ranges.clone(), members: found.members.clone(), captures: found.captures.clone(), chain_ids,
        concept_id: rule.concept_id.clone(), sense_id: (rule.sense_ids.len() == 1).then(|| rule.sense_ids[0].clone()), sense_candidates: rule.sense_ids.clone(),
        status: if rule.sense_ids.len() <= 1 { "observed".into() } else { "pending".into() },
        reason: if rule.sense_ids.len() <= 1 { "规则条件与正文词形一致".into() } else { "规则成立，语义分支需要上下文决定".into() },
        title, summary, evidence: rule_evidence(document, rule, found) }
}

fn rule_evidence(document: &UnifiedDocument, rule: &Rule, found: &RuleMatch) -> Vec<String> {
    let mut evidence = vec![format!("rule:{}", rule.id)];
    evidence.extend(rule.source_refs.clone());
    for entity in document.structure_graph.entities.iter().filter(|entity| entity.kind == kotoclip_nlp::external::NodeKind::BasicPhrase
        && entity.coverage.iter().any(|coverage| coverage.morpheme_indices.iter().any(|index| found.members.contains(index)))) {
        evidence.push(format!("{}:basic_phrase:{}", entity.provider, entity.source_id));
        if entity.provider == "kwja" {
            if let Some(object) = entity.features.as_object() {
                for (key, value) in object.iter().filter(|(key, _)| key.contains("type") || key.contains("label") || key.contains("feature")) {
                    evidence.push(format!("kwja:{key}:{value}"));
                }
            }
        }
    }
    evidence
}

fn deduplicate_language_layers(document: &mut UnifiedDocument) {
    let mut grammar = BTreeSet::new();
    document.grammar.occurrences.retain(|item| grammar.insert((item.id.clone(), item.char_range)));
    let mut expressions = BTreeSet::new();
    document.expression.occurrences.retain(|item| expressions.insert((item.rule_id.clone(), item.char_range, item.morpheme_indices.clone())));
}
