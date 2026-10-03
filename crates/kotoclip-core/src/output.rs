use crate::dictionary::{lookup::DictionaryEngine, model::{DictEntry, DictionaryFormGroup, DictionaryLookupTiming, PosTag}};
use kotoclip_nlp::model::{MorphemeToken, QueryForm};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct QueryGroup {
    pub form: QueryForm,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    pub entries: Vec<DictEntry>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct QueryOutput {
    pub analysis_id: Option<String>,
    pub token_id: Option<String>,
    pub dictionary_names: Vec<String>,
    pub groups: Vec<QueryGroup>,
    #[serde(default)]
    pub forms: Vec<DictionaryFormGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_form_id: Option<String>,
    #[serde(default)]
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_form: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<DictionaryLookupTiming>,
}

pub fn query(
    dictionary: &DictionaryEngine,
    analysis_id: Option<String>,
    token: Option<&MorphemeToken>,
    forms: &[QueryForm],
    selected_form: Option<&str>,
) -> QueryOutput {
    let observed_form = token.map(|value| value.surface.clone());
    let pos = token.map(|value| PosTag {
        major: value.pos[0].clone().unwrap_or_else(|| "*".into()),
        sub1: value.pos[1].clone().unwrap_or_else(|| "*".into()),
        sub2: value.pos[2].clone().unwrap_or_else(|| "*".into()),
        sub3: value.pos[3].clone().unwrap_or_else(|| "*".into()),
    });
    let mut matrix_forms = Vec::new();
    let mut selected_form_id = None;
    let mut timing = None;
    let groups = forms
        .iter()
        .map(|form| {
            let lookup = dictionary.lookup_matrix_profiled(
                &form.form,
                observed_form.as_deref(),
                form.reading.as_deref(),
                pos.as_ref(),
                selected_form,
                &[],
            );
            if matrix_forms.is_empty() { matrix_forms = lookup.forms.clone(); }
            if selected_form_id.is_none() { selected_form_id = lookup.selected_form_id.clone(); }
            if timing.is_none() { timing = lookup.timing.clone(); }
            let total = lookup.entries.len();
            let entries = lookup.entries;
            QueryGroup {
                form: form.clone(),
                form_id: selected_form_id.clone(),
                entries,
                total,
            }
        })
        .collect();
    QueryOutput {
        analysis_id,
        token_id: token.map(|t| t.id.clone()),
        dictionary_names: dictionary.names(),
        groups,
        forms: matrix_forms,
        selected_form_id,
        mode: "contextual".into(),
        observed_form,
        reading: forms.iter().find_map(|form| form.reading.clone()),
        timing,
    }
}
