pub const PREPARE_TEXT_PROTOCOL_VERSION: &str = "kotoclip.prepare-text.v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RubyAnnotation {
    pub base: String,
    pub reading: String,
    pub char_range: [usize; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedText {
    pub text: String,
    pub annotations: Vec<RubyAnnotation>,
}

fn is_kanji(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}'
            | '\u{20000}'..='\u{2fa1f}'
    )
}

fn is_kana(character: char) -> bool {
    matches!(
        character,
        '\u{3041}'..='\u{3096}'
            | '\u{309d}'..='\u{309f}'
            | '\u{30a1}'..='\u{30fa}'
            | '\u{30fd}'..='\u{30ff}'
            | 'ー'
    )
}

pub fn normalize_reading(reading: &str) -> String {
    reading
        .chars()
        .map(|character| {
            if ('\u{3041}'..='\u{3096}').contains(&character) {
                char::from_u32(character as u32 + 0x60).unwrap_or(character)
            } else {
                character
            }
        })
        .collect()
}

fn strip_markdown_images(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < chars.len() {
        if index + 1 < chars.len() && chars[index] == '!' && chars[index + 1] == '[' {
            let mut close_bracket = index + 2;
            while close_bracket < chars.len() && chars[close_bracket] != ']' {
                close_bracket += 1;
            }
            if close_bracket + 1 < chars.len() && chars[close_bracket + 1] == '(' {
                let mut close_paren = close_bracket + 2;
                while close_paren < chars.len() && chars[close_paren] != ')' {
                    close_paren += 1;
                }
                if close_paren < chars.len() {
                    index = close_paren + 1;
                    continue;
                }
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

pub fn prepare_text(input: &str) -> PreparedText {
    let chars: Vec<char> = strip_markdown_images(input).chars().collect();
    let mut cleaned = Vec::with_capacity(chars.len());
    let mut annotations = Vec::new();
    let mut base_boundary = 0;
    let mut index = 0;

    while index < chars.len() {
        if chars[index] != '《' {
            cleaned.push(chars[index]);
            if !is_kanji(chars[index]) {
                base_boundary = cleaned.len();
            }
            index += 1;
            continue;
        }

        let Some(relative_end) = chars[index + 1..]
            .iter()
            .position(|&character| character == '》')
        else {
            cleaned.push(chars[index]);
            base_boundary = cleaned.len();
            index += 1;
            continue;
        };
        let annotation_end = index + 1 + relative_end;
        let reading_chars = &chars[index + 1..annotation_end];
        let valid_reading = !reading_chars.is_empty() && reading_chars.iter().copied().all(is_kana);
        let mut base_start = cleaned.len();
        while base_start > base_boundary && is_kanji(cleaned[base_start - 1]) {
            base_start -= 1;
        }

        if valid_reading && base_start < cleaned.len() {
            annotations.push(RubyAnnotation {
                base: cleaned[base_start..].iter().collect(),
                reading: normalize_reading(&reading_chars.iter().collect::<String>()),
                char_range: [base_start, cleaned.len()],
            });
            base_boundary = cleaned.len();
            index = annotation_end + 1;
        } else {
            cleaned.extend_from_slice(&chars[index..=annotation_end]);
            base_boundary = cleaned.len();
            index = annotation_end + 1;
        }
    }

    PreparedText {
        text: cleaned.into_iter().collect(),
        annotations,
    }
}

pub fn expected_reading_for_range(
    text_chars: &[char],
    char_range: [usize; 2],
    annotations: &[RubyAnnotation],
) -> Option<String> {
    let relevant: Vec<&RubyAnnotation> = annotations
        .iter()
        .filter(|annotation| {
            annotation.char_range[0] >= char_range[0] && annotation.char_range[1] <= char_range[1]
        })
        .collect();
    if relevant.is_empty() || char_range[1] > text_chars.len() {
        return None;
    }

    let mut output = String::new();
    let mut cursor = char_range[0];
    let mut annotation_index = 0;
    while cursor < char_range[1] {
        if let Some(annotation) = relevant.get(annotation_index) {
            if annotation.char_range[0] == cursor {
                output.push_str(&annotation.reading);
                cursor = annotation.char_range[1];
                annotation_index += 1;
                continue;
            }
        }
        let character = text_chars[cursor];
        if !is_kana(character) {
            return None;
        }
        output.push_str(&normalize_reading(&character.to_string()));
        cursor += 1;
    }
    Some(output)
}

pub fn grouped_annotations(annotations: &[RubyAnnotation]) -> Vec<RubyAnnotation> {
    let mut groups = Vec::new();
    let mut current: Option<RubyAnnotation> = None;
    for annotation in annotations {
        match current.as_mut() {
            Some(group) if group.char_range[1] == annotation.char_range[0] => {
                group.base.push_str(&annotation.base);
                group.reading.push_str(&annotation.reading);
                group.char_range[1] = annotation.char_range[1];
            }
            Some(_) => {
                groups.push(current.take().expect("ruby group"));
                current = Some(annotation.clone());
            }
            None => current = Some(annotation.clone()),
        }
    }
    if let Some(group) = current {
        groups.push(group);
    }
    groups
}

pub fn has_long_dialogue(text: &str) -> bool {
    let mut opening: Option<usize> = None;
    let chars: Vec<char> = text.chars().collect();
    for (index, character) in chars.iter().copied().enumerate() {
        match character {
            '「' if opening.is_none() => opening = Some(index),
            '」' => {
                if let Some(start) = opening.take() {
                    if chars[start + 1..index].iter().count() > 10 {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_ruby_and_keeps_scalar_ranges() {
        let prepared = prepare_text("煙草《たばこ》と古《ふる》川《かわ》");
        assert_eq!(prepared.text, "煙草と古川");
        assert_eq!(prepared.annotations[0].reading, "タバコ");
        assert_eq!(prepared.annotations[1].char_range, [3, 4]);
        assert_eq!(prepared.annotations[2].char_range, [4, 5]);
        let groups = grouped_annotations(&prepared.annotations);
        assert_eq!(groups[1].base, "古川");
        assert_eq!(groups[1].reading, "フルカワ");
    }

    #[test]
    fn preserves_invalid_ruby() {
        assert_eq!(prepare_text("語《abc》").text, "語《abc》");
        assert_eq!(prepare_text("語《ご").text, "語《ご");
    }

    #[test]
    fn routes_only_dialogue_longer_than_ten_scalars() {
        assert!(!has_long_dialogue("「1234567890」"));
        assert!(has_long_dialogue("「12345678901」"));
        assert!(has_long_dialogue("本文「これは十一个字以上です」本文"));
    }
}
