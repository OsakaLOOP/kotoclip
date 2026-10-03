use serde::{Deserialize, Serialize};

pub const PREPARE_TEXT_PROTOCOL_VERSION: &str = "kotoclip.prepare-text.v4";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubyAnnotation {
    pub base: String,
    pub reading: String,
    pub char_range: [usize; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedMarkup {
    pub source_range: [usize; 2],
    pub text_offset: usize,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparationMap {
    pub schema: String,
    pub source_text: String,
    pub source_sha256: String,
    pub text_sha256: String,
    /// 每个正文 Unicode scalar 在输入中的位置。
    pub origins: Vec<usize>,
    pub removed: Vec<RemovedMarkup>,
}

impl PreparationMap {
    pub fn source_ranges(&self, range: [usize; 2]) -> Vec<[usize; 2]> {
        crate::external::merge_ranges(self.origins[range[0]..range[1]].iter().map(|&i| [i, i + 1]).collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedText {
    pub text: String,
    pub annotations: Vec<RubyAnnotation>,
    pub mapping: PreparationMap,
}

pub(crate) fn is_kanji(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}'
            | '\u{20000}'..='\u{2fa1f}'
    ) || matches!(character, '々' | '〇')
}

pub(crate) fn is_kana(character: char) -> bool {
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

/// 比较时统一大小假名，保留字符数量以及作者读音和 UniDic 原始字段。
pub fn normalize_validation_reading(reading: &str) -> String {
    normalize_reading(reading)
        .chars()
        .map(|character| match character {
            'ァ' => 'ア',
            'ィ' => 'イ',
            'ゥ' => 'ウ',
            'ェ' => 'エ',
            'ォ' => 'オ',
            'ャ' => 'ヤ',
            'ュ' => 'ユ',
            'ョ' => 'ヨ',
            'ッ' => 'ツ',
            'ヮ' => 'ワ',
            _ => character,
        })
        .collect()
}

fn markdown_image_end(chars: &[char], index: usize) -> Option<usize> {
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
                return Some(close_paren + 1);
            }
        }
    }
    None
}

pub fn prepare_text(input: &str) -> PreparedText {
    let chars: Vec<char> = input.chars().collect();
    let mut cleaned = Vec::with_capacity(chars.len());
    let mut annotations = Vec::new();
    let mut origins = Vec::with_capacity(chars.len());
    let mut removed = Vec::new();
    let mut base_boundary = 0;
    let mut index = 0;

    while index < chars.len() {
        if let Some(next) = markdown_image_end(&chars, index) {
            removed.push(RemovedMarkup { source_range: [index, next], text_offset: cleaned.len(), kind: "image".into() });
            base_boundary = cleaned.len();
            index = next;
            continue;
        }
        if chars[index] != '《' {
            cleaned.push(crate::kyujitai::normalize_char(chars[index]));
            origins.push(index);
            if !is_kanji(chars[index]) && !is_kana(chars[index]) {
                base_boundary = cleaned.len();
            }
            index += 1;
            continue;
        }

        let Some(relative_end) = chars[index + 1..]
            .iter()
            .position(|&character| character == '》')
        else {
            cleaned.push(crate::kyujitai::normalize_char(chars[index]));
            origins.push(index);
            base_boundary = cleaned.len();
            index += 1;
            continue;
        };
        let annotation_end = index + 1 + relative_end;
        let reading_chars = &chars[index + 1..annotation_end];
        let valid_reading = !reading_chars.is_empty() && reading_chars.iter().copied().all(is_kana);
        let include_kana = cleaned.last().copied().is_some_and(is_kana);
        let mut base_start = cleaned.len();
        while base_start > base_boundary
            && (is_kanji(cleaned[base_start - 1])
                || (include_kana && is_kana(cleaned[base_start - 1])))
        {
            base_start -= 1;
        }
        let has_kanji = cleaned[base_start..].iter().copied().any(is_kanji);

        if valid_reading && base_start < cleaned.len() && has_kanji {
            annotations.push(RubyAnnotation {
                base: cleaned[base_start..].iter().collect(),
                reading: normalize_reading(&reading_chars.iter().collect::<String>()),
                char_range: [base_start, cleaned.len()],
            });
            base_boundary = cleaned.len();
            removed.push(RemovedMarkup { source_range: [index, annotation_end + 1], text_offset: cleaned.len(), kind: "ruby".into() });
            index = annotation_end + 1;
        } else {
            cleaned.extend(chars[index..=annotation_end].iter().copied().map(crate::kyujitai::normalize_char));
            origins.extend(index..=annotation_end);
            base_boundary = cleaned.len();
            index = annotation_end + 1;
        }
    }

    let text: String = cleaned.into_iter().collect();
    let mapping = PreparationMap {
        schema: PREPARE_TEXT_PROTOCOL_VERSION.into(), source_text: input.into(),
        source_sha256: crate::external::text_digest(input), text_sha256: crate::external::text_digest(&text),
        origins, removed,
    };
    PreparedText {
        text,
        annotations,
        mapping,
    }
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
    fn maps_repeated_ruby_and_images_to_original_scalars() {
        let input = "甲《こう》乙![](x.png)\n𠮷。甲《こう》  \n";
        let prepared = prepare_text(input);
        assert_eq!(prepared.text, "甲乙\n𠮷。甲  \n");
        assert_eq!(prepared.mapping.source_ranges([0, 2]), vec![[0, 1], [5, 6]]);
        let source: Vec<_> = input.chars().collect();
        let rebuilt: String = prepared.mapping.origins.iter().map(|&i| source[i]).collect();
        assert_eq!(rebuilt, prepared.text);
        let mut coverage: Vec<_> = prepared.mapping.origins.iter().map(|&i| [i, i + 1]).collect();
        coverage.extend(prepared.mapping.removed.iter().map(|item| item.source_range));
        coverage.sort_unstable();
        assert_eq!(coverage[0][0], 0);
        assert_eq!(coverage.last().unwrap()[1], source.len());
        assert!(coverage.windows(2).all(|pair| pair[0][1] == pair[1][0]));
        assert_eq!(prepared.mapping.removed.iter().map(|item| item.kind.as_str()).collect::<Vec<_>>(), vec!["ruby", "image", "ruby"]);
        assert_ne!(prepared.mapping.source_ranges([0, 1]), prepared.mapping.source_ranges([5, 6]));
    }

    #[test]
    fn invalid_and_unclosed_ruby_preserve_identity_mapping() {
        for input in ["甲《abc》  ", "乙《おつ", ""] {
            let prepared = prepare_text(input);
            assert_eq!(prepared.mapping.origins, (0..input.chars().count()).collect::<Vec<_>>());
            assert!(prepared.mapping.removed.is_empty());
        }
    }

    #[test]
    fn preserves_invalid_ruby() {
        assert_eq!(prepare_text("語《abc》").text, "語《abc》");
        assert_eq!(prepare_text("語《ご").text, "語《ご");
    }

    #[test]
    fn normalizes_full_size_digraphs_for_validation() {
        assert_eq!(normalize_validation_reading("キヨウガク"), "キヨウガク");
        assert_eq!(normalize_validation_reading("きょうがく"), "キヨウガク");
        assert_eq!(normalize_validation_reading("とっさ"), "トツサ");
    }

    #[test]
    fn accepts_ruby_with_a_kana_suffix_and_iteration_mark() {
        let prepared = prepare_text("可愛らしい《かわいらしい》神々《かみがみ》");
        assert_eq!(prepared.text, "可愛らしい神々");
        assert_eq!(prepared.annotations[0].base, "可愛らしい");
        assert_eq!(prepared.annotations[0].char_range, [0, 5]);
        assert_eq!(prepared.annotations[1].base, "神々");
    }

    #[test]
    fn keeps_a_kanji_marker_after_okurigana_local() {
        let prepared = prepare_text("喰い神《がみ》");
        assert_eq!(prepared.text, "喰い神");
        assert_eq!(prepared.annotations[0].base, "神");
        assert_eq!(prepared.annotations[0].char_range, [2, 3]);
    }

    #[test]
    fn images_and_invalid_markers_keep_ruby_boundaries() {
        let prepared =
            prepare_text("少女は腕に![](./00011.jpeg)《とう》の箱。神《かみ》かな《かな》");
        assert_eq!(prepared.text, "少女は腕に《とう》の箱。神かな《かな》");
        assert_eq!(prepared.annotations.len(), 1);
        assert_eq!(prepared.annotations[0].base, "神");
    }
}
