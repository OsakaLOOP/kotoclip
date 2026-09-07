use crate::dictionary::{lookup::DictionaryEngine, model::DictEntry};
use kotoclip_nlp::model::{MorphemeToken, QueryForm};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize)]
pub struct QueryGroup {
    pub form: QueryForm,
    pub entries: Vec<DictEntry>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct QueryOutput {
    pub analysis_id: Option<String>,
    pub token_id: Option<String>,
    pub dictionary_names: Vec<String>,
    pub groups: Vec<QueryGroup>,
}

pub fn query(
    dictionary: &DictionaryEngine,
    analysis_id: Option<String>,
    token: Option<&MorphemeToken>,
    forms: &[QueryForm],
) -> QueryOutput {
    let groups = forms
        .iter()
        .map(|form| {
            // 读音用于排序，精确表记结果保留供用户核对。
            let mut entries = dictionary.lookup(&form.form, None);
            let mut seen = HashSet::new();
            entries.retain(|e| {
                seen.insert((
                    e.dict_name.clone(),
                    e.entry_key.clone(),
                    e.occurrence_id.clone(),
                ))
            });
            let normalize = |s: &str| {
                s.chars()
                    .map(|c| {
                        if ('ぁ'..='ゖ').contains(&c) {
                            char::from_u32(c as u32 + 0x60).unwrap_or(c)
                        } else {
                            c
                        }
                    })
                    .collect::<String>()
            };
            if let Some(reading) = &form.reading {
                entries.sort_by_key(|e| {
                    e.reading.as_ref().map(|r| normalize(r)) != Some(normalize(reading))
                });
            }
            let total = entries.len();
            entries.truncate(60);
            QueryGroup {
                form: form.clone(),
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
    }
}
