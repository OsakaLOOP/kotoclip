use crate::model::*;
use sha2::{Digest, Sha256};

pub fn unify(text: &str, source: SourceAnalysis) -> Result<UnifiedDocument, String> {
    let chars: Vec<char> = text.chars().collect();
    let id = format!(
        "{:x}",
        Sha256::digest(format!(
            "{SCHEMA}\0{}\0{text}",
            source.provider.dictionary_sha256
        ))
    );
    let mut gaps = Vec::new();
    let mut morphemes = Vec::new();
    let mut end = 0;
    for token in &source.tokens {
        let [start, next] = token.char_range;
        if start < end
            || next <= start
            || next > chars.len()
            || chars[start..next].iter().collect::<String>() != token.surface
        {
            return Err(format!("来源 token {} 的字符范围与原文不符", token.index));
        }
        if start > end {
            gaps.push(TextGap {
                char_range: [end, start],
                surface: chars[end..start].iter().collect(),
            });
        }
        let value = |index: usize| token.fields[index].value.clone();
        let mut forms: Vec<QueryForm> = Vec::new();
        for (kind, form, reading, field) in [
            ("observed", Some(token.surface.clone()), value(20), "kana"),
            ("base", value(10), value(21), "kanaBase"),
            ("lemma", value(7), value(6), "lForm"),
        ] {
            if let Some(form) = form.filter(|f| !f.trim().is_empty()) {
                if !forms.iter().any(|f| f.form == form) {
                    forms.push(QueryForm {
                        kind: kind.into(),
                        form,
                        reading_field: reading.as_ref().map(|_| field.into()),
                        reading,
                    });
                }
            }
        }
        morphemes.push(MorphemeToken {
            id: format!("m{}", token.index),
            source_index: token.index,
            surface: token.surface.clone(),
            char_range: token.char_range,
            pos: std::array::from_fn(value),
            lemma: value(7),
            reading: value(20),
            query_forms: forms,
        });
        end = next;
    }
    if end < chars.len() {
        gaps.push(TextGap {
            char_range: [end, chars.len()],
            surface: chars[end..].iter().collect(),
        });
    }
    Ok(UnifiedDocument {
        schema: SCHEMA.into(),
        id,
        text: text.into(),
        characters: chars.len(),
        source,
        morphemes,
        gaps,
        elapsed_ms: 0.0,
    })
}
