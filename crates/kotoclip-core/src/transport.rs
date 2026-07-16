//! 阅读器热路径使用的紧凑 IPC 表示。
//!
//! 常规 `AnnotatedToken` 为编辑、导出和独立命令保留可读的嵌套结构；整页分析
//! 则有大量重复的词性、语法和表达字符串。这里将字符串提升为共享表，既保持
//! 前端可无损恢复原模型，也显著缩小 JSON 序列化与 WebView 解析负担。

use crate::models::{
    AnnotatedToken, Bunsetsu, BunsetsuFunctionAnnotation, DictionaryEntryRef,
    DictionaryLexicalUnitAnnotation, ExpressionAnnotation, GrammarCapture, GrammarContentBlock,
    GrammarDictionaryTarget, GrammarSenseCandidate, GrammarTag, HeadWord, Morpheme, PosTag,
    MorphologyChain, MorphologyOperator, ResolvedGrammarExplanation, WordFormationAnnotation,
    WordFormationCapture,
};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct CompactAnalysis {
    pub s: Vec<String>,
    pub t: Vec<CompactToken>,
}

#[derive(Serialize)]
pub struct CompactAnalysisPatch {
    /// 本次新增字符串在会话字符串表中的起始下标。
    pub b: u32,
    /// 仅包含本次编码新增的字符串；Token 内索引始终指向会话完整字符串表。
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
    pub y: Vec<CompactMorphologyChain>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub w: Vec<CompactWordFormation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub v: Vec<CompactLexicalUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub u: Option<CompactBunsetsuFunction>,
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
pub struct CompactMorphologyChain {
    pub i: u32,
    pub a: usize,
    pub b: (usize, usize),
    pub m: (usize, usize),
    pub c: (usize, usize),
    pub r: u32,
    pub l: u32,
    pub s: u32,
    pub d: u32,
    pub p: u32,
    pub q: u32,
    pub x: Vec<(usize, usize)>,
    pub o: Vec<CompactMorphologyOperator>,
    pub f: Vec<u32>,
    pub e: Vec<u32>,
}

#[derive(Serialize)]
pub struct CompactMorphologyOperator {
    pub i: u32,
    pub k: u32,
    pub m: (usize, usize),
    pub c: (usize, usize),
    pub o: u32,
    pub q: u32,
    pub n: u8,
    pub e: Vec<u32>,
    pub a: Vec<u32>,
    pub l: u32,
    pub d: u32,
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
    pub o: u32,
    pub q: u32,
    pub k: u32,
    pub s: u32,
    pub b: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub z: Vec<(usize, usize)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub a: Vec<CompactGrammarSenseCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<CompactGrammarExplanation>,
}

#[derive(Serialize)]
pub struct CompactGrammarSenseCandidate {
    pub i: u32,
    pub l: u32,
    pub c: u8,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub e: Vec<u32>,
}

#[derive(Serialize)]
pub struct CompactGrammarCapture {
    pub n: u32,
    pub s: u32,
    pub b: u32,
    pub m: (usize, usize),
    pub c: (usize, usize),
}

#[derive(Serialize)]
pub struct CompactGrammarBlock {
    pub k: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub l: Option<u32>,
    pub t: u32,
}

#[derive(Serialize)]
pub struct CompactGrammarDictionaryTarget {
    pub l: u32,
    pub b: u32,
    pub r: u32,
    pub c: (usize, usize),
}

#[derive(Serialize)]
pub struct CompactGrammarExplanation {
    pub s: u32,
    pub o: u32,
    pub c: u32,
    pub t: u32,
    pub m: u32,
    pub f: u32,
    pub n: u32,
    pub a: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<CompactGrammarSenseCandidate>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub v: Vec<CompactGrammarSenseCandidate>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub p: Vec<CompactGrammarCapture>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub h: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub d: Vec<CompactGrammarBlock>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub e: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub g: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub j: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub w: Vec<CompactGrammarDictionaryTarget>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub i: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub q: Vec<u32>,
    pub po: u32,
    pub pa: u32,
    pub pd: u32,
    pub pv: u32,
    pub rv: u32,
    pub vrs: u32,
    pub u: u32,
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
pub struct CompactLexicalUnit {
    pub s: u32,
    pub b: u32,
    pub r: u32,
    pub o: [u32; 4],
    pub m: (usize, usize),
    pub c: (usize, usize),
    pub h: usize,
    pub k: u32,
    pub d: Vec<CompactDictionaryEntryRef>,
    pub a: Vec<u32>,
    pub q: u8,
    pub e: Vec<u32>,
}

#[derive(Serialize)]
pub struct CompactDictionaryEntryRef {
    pub k: u32,
    pub d: u32,
    pub h: u32,
    pub f: u32,
    pub m: u32,
    pub r: Vec<u32>,
}

#[derive(Serialize)]
pub struct CompactBunsetsuFunction {
    pub f: u32,
    pub c: u8,
    pub e: Vec<u32>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub z: Vec<(usize, usize)>,
    pub s: u32,
}

#[derive(Default)]
struct StringTable {
    values: Vec<String>,
    indices: HashMap<String, u32>,
}

#[derive(Default)]
pub struct CompactEncoder {
    strings: StringTable,
}

impl CompactEncoder {
    pub fn encode_patch(&mut self, tokens: &[AnnotatedToken]) -> CompactAnalysisPatch {
        let base = self.strings.values.len();
        let t = encode_tokens(&mut self.strings, tokens);
        CompactAnalysisPatch {
            b: base as u32,
            s: self.strings.values[base..].to_vec(),
            t,
        }
    }
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

    fn morphology_chain(&mut self, value: &MorphologyChain) -> CompactMorphologyChain {
        CompactMorphologyChain {
            i: self.intern(&value.chain_id),
            a: value.anchor_morpheme,
            b: value.anchor_range,
            m: value.morpheme_range,
            c: value.char_range,
            r: self.intern(match value.role {
                crate::models::MorphologyChainRole::Lexical => "lexical",
                crate::models::MorphologyChainRole::Functional => "functional",
            }),
            l: self.intern(&value.base_lexeme),
            s: self.intern(&value.surface_form),
            d: self.intern(&value.dictionary_form),
            p: self.intern(&value.lemma_form),
            q: self.intern(&value.lookup_form),
            x: value.source_ranges.clone(),
            o: value
                .operators
                .iter()
                .map(|item| self.morphology_operator(item))
                .collect(),
            f: value
                .connection_forms
                .iter()
                .map(|item| self.intern(item))
                .collect(),
            e: value
                .evidence
                .iter()
                .map(|item| self.intern(item))
                .collect(),
        }
    }

    fn morphology_operator(&mut self, value: &MorphologyOperator) -> CompactMorphologyOperator {
        CompactMorphologyOperator {
            i: self.intern(&value.operator_id),
            k: self.intern(&value.kind),
            m: value.source_morpheme_range,
            c: value.char_range,
            o: self.intern(&value.output_state),
            q: self.intern(&value.concept_id),
            n: value.confidence,
            e: value
                .evidence
                .iter()
                .map(|item| self.intern(item))
                .collect(),
            a: value
                .candidates
                .iter()
                .map(|item| self.intern(item))
                .collect(),
            l: self.intern(&value.label),
            d: self.intern(&value.description),
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
            o: self.intern(&value.occurrence_id),
            q: self.intern(&value.concept_id),
            k: self.intern(match &value.occurrence_kind {
                crate::models::GrammarOccurrenceKind::MorphologyFeature => "morphology_feature",
                crate::models::GrammarOccurrenceKind::FunctionalMorpheme => "functional_morpheme",
                crate::models::GrammarOccurrenceKind::GrammarConstruction => "grammar_construction",
                crate::models::GrammarOccurrenceKind::BunsetsuFunction => "bunsetsu_function",
                crate::models::GrammarOccurrenceKind::CorrelativeGrammar => "correlative_grammar",
                crate::models::GrammarOccurrenceKind::Unknown => "unknown",
            }),
            s: self.intern(match &value.status {
                crate::models::GrammarOccurrenceStatus::Accepted => "accepted",
                crate::models::GrammarOccurrenceStatus::Pending => "pending",
                crate::models::GrammarOccurrenceStatus::Rejected => "rejected",
                crate::models::GrammarOccurrenceStatus::Unknown => "unknown",
            }),
            b: value.show_badge,
            z: value.display_ranges.clone(),
            y: value.selected_sense_id.as_ref().map(|item| self.intern(item)),
            a: value
                .sense_candidates
                .iter()
                .map(|item| self.grammar_sense_candidate(item))
                .collect(),
            x: value
                .explanation
                .as_ref()
                .map(|item| self.grammar_explanation(item)),
        }
    }

    fn grammar_sense_candidate(&mut self, value: &GrammarSenseCandidate) -> CompactGrammarSenseCandidate {
        CompactGrammarSenseCandidate {
            i: self.intern(&value.sense_id),
            l: self.intern(&value.label),
            c: value.confidence,
            e: value.evidence.iter().map(|item| self.intern(item)).collect(),
        }
    }

    fn grammar_capture(&mut self, value: &GrammarCapture) -> CompactGrammarCapture {
        CompactGrammarCapture {
            n: self.intern(&value.name),
            s: self.intern(&value.surface),
            b: self.intern(&value.base_form),
            m: value.morpheme_range,
            c: value.char_range,
        }
    }

    fn grammar_block(&mut self, value: &GrammarContentBlock) -> CompactGrammarBlock {
        CompactGrammarBlock {
            k: self.intern(&value.kind),
            l: value.label.as_ref().map(|item| self.intern(item)),
            t: self.intern(&value.text),
        }
    }

    fn grammar_dictionary_target(&mut self, value: &GrammarDictionaryTarget) -> CompactGrammarDictionaryTarget {
        CompactGrammarDictionaryTarget {
            l: self.intern(&value.label),
            b: self.intern(&value.base_form),
            r: self.intern(&value.reading),
            c: value.char_range,
        }
    }

    fn grammar_explanation(&mut self, value: &ResolvedGrammarExplanation) -> CompactGrammarExplanation {
        CompactGrammarExplanation {
            s: self.intern(&value.status),
            o: self.intern(&value.occurrence_summary),
            c: self.intern(&value.concept_id),
            t: self.intern(&value.title),
            m: self.intern(&value.compact_summary),
            f: self.intern(&value.function_summary),
            n: self.intern(&value.connection),
            a: self.intern(&value.actual_form),
            y: value.selected_sense.as_ref().map(|item| self.grammar_sense_candidate(item)),
            v: value.alternative_senses.iter().map(|item| self.grammar_sense_candidate(item)).collect(),
            p: value.bound_captures.iter().map(|item| self.grammar_capture(item)).collect(),
            h: value.morphology_chain.iter().map(|item| self.intern(item)).collect(),
            d: value.content_blocks.iter().map(|item| self.grammar_block(item)).collect(),
            e: value.evidence.iter().map(|item| self.intern(item)).collect(),
            g: value.related_concept_ids.iter().map(|item| self.intern(item)).collect(),
            j: value.contrast_concept_ids.iter().map(|item| self.intern(item)).collect(),
            w: value.dictionary_targets.iter().map(|item| self.grammar_dictionary_target(item)).collect(),
            i: value.available_depths.iter().map(|item| self.intern(item)).collect(),
            q: value.source_refs.iter().map(|item| self.intern(item)).collect(),
            po: self.intern(&value.provenance.origin),
            pa: self.intern(&value.provenance.author),
            pd: self.intern(&value.provenance.date),
            pv: self.intern(&value.provenance.version),
            rv: self.intern(&value.review_status),
            vrs: value.content_version,
            u: self.intern(&value.audit_status),
        }
    }

    fn word_formation_capture(
        &mut self,
        value: &WordFormationCapture,
    ) -> CompactWordFormationCapture {
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
            p: value
                .captures
                .iter()
                .map(|item| self.word_formation_capture(item))
                .collect(),
            q: value.confidence,
        }
    }

    fn dictionary_ref(&mut self, value: &DictionaryEntryRef) -> CompactDictionaryEntryRef {
        CompactDictionaryEntryRef {
            k: self.intern(&value.entry_key),
            d: self.intern(&value.dict_name),
            h: self.intern(&value.headword),
            f: self.intern(&value.matched_form),
            m: self.intern(&value.match_type),
            r: value
                .readings
                .iter()
                .map(|item| self.intern(item))
                .collect(),
        }
    }

    fn lexical_unit(&mut self, value: &DictionaryLexicalUnitAnnotation) -> CompactLexicalUnit {
        CompactLexicalUnit {
            s: self.intern(&value.surface),
            b: self.intern(&value.base_form),
            r: self.intern(&value.reading),
            o: self.position(&value.output_pos),
            m: value.morpheme_range,
            c: value.char_range,
            h: value.head_morpheme,
            k: self.intern(&value.lexical_shape),
            d: value
                .dictionary_refs
                .iter()
                .map(|item| self.dictionary_ref(item))
                .collect(),
            a: value
                .reading_candidates
                .iter()
                .map(|item| self.intern(item))
                .collect(),
            q: value.confidence,
            e: value
                .evidence
                .iter()
                .map(|item| self.intern(item))
                .collect(),
        }
    }

    fn bunsetsu_function(&mut self, value: &BunsetsuFunctionAnnotation) -> CompactBunsetsuFunction {
        CompactBunsetsuFunction {
            f: self.intern(match value.function {
                crate::models::BunsetsuFunction::Predicate => "predicate",
                crate::models::BunsetsuFunction::CasePhrase => "case_phrase",
                crate::models::BunsetsuFunction::Adnominal => "adnominal",
                crate::models::BunsetsuFunction::Adverbial => "adverbial",
                crate::models::BunsetsuFunction::Conjunctive => "conjunctive",
                crate::models::BunsetsuFunction::Nominal => "nominal",
                crate::models::BunsetsuFunction::Standalone => "standalone",
                crate::models::BunsetsuFunction::Unknown => "unknown",
            }),
            c: value.confidence,
            e: value
                .evidence
                .iter()
                .map(|item| self.intern(item))
                .collect(),
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
            y: value
                .morphology
                .chains
                .iter()
                .map(|item| self.morphology_chain(item))
                .collect(),
            w: value
                .word_formations
                .iter()
                .map(|item| self.word_formation(item))
                .collect(),
            v: value
                .lexical_units
                .iter()
                .map(|item| self.lexical_unit(item))
                .collect(),
            u: value
                .function
                .as_ref()
                .map(|item| self.bunsetsu_function(item)),
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
            z: value.matched_ranges.clone(),
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
        let t = encode_tokens(&mut strings, tokens);
        Self {
            s: strings.values,
            t,
        }
    }
}

fn encode_tokens(strings: &mut StringTable, tokens: &[AnnotatedToken]) -> Vec<CompactToken> {
    tokens
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
        .collect()
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
                morphology: Default::default(),
                grammar_occurrences: Vec::new(),
                functional_residuals: Vec::new(),
                word_formations: Vec::new(),
                lexical_units: Vec::new(),
                function: None,
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
