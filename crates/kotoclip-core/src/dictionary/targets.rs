use super::{lookup::DictionaryEngine, lookup_state, model::{DictEntry, DictionaryLookup, PosTag}};
use kotoclip_nlp::{model::{MorphemeToken, QueryForm, UnifiedDocument}, morphology::MorphologyRole};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScopeConstraint {
    pub preceding: Option<String>,
    pub following: Option<String>,
    pub surface: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentRole {
    pub surface: String,
    pub relative_range: [usize; 2],
    pub role: String,
    pub lookup_forms: Vec<QueryForm>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DictionaryMetadataHint {
    pub entry_id: String,
    pub headword: String,
    pub matched_forms: Vec<String>,
    pub readings: Vec<String>,
    pub pos_tags: Vec<String>,
    pub entry_kind: String,
    pub lexical_role: String,
    pub grammar_functions: Vec<String>,
    pub component_roles: Vec<ComponentRole>,
    pub scope_constraints: Vec<ScopeConstraint>,
    pub usage_tags: Vec<String>,
    pub compatibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixRequest {
    pub target_id: String,
    pub observed_form: String,
    pub lookup_forms: Vec<QueryForm>,
    pub reading_constraints: Vec<String>,
    pub pos_constraints: Option<PosTag>,
    pub metadata_entry_ids: Vec<String>,
    pub source_evidence: Vec<String>,
    pub dictionary_order: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupTarget {
    pub id: String,
    pub parent_outer_id: Option<String>,
    pub char_range: [usize; 2],
    pub surface: String,
    pub morpheme_ids: Vec<String>,
    pub lexical_core_ids: Vec<String>,
    pub source_formation_ids: Vec<String>,
    pub lookup_forms: Vec<QueryForm>,
    pub reading_evidence: Vec<String>,
    pub pos_evidence: Vec<String>,
    pub metadata_hints: Vec<DictionaryMetadataHint>,
    pub matrix_request: Option<MatrixRequest>,
    pub decision: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarTarget {
    pub char_range: [usize; 2],
    pub context_range: [usize; 2],
    pub normalized_form: String,
    pub concept_id: Option<String>,
    pub chain_id: Option<String>,
    pub display_form: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupTargetGroup {
    pub schema: String,
    pub source_revision: String,
    pub anchor_range: [usize; 2],
    pub outer_targets: Vec<LookupTarget>,
    pub inner_targets: Vec<LookupTarget>,
    pub candidate_targets: Vec<LookupTarget>,
    pub grammar_targets: Vec<GrammarTarget>,
    pub excluded_ranges: Vec<[usize; 2]>,
    pub coverage_status: String,
    pub default_target_id: Option<String>,
}

#[derive(Clone)]
struct Candidate {
    range: [usize; 2],
    forms: Vec<QueryForm>,
    members: Vec<usize>,
    cores: Vec<String>,
    formations: Vec<String>,
    pos: Option<PosTag>,
    minimal: bool,
    hints: Vec<DictionaryMetadataHint>,
    reason: String,
}

fn overlaps(left: [usize; 2], right: [usize; 2]) -> bool { left[0] < right[1] && right[0] < left[1] }
fn contains(outer: [usize; 2], inner: [usize; 2]) -> bool { outer[0] <= inner[0] && inner[1] <= outer[1] }
fn surface(chars: &[char], range: [usize; 2]) -> String { chars[range[0]..range[1]].iter().collect() }
fn pos(token: &MorphemeToken) -> PosTag {
    PosTag { major: token.pos[0].clone().unwrap_or_default(), sub1: token.pos[1].clone().unwrap_or_default(),
        sub2: token.pos[2].clone().unwrap_or_default(), sub3: token.pos[3].clone().unwrap_or_default() }
}
fn unique_forms(forms: impl IntoIterator<Item = QueryForm>) -> Vec<QueryForm> {
    let mut result = Vec::new();
    for form in forms { if !form.form.is_empty() && !result.iter().any(|item: &QueryForm| item.form == form.form && item.reading == form.reading) { result.push(form); } }
    result
}
fn form(value: String, reading: Option<String>, kind: &str) -> QueryForm {
    QueryForm { kind: kind.into(), form: value, reading_field: reading.as_ref().map(|_| "lookup_evidence".into()), reading }
}
fn lexical(hint: &DictionaryMetadataHint) -> bool {
    hint.lexical_role == "independent" && hint.compatibility != "incompatible" && hint.grammar_functions.is_empty()
}

fn projected_core_indices(chain: &kotoclip_nlp::morphology::MorphologyChain, tokens: &[MorphemeToken]) -> Vec<usize> {
    let mut members = chain.core_morpheme_indices.clone();
    if chain.lookup_form.ends_with("する") && members.len() > 1
        && chain.operators.iter().any(|operator| operator.kind == "causative"
            && tokens.get(operator.source_morpheme_range[0]).is_some_and(|token| token.lemma.as_deref() == Some("しめる"))) {
        if members.last().and_then(|index| tokens.get(*index)).is_some_and(|token| token.surface == "せ") {
            members.pop();
        }
    }
    members
}

fn token_forms(token: &MorphemeToken) -> Vec<QueryForm> {
    let inflected = token.query_forms.iter().any(|query| query.kind == "base" && query.form != token.surface);
    let mut forms = token.query_forms.iter().filter(|query| !inflected || query.kind != "observed")
        .filter(|query| inflected || query.reading.as_ref().zip(token.reading.as_ref()).is_none_or(|(reading, observed)|
            lookup_state::normalize_reading_identity(reading) == lookup_state::normalize_reading_identity(observed)))
        .cloned().collect::<Vec<_>>();
    forms.sort_by_key(|query| match query.kind.as_str() { "base" => 0, "lemma" => 1, _ => 2 });
    if forms.is_empty() { forms.push(form(token.surface.clone(), token.reading.clone(), "observed")); }
    unique_forms(forms)
}

pub fn metadata(entry: &DictEntry) -> DictionaryMetadataHint {
    let tags = entry.header.pos_tags.iter().map(|tag| tag.label.clone())
        .chain(entry.senses.iter().flat_map(|sense| sense.tags.iter().filter(|tag| tag.kind == "pos").map(|tag| tag.label.clone())))
        .collect::<Vec<_>>();
    let functional = tags.iter().any(|tag| tag.contains("助動") || tag.contains("助詞"));
    let component = tags.iter().any(|tag| tag.contains("接尾") || tag.contains("接頭"))
        || matches!(entry.entry_kind.as_str(), "prefix" | "suffix" | "bound_morpheme" | "kanji");
    let substantive = !matches!(entry.entry_kind.as_str(), "navigation" | "redirect")
        && (!entry.senses.is_empty() || !entry.sections.is_empty() || !entry.content_blocks.is_empty());
    DictionaryMetadataHint {
        entry_id: entry.occurrence_id.clone(), headword: entry.headword.clone(),
        matched_forms: std::iter::once(entry.header.display_form.clone()).chain(entry.header.canonical_form.clone())
            .chain(entry.header.scoped_forms.iter().map(|item| item.form.clone())).filter(|value| !value.is_empty()).collect(),
        readings: entry.reading.clone().into_iter().collect(), pos_tags: tags,
        entry_kind: entry.entry_kind.clone(),
        lexical_role: if functional { "functional" } else if component { "component" } else if substantive { "independent" } else { "unknown" }.into(),
        grammar_functions: Vec::new(), component_roles: Vec::new(), scope_constraints: Vec::new(),
        usage_tags: entry.header.usage_tags.iter().map(|tag| tag.label.clone()).collect(),
        compatibility: if entry.match_evidence.as_ref().is_some_and(|evidence| evidence.reading_match == "conflict") { "incompatible" } else { "compatible" }.into(),
    }
}

fn scope_matches(hint: &DictionaryMetadataHint, chars: &[char], range: [usize; 2]) -> bool {
    hint.scope_constraints.iter().all(|constraint| {
        constraint.surface.as_ref().is_none_or(|value| *value == surface(chars, range))
            && constraint.preceding.as_ref().is_none_or(|value| surface(chars, [0, range[0]]).ends_with(value))
            && constraint.following.as_ref().is_none_or(|value| surface(chars, [range[1], chars.len()]).starts_with(value))
    })
}

pub fn build(
    dictionary: &DictionaryEngine,
    document: &UnifiedDocument,
    anchor_range: [usize; 2],
    supplied_metadata: &[DictionaryMetadataHint],
) -> Result<LookupTargetGroup, String> {
    let chars = document.text.chars().collect::<Vec<_>>();
    if anchor_range[0] >= anchor_range[1] || anchor_range[1] > chars.len() { return Err("查词范围超出正文".into()); }
    let tokens = &document.morphemes;
    let lexical_chains = document.morphology.chains.iter().filter(|chain| chain.role == MorphologyRole::Lexical).collect::<Vec<_>>();
    let root_cores = lexical_chains.iter().flat_map(|chain| projected_core_indices(chain, tokens)).collect::<HashSet<_>>();
    let mut functional_members = document.morphology.chains.iter().flat_map(|chain| chain.morpheme_indices.iter().copied()
        .filter(|index| chain.role == MorphologyRole::Functional || !chain.core_morpheme_indices.contains(index)))
        .filter(|index| !root_cores.contains(index)).collect::<HashSet<_>>();
    for chain in &lexical_chains {
        let projected = projected_core_indices(chain, tokens).into_iter().collect::<HashSet<_>>();
        for index in &chain.core_morpheme_indices {
            if !projected.contains(index) { functional_members.insert(*index); }
        }
        for operator in &chain.operators {
            if operator.kind == "te_connection" {
                for index in &chain.morpheme_indices {
                    if tokens.get(*index).is_some_and(|token| overlaps(token.char_range, operator.char_range)) { functional_members.insert(*index); }
                }
            }
        }
    }
    let mut excluded = Vec::new();
    let mut grammar = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if !overlaps(anchor_range, token.char_range) { continue; }
        let functional = functional_members.contains(&index) || (matches!(token.pos[0].as_deref(), Some("助詞" | "助動詞")) && !root_cores.contains(&index));
        if functional || matches!(token.pos[0].as_deref(), Some("記号" | "補助記号" | "空白")) || token.surface.chars().any(char::is_whitespace) {
            excluded.push(token.char_range);
            if functional {
                let chain = document.morphology.chains.iter().find(|chain| chain.morpheme_indices.contains(&index));
                grammar.push(GrammarTarget { char_range: token.char_range, context_range: chain.map_or(token.char_range, |chain| chain.char_range),
                    normalized_form: token.lemma.clone().unwrap_or_else(|| token.surface.clone()), concept_id: None,
                    chain_id: chain.map(|chain| chain.chain_id.clone()), display_form: token.surface.clone() });
            }
        }
    }
    for chain in &document.morphology.chains {
        for operator in &chain.operators {
            if !overlaps(anchor_range, operator.char_range) || matches!(operator.kind.as_str(), "conjugation" | "initial_alternation" | "final_alternation") { continue; }
            grammar.push(GrammarTarget { char_range: operator.char_range, context_range: chain.char_range,
                normalized_form: operator.normalized_form.clone().unwrap_or_else(|| operator.kind.clone()),
                concept_id: Some(operator.concept_id.clone()), chain_id: Some(chain.chain_id.clone()), display_form: operator.label.clone() });
        }
    }
    let valid = |range| contains(anchor_range, range) && !excluded.iter().any(|excluded| overlaps(*excluded, range));
    let mut candidates: BTreeMap<[usize; 2], Candidate> = BTreeMap::new();
    for (index, token) in tokens.iter().enumerate().filter(|(_, token)| valid(token.char_range)) {
        let forms = token_forms(token);
        candidates.insert(token.char_range, Candidate { range: token.char_range, forms, members: vec![index], cores: Vec::new(),
            formations: Vec::new(), pos: Some(pos(token)), minimal: true, hints: Vec::new(), reason: String::new() });
    }
    for chain in document.morphology.chains.iter().filter(|chain| chain.role == MorphologyRole::Lexical) {
        let projected_members = projected_core_indices(chain, tokens);
        let members = &projected_members;
        let Some(first) = members.first().and_then(|index| tokens.get(*index)) else { continue; };
        let Some(last) = members.last().and_then(|index| tokens.get(*index)) else { continue; };
        let range = [first.char_range[0], last.char_range[1]];
        if !valid(range) || members.windows(2).any(|pair| tokens[pair[0]].char_range[1] != tokens[pair[1]].char_range[0]) { continue; }
        let canonical = chain.query_forms.iter().find(|query| query.form == chain.lookup_form).cloned()
            .unwrap_or_else(|| form(chain.lookup_form.clone(), None, "canonical"));
        let mut forms = if members.len() == 1 && chain.core_morpheme_indices.len() == 1 { token_forms(first) } else { vec![canonical] };
        if members.len() > 1 || chain.core_morpheme_indices.len() > 1 {
            forms.extend(chain.query_forms.iter().filter(|query| query.kind != "observed"
                && !query.form.contains("為る") && query.form != surface(&chars, range)).cloned());
        }
        if chain.lookup_form.ends_with("する") && first.pos[0].as_deref() == Some("名詞") {
            forms.extend(token_forms(first));
        }
        let candidate = candidates.entry(range).or_insert_with(|| Candidate { range, forms: Vec::new(), members: members.clone(), cores: Vec::new(),
            formations: Vec::new(), pos: Some(pos(first)), minimal: chain.status == "resolved" && chain.lookup_form != surface(&chars, range), hints: Vec::new(), reason: String::new() });
        candidate.forms = unique_forms(forms);
        candidate.cores.push(chain.chain_id.clone());
        if chain.lookup_form.ends_with("する") || first.pos[0].as_deref() == Some("動詞") { candidate.pos.as_mut().unwrap().major = "動詞".into(); }
    }
    let atoms = candidates.values().filter(|candidate| candidate.members.len() == 1).cloned().collect::<Vec<_>>();
    for start in 0..atoms.len() {
        for end in start + 1..atoms.len() {
            if atoms[end - 1].range[1] != atoms[end].range[0] { break; }
            let range = [atoms[start].range[0], atoms[end].range[1]];
            let mut forms = Vec::new();
            if !tokens[atoms[end].members[0]].query_forms.iter().any(|query| query.kind == "base" && query.form != tokens[atoms[end].members[0]].surface) {
                forms.push(form(surface(&chars, range), atoms[start..=end].iter().map(|atom| tokens[atom.members[0]].reading.clone())
                    .collect::<Option<Vec<_>>>().map(|parts| parts.join("")), "sequence"));
            }
            let prefix = surface(&chars, [range[0], atoms[end].range[0]]);
            let prefix_reading = atoms[start..end].iter().map(|atom| tokens[atom.members[0]].reading.clone()).collect::<Option<Vec<_>>>().map(|parts| parts.join(""));
            forms.extend(atoms[end].forms.iter().map(|query| form(format!("{prefix}{}", query.form),
                prefix_reading.as_ref().zip(query.reading.as_ref()).map(|(prefix, reading)| format!("{prefix}{reading}")), "sequence_base")));
            candidates.entry(range).or_insert_with(|| Candidate { range, forms: unique_forms(forms), members: atoms[start..=end].iter().flat_map(|atom| atom.members.clone()).collect(),
                cores: Vec::new(), formations: Vec::new(), pos: atoms[end].pos.clone(), minimal: false, hints: Vec::new(), reason: String::new() });
        }
    }
    for node in &document.formation.nodes {
        if let Some(candidate) = candidates.get_mut(&node.char_range) {
            candidate.formations.push(node.id.clone());
        }
    }
    let words = candidates.values().flat_map(|candidate| candidate.forms.iter().map(|query| query.form.clone())).collect::<HashSet<_>>();
    let exact = dictionary.contains_exact_batch(&words);
    for candidate in candidates.values_mut() {
        let mut seen = HashSet::new();
        for query in &candidate.forms {
            if !exact.contains(&query.form) && !candidate.minimal { continue; }
            let entries = dictionary.lookup_profiled_with_pos(&query.form, query.reading.as_deref(), candidate.pos.as_ref()).0;
            for entry in entries {
                let hint = supplied_metadata.iter().find(|hint| hint.entry_id == entry.occurrence_id).cloned().unwrap_or_else(|| metadata(&entry));
                if !candidate.minimal && !lookup_state::entry_matches_form(&entry, &query.form) { continue; }
                if hint.compatibility == "incompatible" { continue; }
                if !seen.insert(entry.occurrence_id.clone()) { continue; }
                if scope_matches(&hint, &chars, candidate.range) { candidate.hints.push(hint); }
            }
        }
        candidate.hints.sort_by_key(|hint| (!lexical(hint), hint.compatibility != "compatible", hint.entry_kind != "proper_name", !hint.usage_tags.is_empty()));
        candidate.reason = if candidate.hints.iter().any(lexical) { "dictionary_lexical_entry" }
            else if candidate.hints.iter().any(|hint| hint.compatibility == "incompatible") { "incompatible" }
            else if !candidate.hints.is_empty() { "nonlexical_entry" } else { "no_entry" }.into();
    }
    let candidates = candidates.into_values().collect::<Vec<_>>();
    let mut outer = Vec::new();
    let mut cursor = anchor_range[0];
    while cursor < anchor_range[1] {
        let next = candidates.iter().filter(|candidate| candidate.range[0] == cursor && candidate.hints.iter().any(lexical))
            .max_by_key(|candidate| (candidate.range[1], !candidate.cores.is_empty(), !candidate.formations.is_empty()))
            .or_else(|| candidates.iter().filter(|candidate| candidate.range[0] == cursor && candidate.minimal)
                .max_by_key(|candidate| (!candidate.cores.is_empty(), candidate.range[1])));
        if let Some(candidate) = next {
            outer.push(target(candidate, &chars, tokens, None, if candidate.hints.iter().any(lexical) { "accepted" } else { "fallback" }));
            cursor = candidate.range[1];
        } else { cursor += 1; }
    }
    let mut inner = Vec::new();
    for parent in &outer {
        for candidate in candidates.iter().filter(|candidate| candidate.range != parent.char_range && contains(parent.char_range, candidate.range)) {
            if candidate.members.len() > 1 && !candidate.hints.iter().any(lexical) { continue; }
            let component = candidate.pos.as_ref().is_some_and(|pos| matches!(pos.major.as_str(), "接頭辞" | "接尾辞"))
                || candidate.members.iter().all(|index| tokens[*index].lemma.as_deref() == Some("為る")) && !parent.lexical_core_ids.is_empty()
                || !candidate.hints.iter().any(lexical);
            inner.push(target(candidate, &chars, tokens, Some(&parent.id), if component { "component" } else { "queryable" }));
        }
        for hint in &parent.metadata_hints {
            for component in &hint.component_roles {
                let range = [parent.char_range[0] + component.relative_range[0], parent.char_range[0] + component.relative_range[1]];
                if range[0] >= range[1] || !contains(parent.char_range, range) || !valid(range) || surface(&chars, range) != component.surface { continue; }
                if inner.iter().any(|target| target.parent_outer_id.as_ref() == Some(&parent.id) && target.char_range == range) { continue; }
                let candidate = Candidate { range, forms: component.lookup_forms.clone(), members: tokens.iter().enumerate().filter(|(_, token)| overlaps(range, token.char_range)).map(|(index, _)| index).collect(),
                    cores: Vec::new(), formations: Vec::new(), pos: None, minimal: false, hints: vec![hint.clone()], reason: "dictionary_component".into() };
                inner.push(target(&candidate, &chars, tokens, Some(&parent.id), if component.role == "independent" { "queryable" } else { "component" }));
            }
        }
    }
    let pending = candidates.iter().filter(|candidate| !candidate.formations.is_empty() && !candidate.hints.iter().any(lexical))
        .map(|candidate| target(candidate, &chars, tokens, None, if candidate.reason == "no_entry" { "pending" } else { "rejected" })).collect();
    let default_target_id = outer.iter().max_by_key(|target| (target.decision == "accepted", target.char_range[1] - target.char_range[0], std::cmp::Reverse(target.char_range[0])))
        .map(|target| target.id.clone());
    Ok(LookupTargetGroup { schema: "kotoclip.lookup-targets.v1".into(), source_revision: kotoclip_nlp::external::text_digest(
        &serde_json::to_string(&(&document.id, &document.morphology, &document.formation)).map_err(|error| error.to_string())?),
        anchor_range, outer_targets: outer, inner_targets: inner, candidate_targets: pending, grammar_targets: grammar, excluded_ranges: excluded,
        coverage_status: "complete".into(), default_target_id })
}

fn target(candidate: &Candidate, chars: &[char], tokens: &[MorphemeToken], parent: Option<&String>, decision: &str) -> LookupTarget {
    let id = format!("lookup:{}:{}:{}", candidate.range[0], candidate.range[1], if parent.is_some() { "inner" } else { "outer" });
    let observed = surface(chars, candidate.range);
    let readings = candidate.forms.iter().filter_map(|query| query.reading.clone()).collect::<HashSet<_>>();
    let mut readings = readings.into_iter().collect::<Vec<_>>();
    readings.sort();
    let sources = candidate.cores.iter().chain(&candidate.formations).cloned().chain(candidate.members.iter().map(|index| tokens[*index].id.clone())).collect();
    LookupTarget { id: id.clone(), parent_outer_id: parent.cloned(), char_range: candidate.range, surface: observed.clone(),
        morpheme_ids: candidate.members.iter().map(|index| tokens[*index].id.clone()).collect(), lexical_core_ids: candidate.cores.clone(),
        source_formation_ids: candidate.formations.clone(), lookup_forms: candidate.forms.clone(), reading_evidence: readings.clone(),
        pos_evidence: candidate.pos.as_ref().map(|pos| vec![pos.major.clone(), pos.sub1.clone()]).unwrap_or_default(), metadata_hints: candidate.hints.clone(),
        matrix_request: (!matches!(decision, "component" | "pending" | "rejected")).then(|| MatrixRequest { target_id: id, observed_form: observed,
            lookup_forms: candidate.forms.clone(), reading_constraints: readings, pos_constraints: candidate.pos.clone(),
            metadata_entry_ids: candidate.hints.iter().filter(|hint| lexical(hint)).map(|hint| hint.entry_id.clone()).collect(), source_evidence: sources, dictionary_order: Vec::new() }),
        decision: decision.into(), reason: candidate.reason.clone() }
}

pub fn query(dictionary: &DictionaryEngine, request: &MatrixRequest, selected_form: Option<&str>) -> DictionaryLookup {
    let mut merged = DictionaryLookup { query: request.lookup_forms.first().map(|form| form.form.clone()).unwrap_or_default(), observed_form: Some(request.observed_form.clone()),
        reading: request.lookup_forms.first().and_then(|form| form.reading.clone()), pos: request.pos_constraints.clone(), selected_form_id: None,
        mode: "lookup_target".into(), forms: Vec::new(), dictionary_names: dictionary.names(), entries: Vec::new(), timing: None };
    let mut entries = Vec::new();
    for form in &request.lookup_forms {
        let matrix = dictionary.lookup_matrix_profiled(&form.form, Some(&request.observed_form), form.reading.as_deref(), request.pos_constraints.as_ref(), selected_form, &request.dictionary_order);
        if merged.selected_form_id.is_none() && !matrix.entries.is_empty() {
            merged.selected_form_id = matrix.forms.iter().find(|group| group.normalized_form == lookup_state::normalize_form_identity(&form.form))
                .map(|group| group.form_id.clone()).or(matrix.selected_form_id.clone());
            merged.reading = form.reading.clone();
        }
        for group in matrix.forms {
            if let Some(existing) = merged.forms.iter_mut().find(|existing| existing.form_id == group.form_id) {
                for reading in group.readings { if !existing.readings.contains(&reading) { existing.readings.push(reading); } }
                for variant in group.variants { if !existing.variants.contains(&variant) { existing.variants.push(variant); } }
                for availability in group.dictionaries { if let Some(current) = existing.dictionaries.iter_mut().find(|current| current.dictionary_name == availability.dictionary_name) { current.available |= availability.available; } }
            } else { merged.forms.push(group); }
        }
        for entry in matrix.entries {
            if (request.metadata_entry_ids.is_empty() || request.metadata_entry_ids.contains(&entry.occurrence_id))
                && !entries.iter().any(|existing: &DictEntry| existing.occurrence_id == entry.occurrence_id) { entries.push(entry); }
        }
    }
    if let Some(selected) = selected_form { merged.selected_form_id = merged.forms.iter().find(|form| form.form_id == selected || form.display_form == selected).map(|form| form.form_id.clone()); }
    if let Some(active) = merged.forms.iter().find(|form| Some(&form.form_id) == merged.selected_form_id.as_ref()) {
        for variant in &active.variants {
            for entry in dictionary.lookup_exact_form_profiled_with_pos(&variant.surface_form, None, request.pos_constraints.as_ref()).0 {
                if !entries.iter().any(|existing| existing.occurrence_id == entry.occurrence_id)
                    && request.lookup_forms.iter().any(|form| lookup_state::entry_matches_reading(&entry, form.reading.as_deref()))
                    && (request.metadata_entry_ids.is_empty() || request.metadata_entry_ids.contains(&entry.occurrence_id)) {
                    entries.push(entry);
                }
            }
        }
        merged.entries = entries.into_iter().filter(|entry| lookup_state::entry_matches_form(entry, &active.display_form)).collect();
    }
    merged
}

pub fn report(dictionary: &DictionaryEngine, group: &LookupTargetGroup) -> Result<serde_json::Value, String> {
    let mut result = serde_json::to_value(group).map_err(|error| error.to_string())?;
    for (row, target) in result["outer_targets"].as_array_mut().ok_or("查询目标序列化失败")?.iter_mut().zip(&group.outer_targets) {
        let request = target.matrix_request.as_ref().ok_or("外层目标缺少矩阵请求")?;
        let matrix = query(dictionary, request, None);
        let mut entries = matrix.entries.clone();
        for form in &request.lookup_forms {
            for entry in dictionary.lookup_profiled_with_pos(&form.form, form.reading.as_deref(), request.pos_constraints.as_ref()).0 {
                if lookup_state::entry_matches_reading(&entry, form.reading.as_deref())
                    && request.metadata_entry_ids.contains(&entry.occurrence_id)
                    && !entries.iter().any(|existing| existing.occurrence_id == entry.occurrence_id) { entries.push(entry); }
            }
        }
        row["entries"] = serde_json::to_value(entries).map_err(|error| error.to_string())?;
        row["selected_form_id"] = serde_json::json!(matrix.selected_form_id);
    }
    Ok(result)
}
