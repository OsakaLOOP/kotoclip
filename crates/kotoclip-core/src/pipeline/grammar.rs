use crate::models::{Bunsetsu, GrammarTag, Morpheme, PosTag};
use aho_corasick::AhoCorasick;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 语法约束条件，用于二次验证
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// 匹配序列中的相对索引 (从 0 开始)
    pub index: usize,
    /// 校验的字段 (如 "surface", "base_form", "pos_major")
    pub field: String,
    /// 允许的候选值列表
    pub values: Vec<String>,
}

/// 语法模式结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarPattern {
    pub id: String,
    pub name_ja: String,
    pub name_en: String,
    pub jlpt_level: Option<u8>,
    pub description: String,
    /// 词性模式字节串 (例如 "VXX" 代表 动词+助动词+助动词)
    pub pos_pattern: String,
    /// 限制约束条件
    pub constraints: Vec<Constraint>,
}

pub struct GrammarMatcher {
    patterns: Vec<GrammarPattern>,
    ac: AhoCorasick,
}

/// 转换形态素词性为单一特征字节码
fn get_pos_byte(pos: &PosTag) -> u8 {
    if pos.major == "動詞" && pos.sub1 == "接尾" {
        return b'X'; // 动词性接尾辞在语法匹配上视同助动词 (Auxiliary Verb)
    }
    if pos.sub1 == "接尾" {
        return b'T'; // 其他接尾辞 (Tail)
    }
    match pos.major.as_str() {
        "動詞" => b'V', // Verb
        "名詞" => b'N', // Noun
        "形容詞" => b'A', // Adjective
        "助動詞" => b'X', // Auxiliary Verb
        "助詞" => b'P', // Particle
        "副詞" => b'D', // Adverb
        "連体詞" => b'R', // Pre-noun Adjectival
        "接続詞" => b'C', // Conjunction
        "感動詞" => b'I', // Interjection
        "接頭詞" => b'H', // Prefix
        _ => b'O', // Other
    }
}

impl GrammarMatcher {
    /// 构造语法匹配器并加载内置种子模式库
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // 内置高频 N1-N5 语法种子模式
        let patterns = vec![
            // 1. 使役受身 (Causative Passive) - VXX (动词+助动词+助动词)
            GrammarPattern {
                id: "causative_passive".to_string(),
                name_ja: "使役受身".to_string(),
                name_en: "Causative Passive".to_string(),
                jlpt_level: Some(3),
                description: "被动使役，表示被迫做某事 (如: 食べさせられる)".to_string(),
                pos_pattern: "VXX".to_string(),
                constraints: vec![
                    Constraint {
                        index: 1,
                        field: "base_form".to_string(),
                        values: vec!["せる".to_string(), "させる".to_string()],
                    },
                    Constraint {
                        index: 2,
                        field: "base_form".to_string(),
                        values: vec!["れる".to_string(), "られる".to_string()],
                    },
                ],
            },
            // 2. 使役 (Causative) - VX (动词+助动词)
            GrammarPattern {
                id: "causative".to_string(),
                name_ja: "使役".to_string(),
                name_en: "Causative".to_string(),
                jlpt_level: Some(4),
                description: "使役形，表示让/令某人做某事 (如: 食べさせる)".to_string(),
                pos_pattern: "VX".to_string(),
                constraints: vec![
                    Constraint {
                        index: 1,
                        field: "base_form".to_string(),
                        values: vec!["せる".to_string(), "させる".to_string()],
                    },
                ],
            },
            // 3. 受身/可能 (Passive/Potential) - VX (动词+助动词)
            GrammarPattern {
                id: "passive_potential".to_string(),
                name_ja: "受身・可能".to_string(),
                name_en: "Passive/Potential".to_string(),
                jlpt_level: Some(4),
                description: "被动、可能、尊他或自发表达 (如: 食べられる)".to_string(),
                pos_pattern: "VX".to_string(),
                constraints: vec![
                    Constraint {
                        index: 1,
                        field: "base_form".to_string(),
                        values: vec!["れる".to_string(), "られる".to_string()],
                    },
                ],
            },
            // 4. 过去否定 (Past Negative) - XX (助动词+助动词)
            GrammarPattern {
                id: "past_negative".to_string(),
                name_ja: "過去否定".to_string(),
                name_en: "Past Negative".to_string(),
                jlpt_level: Some(5),
                description: "过去否定形 (如: なかった)".to_string(),
                pos_pattern: "XX".to_string(),
                constraints: vec![
                    Constraint {
                        index: 0,
                        field: "base_form".to_string(),
                        values: vec!["ない".to_string()],
                    },
                    Constraint {
                        index: 1,
                        field: "base_form".to_string(),
                        values: vec!["た".to_string()],
                    },
                ],
            },
            // 5. 进行时/状态存续 (State Continuation) - VPV (动词+助词+动词)
            GrammarPattern {
                id: "te_iru".to_string(),
                name_ja: "〜ている".to_string(),
                name_en: "Progressive/State".to_string(),
                jlpt_level: Some(5),
                description: "动作正在进行或状态的持续 (如: 食べている)".to_string(),
                pos_pattern: "VPV".to_string(),
                constraints: vec![
                    Constraint {
                        index: 1,
                        field: "surface".to_string(),
                        values: vec!["て".to_string(), "で".to_string()],
                    },
                    Constraint {
                        index: 2,
                        field: "base_form".to_string(),
                        values: vec!["いる".to_string(), "おる".to_string()],
                    },
                ],
            },
            // 6. 想做某事 (Desire) - VX (动词+助动词)
            GrammarPattern {
                id: "desire_tai".to_string(),
                name_ja: "〜たい".to_string(),
                name_en: "Desire".to_string(),
                jlpt_level: Some(5),
                description: "第一人称想做某事 (如: 食べたい)".to_string(),
                pos_pattern: "VX".to_string(),
                constraints: vec![
                    Constraint {
                        index: 1,
                        field: "base_form".to_string(),
                        values: vec!["たい".to_string()],
                    },
                ],
            },
            // 7. 试图/将要 (Attempt to do) - VXPV (动词意向形+助动词+助词+动词)
            GrammarPattern {
                id: "volitional_to_suru".to_string(),
                name_ja: "〜ようとする".to_string(),
                name_en: "Attempt/About to".to_string(),
                jlpt_level: Some(3),
                description: "正要做某事，或试图做某事 (如: 食べようとする)".to_string(),
                pos_pattern: "VXPV".to_string(),
                constraints: vec![
                    Constraint {
                        index: 1,
                        field: "base_form".to_string(),
                        values: vec!["う".to_string(), "よう".to_string()],
                    },
                    Constraint {
                        index: 2,
                        field: "surface".to_string(),
                        values: vec!["と".to_string()],
                    },
                    Constraint {
                        index: 3,
                        field: "base_form".to_string(),
                        values: vec!["する".to_string()],
                    },
                ],
            },
        ];

        // The seed is replaceable in development/test builds and by the desktop
        // data directory loader. Invalid files are rejected without taking down NLP.
        let patterns = std::env::var("KOTOCLIP_GRAMMAR_PATTERNS")
            .ok()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|content| serde_json::from_str::<Vec<GrammarPattern>>(&content).ok())
            .filter(|rules| validate_patterns(rules))
            .unwrap_or(patterns);

        // Additional patterns cover the current reader examples.
        let mut patterns = patterns;
        patterns.extend(example_patterns());

        // 获取所有的 pos_patterns 供 Aho-Corasick 构建
        let ac_patterns: Vec<String> = patterns.iter().map(|p| p.pos_pattern.clone()).collect();
        let ac = AhoCorasick::new(ac_patterns)?;

        Ok(Self { patterns, ac })
    }

    /// 在给定的文节列表中寻找并标注语法模式
    pub fn match_patterns(&self, bunsetsus: &mut [Bunsetsu]) {
        if bunsetsus.is_empty() {
            return;
        }

        // 临时存储所有命中的语法标签：(目标文节索引, 语法标签)
        let mut matched_tags = Vec::new();

        {
            // 1. 展平提取所有形态素，并映射形态素索引到文节索引
            let mut flat_morphemes: Vec<&Morpheme> = Vec::new();
            let mut morph_to_bunsetsu = Vec::new(); // flat_morphemes 索引 -> bunsetsus 索引

            for (b_idx, bunsetsu) in bunsetsus.iter().enumerate() {
                for m in &bunsetsu.morphemes {
                    flat_morphemes.push(m);
                    morph_to_bunsetsu.push(b_idx);
                }
            }

            // 2. 将形态素序列映射为特征字节流
            let haystack: Vec<u8> = flat_morphemes.iter().map(|m| get_pos_byte(&m.pos)).collect();

            // 3. 使用 Aho-Corasick 进行模式快速定位
            // Multiple grammar patterns commonly share a prefix (for example VX and VXX).
            // Inspect overlapping candidates so a shorter match cannot hide a longer one.
            for mat in self.ac.find_overlapping_iter(&haystack) {
                let pattern_idx = mat.pattern().as_usize();
                let start = mat.start();
                let end = mat.end();

                let pattern = &self.patterns[pattern_idx];
                let sub_morphemes = &flat_morphemes[start..end];

                // 4. 精细约束二次校验
                if verify_constraints(sub_morphemes, &pattern.constraints) {
                    // 校验通过，准备写入到命中的第一个文节中
                    let target_bunsetsu_idx = morph_to_bunsetsu[start];
                    
                    let tag = GrammarTag {
                        pattern_id: pattern.id.clone(),
                        name_ja: pattern.name_ja.clone(),
                        name_en: pattern.name_en.clone(),
                        jlpt_level: pattern.jlpt_level,
                        description: pattern.description.clone(),
                        morpheme_range: (start, end),
                        char_range: (sub_morphemes.first().unwrap().char_range.0, sub_morphemes.last().unwrap().char_range.1),
                    };

                    matched_tags.push((target_bunsetsu_idx, tag));
                }
            }
        } // 局部块结束，flat_morphemes 占用的 bunsetsus 不可变借用生命周期在此安全释放

        // 5. 应用语法标签至对应文节上，此时可以安全地获取可变借用
        for (target_bunsetsu_idx, tag) in matched_tags {
            let bunsetsu = &mut bunsetsus[target_bunsetsu_idx];
            // 避免重复写入同一个语法标签
            if !bunsetsu.grammar_tags.iter().any(|t| t.pattern_id == tag.pattern_id) {
                bunsetsu.grammar_tags.push(tag);
            }
        }
    }
}

fn validate_patterns(patterns: &[GrammarPattern]) -> bool {
    let mut ids = HashSet::new();
    patterns.iter().all(|pattern| {
        !pattern.id.is_empty() && !pattern.name_ja.is_empty() && ids.insert(pattern.id.clone())
            && pattern.pos_pattern.bytes().all(|byte| b"VNAXPDRCIOHT".contains(&byte))
            && pattern.constraints.iter().all(|constraint| constraint.index < pattern.pos_pattern.len() && !constraint.values.is_empty())
    })
}

fn example_patterns() -> Vec<GrammarPattern> {
    let make = |id: &str, name: &str, pattern: &str, index: usize, values: &[&str], level: u8| GrammarPattern {
        id: id.to_string(), name_ja: name.to_string(), name_en: name.to_string(), jlpt_level: Some(level),
        description: format!("例句语法：{}", name), pos_pattern: pattern.to_string(),
        constraints: vec![Constraint { index, field: "base_form".to_string(), values: values.iter().map(|v| (*v).to_string()).collect() }],
    };
    vec![
        make("te_kuru", "〜てくる", "VPV", 2, &["くる", "来る"], 3),
        make("te_yaru", "〜てやる", "VPV", 2, &["やる"], 3),
        make("te_oku", "〜ておく", "VPV", 2, &["おく"], 3),
        make("tsumori", "〜つもりだ", "NPD", 1, &["つもり"], 4),
        make("nagaramo", "〜ながら（も）", "VP", 1, &["ながら"], 3),
        make("negative_n", "〜ん", "VX", 1, &["ぬ", "ん"], 3),
    ]
}

/// 校验约束条件是否符合
fn verify_constraints(morphemes: &[&Morpheme], constraints: &[Constraint]) -> bool {
    for c in constraints {
        if c.index >= morphemes.len() {
            return false;
        }
        let m = morphemes[c.index];
        let val_to_check = match c.field.as_str() {
            "surface" => &m.surface,
            "base_form" => &m.base_form,
            "pos_major" => &m.pos.major,
            _ => continue,
        };
        if !c.values.contains(val_to_check) {
            return false;
        }
    }
    true
}
