use crate::models::{Bunsetsu, GrammarTag, HeadWord, Morpheme};

/// 判断形态素是否是自立语 (能够独立构成词意的词，如动词、名词、形容词等)
fn is_jiritsugo(m: &Morpheme) -> bool {
    let pos = &m.pos;
    match pos.major.as_str() {
        "動詞" => pos.sub1 == "自立",
        "形容詞" => true,
        "名詞" => {
            // 名词中要排除接尾辞、非自立名词等
            pos.sub1 != "接尾" && pos.sub1 != "非自立" && pos.sub1 != "特殊"
        }
        "副詞" | "連体詞" | "接続詞" | "感動詞" | "接頭詞" => true,
        _ => false,
    }
}

/// 判断形态素是否是标点/记号
fn is_punctuation(m: &Morpheme) -> bool {
    m.pos.major == "記号"
}

/// 基于词性状态机，将形态素序列聚合成文节 (Bunsetsu) 列表，并在此阶段应用用户自定义合并规则
pub fn chunk(morphemes: &[Morpheme], merge_rules: &[Vec<String>]) -> Vec<Bunsetsu> {
    if morphemes.is_empty() {
        return Vec::new();
    }

    let mut bunsetsus = Vec::new();
    let mut current_morphemes: Vec<Morpheme> = Vec::new();
    let mut i = 0;
    let n = morphemes.len();

    while i < n {
        // 尝试匹配用户自定义合并规则 (优先尝试匹配更长的规则)
        let mut matched_rule_len = 0;
        for rule in merge_rules {
            let rule_len = rule.len();
            if i + rule_len <= n {
                let mut matches = true;
                for (offset, expected_surface) in rule.iter().enumerate() {
                    if morphemes[i + offset].surface != *expected_surface {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    matched_rule_len = rule_len;
                    break;
                }
            }
        }

        if matched_rule_len > 0 {
            // 如果在匹配前，current_morphemes 中有暂存数据，先结算它们
            if !current_morphemes.is_empty() {
                bunsetsus.push(build_bunsetsu(current_morphemes));
                current_morphemes = Vec::new();
            }

            // 将匹配到的这组形态素强制聚合并输出为一个独立文节
            let mut merged = Vec::new();
            for offset in 0..matched_rule_len {
                merged.push(morphemes[i + offset].clone());
            }
            bunsetsus.push(build_bunsetsu(merged));

            // 跳转索引并继续
            i += matched_rule_len;
            continue;
        }

        // 常规形态素组块逻辑 (词性状态机)
        let m = &morphemes[i];
        let m_clone = m.clone();

        if current_morphemes.is_empty() {
            current_morphemes.push(m_clone);
            i += 1;
            continue;
        }

        let is_m_jiritsugo = is_jiritsugo(m);
        let is_m_punc = is_punctuation(m);
        let is_prev_punc = is_punctuation(&current_morphemes[current_morphemes.len() - 1]);
        let is_prev_prefix = current_morphemes[current_morphemes.len() - 1].pos.major == "接頭詞";

        if is_m_punc || is_prev_punc || (is_m_jiritsugo && !is_prev_prefix) {
            bunsetsus.push(build_bunsetsu(current_morphemes));
            current_morphemes = Vec::new();
        }

        current_morphemes.push(m_clone);
        i += 1;
    }

    if !current_morphemes.is_empty() {
        bunsetsus.push(build_bunsetsu(current_morphemes));
    }

    bunsetsus
}

/// 构造单个文节，并提取其核心自立语 (HeadWord) 与属性
pub(crate) fn build_bunsetsu(morphemes: Vec<Morpheme>) -> Bunsetsu {
    // 拼接表层形
    let surface: String = morphemes.iter().map(|m| m.surface.as_str()).collect();

    // 确定字符偏移区间
    let start = morphemes.first().map(|m| m.char_range.0).unwrap_or(0);
    let end = morphemes.last().map(|m| m.char_range.1).unwrap_or(0);

    // 提取核心自立语
    // 优先选择文节中第一个满足 is_jiritsugo 且不是 "接頭詞" 的形态素
    // 如果找不到，则回退选择第一个自立语，或者整个文节的第一个形态素
    let head_index = morphemes
        .iter()
        .position(|m| is_jiritsugo(m) && m.pos.major != "接頭詞")
        .or_else(|| morphemes.iter().position(is_jiritsugo))
        .unwrap_or(0);
    let head_morpheme = &morphemes[head_index];

    // Keep the candidate available; the dictionary-aware resolver decides whether
    // nominal suffixes belong to the lexical head.
    let mut head_surface = head_morpheme.surface.clone();
    let mut head_base_form = head_morpheme.base_form.clone();
    let mut head_reading = head_morpheme.reading.clone();
    if head_morpheme.pos.major == "名詞" {
        for suffix in morphemes.iter().skip(head_index + 1) {
            if suffix.pos.major != "名詞" || suffix.pos.sub1 != "接尾" {
                break;
            }
            head_surface.push_str(&suffix.surface);
            head_base_form.push_str(&suffix.base_form);
            if suffix.reading != "*" {
                if head_reading == "*" {
                    head_reading.clear();
                }
                head_reading.push_str(&suffix.reading);
            }
        }
    }

    let head_word = HeadWord {
        surface: head_surface,
        base_form: head_base_form,
        reading: head_reading,
        pos: head_morpheme.pos.clone(),
    };

    let mut bunsetsu = Bunsetsu {
        morphemes,
        surface,
        head_word,
        grammar_tags: Vec::new(), // 在后续的语法匹配阶段填充
        char_range: (start, end),
    };

    bunsetsu.head_word.base_form = super::restore::restore_base_form(&bunsetsu);
    bunsetsu
}

pub fn resolve_lexical_boundaries<F: Fn(&str) -> bool>(
    bunsetsus: &mut [Bunsetsu],
    contains_exact: F,
) {
    for bunsetsu in bunsetsus {
        let Some(head_index) = bunsetsu
            .morphemes
            .iter()
            .position(|m| is_jiritsugo(m) && m.pos.major != "接頭詞")
        else {
            continue;
        };
        let suffix_start = head_index + 1;

        let mut suffix_indices = Vec::new();
        for idx in suffix_start..bunsetsu.morphemes.len() {
            let m = &bunsetsu.morphemes[idx];
            if m.pos.major == "名詞" && m.pos.sub1 == "接尾" {
                suffix_indices.push(idx);
            } else {
                break;
            }
        }
        if suffix_indices.is_empty() {
            continue;
        }

        let candidate = bunsetsu.head_word.base_form.clone();
        if contains_exact(&candidate) {
            continue;
        }

        let root = &bunsetsu.morphemes[head_index];
        bunsetsu.head_word.surface = root.surface.clone();
        bunsetsu.head_word.base_form = root.base_form.clone();
        bunsetsu.head_word.reading = root.reading.clone();

        for idx in suffix_indices {
            let suffix = &bunsetsu.morphemes[idx];
            bunsetsu.grammar_tags.push(GrammarTag {
                pattern_id: format!("nominal_suffix:{}", suffix.surface),
                name_ja: format!("接尾辞「{}」", suffix.surface),
                name_en: "Nominal suffix".to_string(),
                jlpt_level: None,
                description: "词典未收录的名词接尾辞".to_string(),
                morpheme_range: (idx, idx + 1),
                char_range: suffix.char_range,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PosTag;

    #[test]
    fn test_resolve_lexical_boundaries_splits_on_missing_dict() {
        let m1 = Morpheme {
            surface: "警察".to_string(),
            pos: PosTag {
                major: "名詞".to_string(),
                sub1: "一般".to_string(),
                sub2: "*".to_string(),
                sub3: "*".to_string(),
            },
            base_form: "警察".to_string(),
            reading: "ケイサツ".to_string(),
            conjugation_type: "*".to_string(),
            conjugation_form: "*".to_string(),
            char_range: (0, 2),
        };
        let m2 = Morpheme {
            surface: "署".to_string(),
            pos: PosTag {
                major: "名詞".to_string(),
                sub1: "接尾".to_string(),
                sub2: "*".to_string(),
                sub3: "*".to_string(),
            },
            base_form: "署".to_string(),
            reading: "ショ".to_string(),
            conjugation_type: "*".to_string(),
            conjugation_form: "*".to_string(),
            char_range: (2, 3),
        };
        let m3 = Morpheme {
            surface: "に".to_string(),
            pos: PosTag {
                major: "助詞".to_string(),
                sub1: "格助詞".to_string(),
                sub2: "*".to_string(),
                sub3: "*".to_string(),
            },
            base_form: "に".to_string(),
            reading: "ニ".to_string(),
            conjugation_type: "*".to_string(),
            conjugation_form: "*".to_string(),
            char_range: (3, 4),
        };

        // 场景 1：如果字典中没有“警察署”，应剥离并产生接尾辞的 grammar tag
        let bunsetsu = build_bunsetsu(vec![m1.clone(), m2.clone(), m3.clone()]);
        let mut list = vec![bunsetsu];
        resolve_lexical_boundaries(&mut list, |word| word == "警察");

        assert_eq!(list[0].head_word.base_form, "警察");
        assert_eq!(list[0].grammar_tags.len(), 1);
        assert_eq!(list[0].grammar_tags[0].pattern_id, "nominal_suffix:署");
        assert_eq!(list[0].grammar_tags[0].char_range, (2, 3));

        // 场景 2：如果字典中有“警察署”，保留合并的长词
        let bunsetsu = build_bunsetsu(vec![m1.clone(), m2.clone(), m3.clone()]);
        let mut list = vec![bunsetsu];
        resolve_lexical_boundaries(&mut list, |word| word == "警察署");

        assert_eq!(list[0].head_word.base_form, "警察署");
        assert!(list[0].grammar_tags.is_empty());
    }
}
