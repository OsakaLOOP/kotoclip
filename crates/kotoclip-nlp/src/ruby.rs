use crate::model::{ProviderToken, RubyValidation};
use crate::prepare::{
    grouped_annotations, is_kanji, normalize_reading, normalize_validation_reading, RubyAnnotation,
};

struct TokenReading {
    range: [usize; 2],
    reading: Option<Vec<char>>,
    boundaries: Vec<Option<usize>>,
}

impl TokenReading {
    fn new(token: &ProviderToken) -> Self {
        let surface: Vec<char> = token.surface.chars().collect();
        let reading: Option<Vec<char>> = token
            .fields
            .get(20)
            .and_then(|field| field.value.as_deref())
            .filter(|reading| !reading.is_empty())
            .map(|reading| normalize_reading(reading).chars().collect());
        let boundaries = reading
            .as_ref()
            .map(|reading| align_boundaries(&surface, reading))
            .unwrap_or_else(|| vec![None; surface.len() + 1]);
        Self {
            range: token.char_range,
            reading,
            boundaries,
        }
    }

    fn project(&self, target: [usize; 2]) -> Option<String> {
        let start = target[0].max(self.range[0]) - self.range[0];
        let end = target[1].min(self.range[1]) - self.range[0];
        let reading = self.reading.as_ref()?;
        let left = self.boundaries.get(start).copied().flatten()?;
        let right = self.boundaries.get(end).copied().flatten()?;
        (left < right).then(|| reading[left..right].iter().collect())
    }
}

// 汉字连续段整体对应非空读音，假名逐字锚定。前后可达表用于确定所有可行路径共有的边界。
fn align_boundaries(surface: &[char], reading: &[char]) -> Vec<Option<usize>> {
    let mut units = Vec::new();
    let mut cursor = 0;
    while cursor < surface.len() {
        let start = cursor;
        if is_kanji(surface[cursor]) {
            while cursor < surface.len() && is_kanji(surface[cursor]) {
                cursor += 1;
            }
            units.push((start, cursor, None));
        } else {
            let kana = normalize_reading(&surface[cursor].to_string())
                .chars()
                .next()
                .unwrap();
            cursor += 1;
            units.push((start, cursor, Some(kana)));
        }
    }
    let n = reading.len();
    let mut forward = vec![vec![false; n + 1]; units.len() + 1];
    forward[0][0] = true;
    for (i, &(_, _, literal)) in units.iter().enumerate() {
        let mut earlier = false;
        for j in 0..=n {
            forward[i + 1][j] = match literal {
                Some(character) => j > 0 && forward[i][j - 1] && reading[j - 1] == character,
                None => earlier,
            };
            earlier |= forward[i][j];
        }
    }
    let mut boundaries = vec![None; surface.len() + 1];
    // 完整 token 的出现假名始终保留，内部无法对齐时单独报告。
    boundaries[0] = Some(0);
    boundaries[surface.len()] = Some(n);
    if !forward[units.len()][n] {
        return boundaries;
    }
    let mut backward = vec![vec![false; n + 1]; units.len() + 1];
    backward[units.len()][n] = true;
    for i in (0..units.len()).rev() {
        let mut later = false;
        for j in (0..=n).rev() {
            backward[i][j] = match units[i].2 {
                Some(character) => j < n && backward[i + 1][j + 1] && reading[j] == character,
                None => later,
            };
            later |= backward[i + 1][j];
        }
    }
    for (i, &(_, end, _)) in units.iter().enumerate() {
        let mut positions = (0..=n).filter(|&j| forward[i + 1][j] && backward[i + 1][j]);
        let first = positions.next();
        if positions.next().is_none() {
            boundaries[end] = first;
        }
    }
    boundaries
}

fn token_range(tokens: &[TokenReading], target: [usize; 2]) -> Option<[usize; 2]> {
    let first = tokens.partition_point(|token| token.range[1] <= target[0]);
    let end = tokens.partition_point(|token| token.range[0] < target[1]);
    if first >= end
        || tokens[first].range[0] > target[0]
        || tokens[end - 1].range[1] < target[1]
        || tokens[first..end]
            .windows(2)
            .any(|pair| pair[0].range[1] != pair[1].range[0])
    {
        return None;
    }
    Some([first, end])
}

fn project(tokens: &[TokenReading], target: [usize; 2]) -> Option<String> {
    let [first, end] = token_range(tokens, target)?;
    tokens[first..end]
        .iter()
        .map(|token| token.project(target))
        .collect()
}

fn comparison(expected: &str, observed: &str) -> Option<&'static str> {
    if normalize_reading(expected) == normalize_reading(observed) {
        Some("exact")
    } else if normalize_validation_reading(expected) == normalize_validation_reading(observed) {
        Some("small_kana")
    } else {
        None
    }
}

pub(crate) fn validate_ruby(
    text: &[char],
    source: &[ProviderToken],
    annotations: &[RubyAnnotation],
    mapping: &crate::prepare::PreparationMap,
) -> Vec<RubyValidation> {
    if annotations.is_empty() {
        return Vec::new();
    }
    let tokens: Vec<_> = source.iter().map(TokenReading::new).collect();
    grouped_annotations(annotations)
        .into_iter()
        .map(|mut group| {
            let original_range = group.char_range;
            let first_annotation_end = annotations
                .iter()
                .find(|annotation| annotation.char_range[0] == original_range[0])
                .map(|annotation| annotation.char_range[1])
                .unwrap_or(original_range[1]);
            // 标记省略左边界时，按可独立投影的后缀寻找最大匹配范围；逐字 ruby 先合并以保留熟字训。
            for start in group.char_range[0]..first_annotation_end {
                if !is_kanji(text[start]) {
                    continue;
                }
                let range = [start, group.char_range[1]];
                if project(&tokens, range)
                    .is_some_and(|observed| comparison(&group.reading, &observed).is_some())
                {
                    group.char_range = range;
                    group.base = text[start..range[1]].iter().collect();
                    break;
                }
            }
            let range = token_range(&tokens, group.char_range);
            let observed = project(&tokens, group.char_range);
            let method = observed
                .as_ref()
                .and_then(|observed| comparison(&group.reading, observed));
            let (status, reason) = match (&observed, range, method) {
                (Some(_), _, Some(method)) => ("matched", method),
                (Some(_), _, None) => ("variant", "reading_difference"),
                (None, Some([first, end]), _)
                    if tokens[first..end]
                        .iter()
                        .any(|token| token.reading.is_none()) =>
                {
                    ("unavailable", "missing_reading")
                }
                (None, Some(_), _) => ("unavailable", "ambiguous_alignment"),
                (None, None, _) => ("unmatched", "source_gap"),
            };
            RubyValidation {
                base: group.base,
                ruby_reading: group.reading.clone(),
                expected_reading: group.reading,
                char_range: group.char_range,
                candidate_char_range: original_range,
                source_char_ranges: mapping.source_ranges(group.char_range),
                token_range: range,
                observed_reading: observed,
                status: status.into(),
                reason: reason.into(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{prepare::prepare_text, sources::parse_fields};

    fn validate(input: &str, tokens: &[(&str, Option<&str>)]) -> Vec<RubyValidation> {
        let prepared = prepare_text(input);
        let mut offset = 0;
        let tokens: Vec<_> = tokens
            .iter()
            .enumerate()
            .map(|(index, (surface, reading))| {
                let mut fields = parse_fields("名詞,普通名詞,一般,*,*,*", true).unwrap();
                fields[20].value = reading.map(str::to_owned);
                let start = offset;
                offset += surface.chars().count();
                ProviderToken {
                    index,
                    surface: (*surface).into(),
                    char_range: [start, offset],
                    byte_range: [0, 0],
                    lexicon_type: "system".into(),
                    left_id: 0,
                    right_id: 0,
                    word_cost: 0,
                    total_cost: 0,
                    raw_feature: String::new(),
                    fields,
                }
            })
            .collect();
        validate_ruby(
            &prepared.text.chars().collect::<Vec<_>>(),
            &tokens,
            &prepared.annotations,
            &prepared.mapping,
        )
    }

    #[test]
    fn projects_compound_readings_from_kana_anchors() {
        for (input, surface, reading, expected) in [
            (
                "可愛《カワイ》らしい",
                "可愛らしい",
                "カワイラシイ",
                "カワイ",
            ),
            ("覗《のぞ》き見る", "覗き見る", "ノゾキミル", "ノゾ"),
            ("引き摺《ず》った", "引き摺った", "ヒキズッタ", "ズ"),
            ("真っ直《す》ぐ", "真っ直ぐ", "マッスグ", "ス"),
        ] {
            let results = validate(input, &[(surface, Some(reading))]);
            assert_eq!(results[0].status, "matched", "{input}");
            assert_eq!(results[0].observed_reading.as_deref(), Some(expected));
        }
        let results = validate(
            "覆《おお》い被《かぶ》さる",
            &[("覆い被さる", Some("オオイカブサル"))],
        );
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|result| result.status == "matched"));
    }

    #[test]
    fn repeated_annotations_keep_separate_source_positions() {
        let results = validate("甲《こう》甲《こう》", &[("甲甲", Some("コウコウ"))]);
        assert_eq!(results[0].source_char_ranges, vec![[0, 1], [5, 6]]);
        assert_eq!(results[0].candidate_char_range, [0, 2]);
    }

    #[test]
    fn groups_ruby_before_resolving_multi_token_coverage() {
        let results = validate(
            "産業廃《はい》棄《き》物《ぶつ》",
            &[
                ("産業", Some("サンギョウ")),
                ("廃棄", Some("ハイキ")),
                ("物", Some("ブツ")),
            ],
        );
        assert_eq!(results[0].base, "廃棄物");
        assert_eq!(results[0].candidate_char_range, [0, 5]);
        assert_eq!(results[0].char_range, [2, 5]);
        assert_eq!(results[0].token_range, Some([1, 3]));
        assert_eq!(results[0].status, "matched");
        let results = validate("大《おと》人《な》", &[("大人", Some("オトナ"))]);
        assert_eq!(results[0].base, "大人");
        assert_eq!(results[0].status, "matched");
        let results = validate(
            "甲《こ》乙《う》",
            &[("甲", Some("キノエ")), ("乙", Some("コウ"))],
        );
        assert_eq!(results[0].base, "甲乙");
        assert_eq!(results[0].status, "variant");
    }

    #[test]
    fn ambiguous_or_missing_readings_remain_pending() {
        let cases = [
            ("仏《フランス》語", "仏語", Some("ブツゴ")),
            ("学校《こう》", "学校", Some("ガッコウ")),
            ("甲《こう》あ乙", "甲あ乙", Some("コウアアオツ")),
            ("未知《みち》", "未知", None),
        ];
        for (input, surface, reading) in cases {
            let results = validate(input, &[(surface, reading)]);
            assert_ne!(results[0].status, "matched", "{input}");
        }
        let results = validate("引き摺《ひ》った", &[("引き摺った", Some("ヒキズッタ"))]);
        assert_eq!(results[0].status, "variant");
        assert_eq!(results[0].observed_reading.as_deref(), Some("ズ"));
        assert_eq!(validate("空《そら》", &[])[0].status, "unmatched");
    }
}
