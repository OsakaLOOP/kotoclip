use crate::models::{Bunsetsu, HeadWord, Morpheme};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RubyAnnotation {
    pub base: String,
    pub reading: String,
    pub char_range: (usize, usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedText {
    pub text: String,
    pub annotations: Vec<RubyAnnotation>,
}

fn is_kanji(c: char) -> bool {
    matches!(
        c,
        '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}'
            | '\u{20000}'..='\u{2fa1f}'
    )
}

fn is_kana(c: char) -> bool {
    matches!(
        c,
        '\u{3041}'..='\u{3096}'
            | '\u{309d}'..='\u{309f}'
            | '\u{30a1}'..='\u{30fa}'
            | '\u{30fd}'..='\u{30ff}'
            | 'ー'
    )
}

fn normalize_reading(reading: &str) -> String {
    reading
        .chars()
        .map(|c| {
            if ('\u{3041}'..='\u{3096}').contains(&c) {
                char::from_u32(c as u32 + 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

fn strip_markdown_images(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut result = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '!' && chars[i + 1] == '[' {
            let mut j = i + 2;
            let mut found_bracket = false;
            while j < chars.len() {
                if chars[j] == ']' {
                    found_bracket = true;
                    break;
                }
                j += 1;
            }
            if found_bracket && j + 1 < chars.len() && chars[j + 1] == '(' {
                let mut k = j + 2;
                let mut found_paren = false;
                while k < chars.len() {
                    if chars[k] == ')' {
                        found_paren = true;
                        break;
                    }
                    k += 1;
                }
                if found_paren {
                    i = k + 1;
                    continue;
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result.into_iter().collect()
}

/// Removes valid `漢字《かな》` markup and records its range in the cleaned text.
/// A new ruby annotation starts a new base boundary, which makes
/// `古《ふる》川《かわ》` two annotations instead of treating the second as `古川`.
pub fn prepare_text(input: &str) -> PreparedText {
    let stripped = strip_markdown_images(input);
    let chars: Vec<char> = stripped.chars().collect();
    let mut cleaned = Vec::with_capacity(chars.len());
    let mut annotations = Vec::new();
    let mut base_boundary = 0;
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '《' {
            cleaned.push(chars[i]);
            if !is_kanji(chars[i]) {
                base_boundary = cleaned.len();
            }
            i += 1;
            continue;
        }

        let Some(relative_end) = chars[i + 1..].iter().position(|&c| c == '》') else {
            cleaned.push(chars[i]);
            base_boundary = cleaned.len();
            i += 1;
            continue;
        };
        let annotation_end = i + 1 + relative_end;
        let reading_chars = &chars[i + 1..annotation_end];
        let valid_reading = !reading_chars.is_empty() && reading_chars.iter().all(|&c| is_kana(c));

        let mut base_start = cleaned.len();
        while base_start > base_boundary && is_kanji(cleaned[base_start - 1]) {
            base_start -= 1;
        }

        if valid_reading && base_start < cleaned.len() {
            let base: String = cleaned[base_start..].iter().collect();
            let reading: String = reading_chars.iter().collect();
            annotations.push(RubyAnnotation {
                base,
                reading: normalize_reading(&reading),
                char_range: (base_start, cleaned.len()),
            });
            base_boundary = cleaned.len();
            i = annotation_end + 1;
        } else {
            cleaned.extend_from_slice(&chars[i..=annotation_end]);
            base_boundary = cleaned.len();
            i = annotation_end + 1;
        }
    }

    PreparedText {
        text: cleaned.into_iter().collect(),
        annotations,
    }
}

fn reading_for_range(
    text_chars: &[char],
    char_range: (usize, usize),
    annotations: &[RubyAnnotation],
) -> Option<String> {
    let relevant: Vec<&RubyAnnotation> = annotations
        .iter()
        .filter(|annotation| {
            annotation.char_range.0 >= char_range.0 && annotation.char_range.1 <= char_range.1
        })
        .collect();
    if relevant.is_empty() {
        return None;
    }

    let mut reading = String::new();
    let mut cursor = char_range.0;
    let mut annotation_index = 0;

    while cursor < char_range.1 {
        if let Some(annotation) = relevant.get(annotation_index) {
            if annotation.char_range.0 == cursor {
                reading.push_str(&annotation.reading);
                cursor = annotation.char_range.1;
                annotation_index += 1;
                continue;
            }
        }

        let c = text_chars[cursor];
        if is_kana(c) {
            reading.push_str(&normalize_reading(&c.to_string()));
            cursor += 1;
        } else {
            // An unannotated kanji cannot be aligned to a subsection of the NLP reading
            // without guessing. Preserve the analyzer's complete reading in that case.
            return None;
        }
    }

    Some(reading)
}

pub fn override_morpheme_readings(
    text: &str,
    morphemes: &mut [Morpheme],
    annotations: &[RubyAnnotation],
) {
    let text_chars: Vec<char> = text.chars().collect();
    override_morpheme_readings_with_chars(&text_chars, morphemes, annotations);
}

pub fn override_morpheme_readings_with_chars(
    text_chars: &[char],
    morphemes: &mut [Morpheme],
    annotations: &[RubyAnnotation],
) {
    for morpheme in morphemes {
        if let Some(reading) = reading_for_range(text_chars, morpheme.char_range, annotations) {
            morpheme.reading = reading;
        }
    }
}

pub fn override_bunsetsu_readings(
    text: &str,
    bunsetsus: &mut [Bunsetsu],
    annotations: &[RubyAnnotation],
) {
    let text_chars: Vec<char> = text.chars().collect();
    override_bunsetsu_readings_with_chars(&text_chars, bunsetsus, annotations);
}

pub fn override_bunsetsu_readings_with_chars(
    text_chars: &[char],
    bunsetsus: &mut [Bunsetsu],
    annotations: &[RubyAnnotation],
) {
    let document_readings = document_reading_map(annotations);
    for bunsetsu in bunsetsus {
        if let Some(reading) = document_readings
            .get(&bunsetsu.head_word.surface)
            .or_else(|| document_readings.get(&bunsetsu.head_word.base_form))
        {
            bunsetsu.head_word.reading = reading.clone();
            continue;
        }
        // A whole-word annotation is also an explicit lexical boundary. This covers
        // cases where the tokenizer kept the word in one bunsetsu but chose a shorter head.
        if let Some(annotation) = annotations.iter().find(|annotation| {
            annotation.char_range.0 >= bunsetsu.char_range.0
                && annotation.char_range.1 <= bunsetsu.char_range.1
                && bunsetsu.head_word.surface != annotation.base
                && annotation.base.starts_with(&bunsetsu.head_word.surface)
                && bunsetsu
                    .morphemes
                    .iter()
                    .any(|m| m.char_range.0 == annotation.char_range.0)
                && bunsetsu
                    .morphemes
                    .iter()
                    .any(|m| m.char_range.1 == annotation.char_range.1)
        }) {
            bunsetsu.head_word.surface = annotation.base.clone();
            bunsetsu.head_word.base_form = annotation.base.clone();
            bunsetsu.head_word.reading = annotation.reading.clone();
            continue;
        }

        for start in 0..bunsetsu.morphemes.len() {
            let mut surface = String::new();
            for end in start..bunsetsu.morphemes.len() {
                surface.push_str(&bunsetsu.morphemes[end].surface);
                if surface == bunsetsu.head_word.surface {
                    let char_range = (
                        bunsetsu.morphemes[start].char_range.0,
                        bunsetsu.morphemes[end].char_range.1,
                    );
                    if let Some(reading) = reading_for_range(text_chars, char_range, annotations) {
                        bunsetsu.head_word.reading = reading;
                    }
                    break;
                }
                if !bunsetsu.head_word.surface.starts_with(&surface) {
                    break;
                }
            }
        }
    }
}

/// 将相邻的显式 ruby 合成为文档内词级读音。仅传播至少两个字符的词，
/// 避免把「七《なの》」这类单字局部读音应用到所有普通单字。
fn document_reading_map(annotations: &[RubyAnnotation]) -> HashMap<String, String> {
    let mut readings = HashMap::new();
    let mut index = 0;
    while index < annotations.len() {
        let mut end = index + 1;
        while end < annotations.len()
            && annotations[end - 1].char_range.1 == annotations[end].char_range.0
        {
            end += 1;
        }
        let base: String = annotations[index..end]
            .iter()
            .map(|annotation| annotation.base.as_str())
            .collect::<Vec<_>>()
            .concat();
        let reading: String = annotations[index..end]
            .iter()
            .map(|annotation| annotation.reading.as_str())
            .collect::<Vec<_>>()
            .concat();
        if base.chars().count() >= 2 {
            readings.insert(base, reading);
        }
        index = end;
    }
    readings
}

/// Treats a whole-word ruby span as a lexical boundary when IPADIC split the
/// annotated base into multiple bunsetsu. NLP still supplies the part of speech;
/// the author-supplied base and reading become the authoritative headword.
pub fn merge_annotated_bunsetsus(
    mut bunsetsus: Vec<Bunsetsu>,
    annotations: &[RubyAnnotation],
) -> Vec<Bunsetsu> {
    for annotation in annotations {
        let Some(start) = bunsetsus
            .iter()
            .position(|bunsetsu| bunsetsu.char_range.0 == annotation.char_range.0)
        else {
            continue;
        };
        let Some(relative_end) = bunsetsus[start..]
            .iter()
            .position(|bunsetsu| bunsetsu.char_range.1 == annotation.char_range.1)
        else {
            continue;
        };
        let end = start + relative_end;
        if end == start
            || bunsetsus[start..=end].iter().any(|bunsetsu| {
                bunsetsu.char_range.0 < annotation.char_range.0
                    || bunsetsu.char_range.1 > annotation.char_range.1
            })
        {
            continue;
        }

        let pos = bunsetsus[start].head_word.pos.clone();
        let morphemes: Vec<Morpheme> = bunsetsus[start..=end]
            .iter()
            .flat_map(|bunsetsu| bunsetsu.morphemes.iter().cloned())
            .collect();
        let merged = Bunsetsu {
            morphemes,
            surface: annotation.base.clone(),
            head_word: HeadWord {
                surface: annotation.base.clone(),
                base_form: annotation.base.clone(),
                reading: annotation.reading.clone(),
                pos,
            },
            grammar_tags: Vec::new(),
            char_range: annotation.char_range,
        };
        bunsetsus.splice(start..=end, [merged]);
    }

    bunsetsus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_whole_word_and_segmented_ruby() {
        let prepared = prepare_text("煙草《たばこ》と古《ふる》川《かわ》");

        assert_eq!(prepared.text, "煙草と古川");
        assert_eq!(prepared.annotations.len(), 3);
        assert_eq!(prepared.annotations[0].base, "煙草");
        assert_eq!(prepared.annotations[0].reading, "タバコ");
        assert_eq!(prepared.annotations[0].char_range, (0, 2));
        assert_eq!(prepared.annotations[1].char_range, (3, 4));
        assert_eq!(prepared.annotations[2].char_range, (4, 5));
    }

    #[test]
    fn preserves_invalid_or_unclosed_markup() {
        assert_eq!(prepare_text("語《abc》").text, "語《abc》");
        assert_eq!(prepare_text("語《ご").text, "語《ご");
    }

    #[test]
    fn combines_authoritative_ruby_with_okurigana() {
        let prepared = prepare_text("屈《かが》み");
        let reading = reading_for_range(
            &prepared.text.chars().collect::<Vec<_>>(),
            (0, 2),
            &prepared.annotations,
        );
        assert_eq!(reading.as_deref(), Some("カガミ"));
    }

    #[test]
    fn combines_adjacent_ruby_into_document_word_reading() {
        let prepared = prepare_text("七《なの》日《か》と七日");
        let readings = document_reading_map(&prepared.annotations);
        assert_eq!(readings.get("七日").map(String::as_str), Some("ナノカ"));
        assert!(!readings.contains_key("七"));
    }

    #[test]
    fn test_strip_markdown_images() {
        assert_eq!(
            strip_markdown_images("这是一个![alt](./img.jpg)图片"),
            "这是一个图片"
        );
        assert_eq!(strip_markdown_images("没有![未闭合"), "没有![未闭合");
        assert_eq!(
            strip_markdown_images("普通的[链接](url)"),
            "普通的[链接](url)"
        );
        assert_eq!(strip_markdown_images("多个![a](b)和![c](d)"), "多个和");
    }
}
