use crate::model::*;
use crate::prepare::{
    expected_reading_for_range, grouped_annotations, normalize_reading, PreparedText,
};
use sha2::{Digest, Sha256};

pub fn unify(
    prepared: &PreparedText,
    source: SourceAnalysis,
    routing: RegisterRouting,
) -> Result<UnifiedDocument, String> {
    let text = &prepared.text;
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
    let ruby_validations = validate_ruby(&chars, &source.tokens, &prepared.annotations);
    Ok(UnifiedDocument {
        schema: SCHEMA.into(),
        id,
        text: text.into(),
        characters: chars.len(),
        source,
        routing,
        ruby_validations,
        morphemes,
        gaps,
        elapsed_ms: 0.0,
    })
}

fn validate_ruby(
    text_chars: &[char],
    source_tokens: &[ProviderToken],
    annotations: &[crate::prepare::RubyAnnotation],
) -> Vec<RubyValidation> {
    grouped_annotations(annotations)
        .into_iter()
        .map(|group| {
            let overlapping: Vec<usize> = source_tokens
                .iter()
                .enumerate()
                .filter(|(_, token)| {
                    token.char_range[1] > group.char_range[0]
                        && token.char_range[0] < group.char_range[1]
                })
                .map(|(index, _)| index)
                .collect();

            let Some(&first) = overlapping.first() else {
                return RubyValidation {
                    base: group.base,
                    ruby_reading: group.reading.clone(),
                    expected_reading: group.reading,
                    char_range: group.char_range,
                    token_range: None,
                    observed_reading: None,
                    status: "unmatched".into(),
                };
            };
            let last = *overlapping.last().unwrap();
            let contiguous = (first..last).all(|index| {
                source_tokens[index].char_range[1] == source_tokens[index + 1].char_range[0]
            });
            let coverage_range = [
                source_tokens[first].char_range[0],
                source_tokens[last].char_range[1],
            ];
            let covers_group = contiguous && coverage_range == group.char_range;
            let observed_reading = if covers_group {
                let reading: String = source_tokens[first..=last]
                    .iter()
                    .filter_map(|token| {
                        token
                            .fields
                            .get(20)
                            .and_then(|field| field.value.as_deref())
                    })
                    .map(normalize_reading)
                    .collect();
                (!reading.is_empty()).then_some(reading)
            } else {
                None
            };
            let expected_reading = if covers_group {
                expected_reading_for_range(text_chars, coverage_range, annotations)
                    .unwrap_or_else(|| group.reading.clone())
            } else {
                group.reading.clone()
            };
            let status = match &observed_reading {
                Some(observed) if observed == &expected_reading => "matched",
                Some(_) => "mismatch",
                None if covers_group => "unavailable",
                None => "unmatched",
            };
            RubyValidation {
                base: group.base,
                ruby_reading: group.reading,
                expected_reading,
                char_range: group.char_range,
                token_range: Some([first, last + 1]),
                observed_reading,
                status: status.into(),
            }
        })
        .collect()
}
