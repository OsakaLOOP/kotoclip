//! 阅读器热路径使用的紧凑 IPC 表示。
//!
//! 常规 `AnnotatedToken` 为编辑、导出和独立命令保留可读的嵌套结构；整页分析
//! 则有大量重复的词性、语法和表达字符串。这里将字符串提升为共享表，既保持
//! 前端可无损恢复原模型，也显著缩小 JSON 序列化与 WebView 解析负担。

use crate::models::{
    AnnotatedToken, Bunsetsu, ExpressionAnnotation, GrammarTag, HeadWord, Morpheme, PosTag,
    WordFormationAnnotation, WordFormationCapture,
};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct CompactAnalysis {
    pub s: Vec<String>,
    pub t: Vec<CompactToken>,
}

#[derive(Serialize)]
pub struct CompactToken {
    pub b: CompactBunsetsu,
    pub n: f32,
    pub k: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub x: Vec<CompactExpression>,
    pub d: u32,
}

#[derive(Serialize)]
pub struct CompactBunsetsu {
    pub m: Vec<CompactMorpheme>,
    pub s: u32,
    pub h: CompactHeadWord,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub g: Vec<CompactGrammarTag>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub w: Vec<CompactWordFormation>,
    pub c: (usize, usize),
}

#[derive(Serialize)]
pub struct CompactMorpheme {
    pub s: u32,
    pub p: [u32; 4],
    pub b: u32,
    pub r: u32,
    pub t: u32,
    pub f: u32,
    pub c: (usize, usize),
}

#[derive(Serialize)]
pub struct CompactHeadWord {
    pub s: u32,
    pub b: u32,
    pub r: u32,
    pub p: [u32; 4],
}

#[derive(Serialize)]
pub struct CompactGrammarTag {
    pub i: u32,
    pub j: u32,
    pub e: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub l: Option<u8>,
    pub d: u32,
    pub m: (usize, usize),
    pub c: (usize, usize),
}

#[derive(Serialize)]
pub struct CompactWordFormation {
    pub i: u32,
    pub k: u32,
    pub s: u32,
    pub b: u32,
    pub r: u32,
    pub o: [u32; 4],
    pub m: (usize, usize),
    pub c: (usize, usize),
    pub h: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub p: Vec<CompactWordFormationCapture>,
    pub q: u8,
}

#[derive(Serialize)]
pub struct CompactWordFormationCapture {
    pub n: u32,
    pub s: u32,
    pub m: (usize, usize),
    pub c: (usize, usize),
}

#[derive(Serialize)]
pub struct CompactExpression {
    pub m: u32,
    pub i: i64,
    pub l: u32,
    pub d: u32,
    pub o: u32,
    pub t: u32,
    pub p: i32,
    pub b: u32,
    pub c: f32,
    pub q: u32,
    pub r: (usize, usize),
    pub a: (usize, usize),
    pub s: u32,
}

struct StringTable {
    values: Vec<String>,
    indices: HashMap<String, u32>,
}

impl StringTable {
    fn intern(&mut self, value: &str) -> u32 {
        if let Some(index) = self.indices.get(value) {
            return *index;
        }
        let index = self.values.len() as u32;
        let owned = value.to_owned();
        self.indices.insert(owned.clone(), index);
        self.values.push(owned);
        index
    }

    fn position(&mut self, value: &PosTag) -> [u32; 4] {
        [
            self.intern(&value.major),
            self.intern(&value.sub1),
            self.intern(&value.sub2),
            self.intern(&value.sub3),
        ]
    }

    fn morpheme(&mut self, value: &Morpheme) -> CompactMorpheme {
        CompactMorpheme {
            s: self.intern(&value.surface),
            p: self.position(&value.pos),
            b: self.intern(&value.base_form),
            r: self.intern(&value.reading),
            t: self.intern(&value.conjugation_type),
            f: self.intern(&value.conjugation_form),
            c: value.char_range,
        }
    }

    fn head_word(&mut self, value: &HeadWord) -> CompactHeadWord {
        CompactHeadWord {
            s: self.intern(&value.surface),
            b: self.intern(&value.base_form),
            r: self.intern(&value.reading),
            p: self.position(&value.pos),
        }
    }

    fn grammar_tag(&mut self, value: &GrammarTag) -> CompactGrammarTag {
        CompactGrammarTag {
            i: self.intern(&value.pattern_id),
            j: self.intern(&value.name_ja),
            e: self.intern(&value.name_en),
            l: value.jlpt_level,
            d: self.intern(&value.description),
            m: value.morpheme_range,
            c: value.char_range,
        }
    }

    fn word_formation_capture(&mut self, value: &WordFormationCapture) -> CompactWordFormationCapture {
        CompactWordFormationCapture {
            n: self.intern(&value.name),
            s: self.intern(&value.surface),
            m: value.morpheme_range,
            c: value.char_range,
        }
    }

    fn word_formation(&mut self, value: &WordFormationAnnotation) -> CompactWordFormation {
        CompactWordFormation {
            i: self.intern(&value.rule_id),
            k: self.intern(&value.category),
            s: self.intern(&value.surface),
            b: self.intern(&value.base_form),
            r: self.intern(&value.reading),
            o: self.position(&value.output_pos),
            m: value.morpheme_range,
            c: value.char_range,
            h: value.head_morpheme,
            p: value.captures.iter().map(|item| self.word_formation_capture(item)).collect(),
            q: value.confidence,
        }
    }

    fn bunsetsu(&mut self, value: &Bunsetsu) -> CompactBunsetsu {
        CompactBunsetsu {
            m: value
                .morphemes
                .iter()
                .map(|item| self.morpheme(item))
                .collect(),
            s: self.intern(&value.surface),
            h: self.head_word(&value.head_word),
            g: value
                .grammar_tags
                .iter()
                .map(|item| self.grammar_tag(item))
                .collect(),
            w: value.word_formations.iter().map(|item| self.word_formation(item)).collect(),
            c: value.char_range,
        }
    }

    fn expression(&mut self, value: &ExpressionAnnotation) -> CompactExpression {
        CompactExpression {
            m: self.intern(&value.match_id),
            i: value.rule_id,
            l: self.intern(&value.label),
            d: self.intern(&value.description),
            o: self.intern(&value.origin),
            t: self.intern(&value.expression_type),
            p: value.priority,
            b: self.intern(&value.boundary_effect),
            c: value.confidence,
            q: self.intern(&value.position),
            r: value.token_range,
            a: value.char_range,
            s: self.intern(&value.surface),
        }
    }
}

impl From<&[AnnotatedToken]> for CompactAnalysis {
    fn from(tokens: &[AnnotatedToken]) -> Self {
        let mut strings = StringTable {
            values: Vec::new(),
            indices: HashMap::new(),
        };
        let t = tokens
            .iter()
            .map(|token| CompactToken {
                b: strings.bunsetsu(&token.bunsetsu),
                n: token.novelty_score,
                k: token.is_known,
                r: token
                    .inference_reason
                    .as_deref()
                    .map(|value| strings.intern(value)),
                x: token
                    .expressions
                    .iter()
                    .map(|item| strings.expression(item))
                    .collect(),
                d: strings.intern(&token.display_class),
            })
            .collect();
        Self {
            s: strings.values,
            t,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CompactAnalysis;
    use crate::models::{AnnotatedToken, Bunsetsu, HeadWord, PosTag};

    #[test]
    fn shares_repeated_display_strings() {
        let position = PosTag {
            major: "名詞".to_string(),
            sub1: "一般".to_string(),
            sub2: "*".to_string(),
            sub3: "*".to_string(),
        };
        let token = AnnotatedToken {
            bunsetsu: Bunsetsu {
                morphemes: Vec::new(),
                surface: "語".to_string(),
                head_word: HeadWord {
                    surface: "語".to_string(),
                    base_form: "語".to_string(),
                    reading: "ゴ".to_string(),
                    pos: position,
                },
                grammar_tags: Vec::new(),
                word_formations: Vec::new(),
                char_range: (0, 1),
            },
            novelty_score: 1.0,
            is_selected: false,
            is_known: false,
            inference_reason: None,
            expressions: Vec::new(),
            display_class: "content".to_string(),
        };
        let compact = CompactAnalysis::from([token.clone(), token].as_slice());
        assert_eq!(
            compact
                .s
                .iter()
                .filter(|value| value.as_str() == "語")
                .count(),
            1
        );
    }
}
