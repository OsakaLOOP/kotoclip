pub mod bunsetsu;
pub mod candidates;
pub mod expressions;
pub mod grammar;
pub mod lexical;
pub mod morpheme;
pub mod morphology;
pub mod restore;
pub mod ruby;
pub mod word_formation;

use crate::analysis_progress::{AnalysisCancelled, AnalysisPhase, AnalysisProgress};
use crate::models::AnnotatedToken;
use crate::performance::TimingCollector;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

/// NLP 处理引擎管线容器
pub struct Pipeline {
    morpheme_analyzer: morpheme::MorphemeAnalyzer,
    grammar_matcher: grammar::GrammarMatcher,
    word_formation_matcher: word_formation::WordFormationMatcher,
    bunsetsu_analyzer: bunsetsu::BunsetsuAnalyzer,
}

/// 构词审计专用输出；不写画像，也不运行表达层。
pub struct WordFormationSegment {
    pub char_range: (usize, usize),
    pub morphemes: Vec<crate::models::Morpheme>,
    pub result: word_formation::WordFormationMatchResult,
}

pub struct DictionaryLexicalSegment {
    pub char_range: (usize, usize),
    pub morphemes: Vec<crate::models::Morpheme>,
    pub result: lexical::DictionaryLexicalMatchResult,
}

fn cancellation_requested(check: Option<&dyn Fn() -> bool>) -> bool {
    check.is_some_and(|check| check())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentType {
    Content,
    Punctuation,
    LineBreak,
}

#[derive(Debug, Clone, Copy)]
struct TextSegment {
    seg_type: SegmentType,
    start_char_idx: usize,
    end_char_idx: usize,
}

struct ContentStage {
    segment: TextSegment,
    annotation_range: (usize, usize),
    morphemes: Vec<crate::models::Morpheme>,
    formations: Vec<word_formation::AcceptedWordFormation>,
    lexical_candidates: Option<lexical::DictionaryLexicalCandidates>,
}

enum SegmentStage {
    Content(ContentStage),
    Literal(TextSegment),
}

fn is_punctuation_or_symbol(c: char) -> bool {
    if c.is_ascii_punctuation() {
        return true;
    }
    if c == '○' || c == '～' || c == '〜' || c == '\u{3000}' || c == ' ' {
        return false;
    }
    let val = c as u32;
    if val >= 0x3000 && val <= 0x303F {
        return val != 0x3005 && val != 0x3006 && val != 0x3007;
    }
    if (val >= 0x2000 && val <= 0x206F)
        || (val >= 0x2E00 && val <= 0x2E7F)
        || (val >= 0xFE30 && val <= 0xFE6F)
    {
        return true;
    }
    if val >= 0xFF00 && val <= 0xFFEF {
        if (val >= 0xFF10 && val <= 0xFF19)
            || (val >= 0xFF21 && val <= 0xFF3A)
            || (val >= 0xFF41 && val <= 0xFF5A)
            || (val >= 0xFF66 && val <= 0xFF9F)
        {
            return false;
        }
        return true;
    }
    if val >= 0x2190 && val <= 0x2BFF {
        return true;
    }
    false
}

fn segment_text(chars: &[char]) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut i = 0;
    let n = chars.len();

    while i < n {
        let c = chars[i];
        if c == '\n' || c == '\r' {
            let start = i;
            if c == '\r' && i + 1 < n && chars[i + 1] == '\n' {
                i += 2;
            } else {
                i += 1;
            }
            segments.push(TextSegment {
                seg_type: SegmentType::LineBreak,
                start_char_idx: start,
                end_char_idx: i,
            });
        } else if is_punctuation_or_symbol(c) {
            let start = i;
            let first_char = c;
            i += 1;
            while i < n
                && is_punctuation_or_symbol(chars[i])
                && chars[i] != '\n'
                && chars[i] != '\r'
            {
                let next_char = chars[i];
                let can_merge = next_char == first_char
                    || (first_char == '！' && next_char == '？')
                    || (first_char == '？' && next_char == '！')
                    || (first_char == '!' && next_char == '?')
                    || (first_char == '?' && next_char == '!');
                if can_merge {
                    i += 1;
                } else {
                    break;
                }
            }
            segments.push(TextSegment {
                seg_type: SegmentType::Punctuation,
                start_char_idx: start,
                end_char_idx: i,
            });
        } else {
            let start = i;
            i += 1;
            while i < n
                && chars[i] != '\n'
                && chars[i] != '\r'
                && !is_punctuation_or_symbol(chars[i])
            {
                i += 1;
            }
            segments.push(TextSegment {
                seg_type: SegmentType::Content,
                start_char_idx: start,
                end_char_idx: i,
            });
        }
    }
    segments
}

fn literal_token(chars: &[char], segment: TextSegment) -> AnnotatedToken {
    let surface: String = chars[segment.start_char_idx..segment.end_char_idx]
        .iter()
        .collect();
    let (major, display_class) = if segment.seg_type == SegmentType::Punctuation {
        ("記号", "punctuation")
    } else {
        ("改行", "line_break")
    };
    let pos = crate::models::PosTag {
        major: major.to_string(),
        sub1: "*".to_string(),
        sub2: "*".to_string(),
        sub3: "*".to_string(),
    };
    let morpheme = crate::models::Morpheme {
        surface: surface.clone(),
        pos: pos.clone(),
        base_form: surface.clone(),
        reading: String::new(),
        conjugation_type: "*".to_string(),
        conjugation_form: "*".to_string(),
        char_range: (segment.start_char_idx, segment.end_char_idx),
    };
    AnnotatedToken {
        bunsetsu: crate::models::Bunsetsu {
            morphemes: vec![morpheme],
            surface: surface.clone(),
            head_word: crate::models::HeadWord {
                surface: surface.clone(),
                base_form: surface,
                reading: String::new(),
                pos,
            },
            grammar_tags: Vec::new(),
            morphology: Default::default(),
            grammar_occurrences: Vec::new(),
            functional_residuals: Vec::new(),
            word_formations: Vec::new(),
            lexical_units: Vec::new(),
            function: None,
            char_range: (segment.start_char_idx, segment.end_char_idx),
        },
        novelty_score: 0.0,
        is_selected: false,
        is_known: true,
        inference_reason: None,
        expressions: Vec::new(),
        display_class: display_class.to_string(),
    }
}

impl Pipeline {
    /// 从指定字典文件路径初始化 NLP 管线
    pub fn new<P: AsRef<Path>>(dict_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let morpheme_analyzer = morpheme::MorphemeAnalyzer::new(dict_path)?;
        let grammar_matcher = grammar::GrammarMatcher::new()?;
        let word_formation_matcher = word_formation::WordFormationMatcher::new()?;
        lexical::validate_catalog()?;
        let bunsetsu_analyzer = bunsetsu::BunsetsuAnalyzer::new()?;
        Ok(Self {
            morpheme_analyzer,
            grammar_matcher,
            word_formation_matcher,
            bunsetsu_analyzer,
        })
    }

    /// 执行完整的 NLP 管线：形態素解析 -> 文節组块 -> 语法匹配 -> 辞書形还原 (应用自定义合并规则)
    pub fn process(&self, text: &str, merge_rules: &[Vec<String>]) -> Vec<AnnotatedToken> {
        self.process_with_progress(text, merge_rules, &mut |_| {})
    }

    pub fn process_with_dictionary(
        &self,
        text: &str,
        merge_rules: &[Vec<String>],
        dictionary: &crate::dictionary::lookup::DictionaryEngine,
    ) -> Vec<AnnotatedToken> {
        self.process_internal(text, merge_rules, &mut |_| {}, None, Some(dictionary), None)
    }

    pub fn inspect_dictionary_lexical_units(
        &self,
        text: &str,
        dictionary: &crate::dictionary::lookup::DictionaryEngine,
    ) -> Vec<DictionaryLexicalSegment> {
        let prepared = ruby::prepare_text(text);
        let prepared_chars: Vec<char> = prepared.text.chars().collect();
        segment_text(&prepared_chars)
            .into_iter()
            .filter(|segment| segment.seg_type == SegmentType::Content)
            .filter_map(|segment| {
                let segment_text: String = prepared_chars
                    [segment.start_char_idx..segment.end_char_idx]
                    .iter()
                    .collect();
                if segment_text.is_empty() {
                    return None;
                }
                let mut morphemes = self.morpheme_analyzer.analyze(&segment_text);
                for morpheme in &mut morphemes {
                    morpheme.char_range.0 += segment.start_char_idx;
                    morpheme.char_range.1 += segment.start_char_idx;
                }
                let formations = self.word_formation_matcher.match_morphemes(&morphemes);
                Some(DictionaryLexicalSegment {
                    char_range: (segment.start_char_idx, segment.end_char_idx),
                    result: lexical::match_dictionary_lexical_units(
                        &morphemes,
                        dictionary,
                        &formations.accepted,
                    ),
                    morphemes,
                })
            })
            .collect()
    }

    pub fn inspect_word_formations(&self, text: &str) -> Vec<WordFormationSegment> {
        let prepared = ruby::prepare_text(text);
        let prepared_chars: Vec<char> = prepared.text.chars().collect();
        segment_text(&prepared_chars)
            .into_iter()
            .filter(|segment| segment.seg_type == SegmentType::Content)
            .filter_map(|segment| {
                let segment_text: String = prepared_chars
                    [segment.start_char_idx..segment.end_char_idx]
                    .iter()
                    .collect();
                if segment_text.is_empty() {
                    return None;
                }
                let mut morphemes = self.morpheme_analyzer.analyze(&segment_text);
                for morpheme in &mut morphemes {
                    morpheme.char_range.0 += segment.start_char_idx;
                    morpheme.char_range.1 += segment.start_char_idx;
                }
                Some(WordFormationSegment {
                    char_range: (segment.start_char_idx, segment.end_char_idx),
                    result: self.word_formation_matcher.match_morphemes(&morphemes),
                    morphemes,
                })
            })
            .collect()
    }

    pub fn inspect_bunsetsu(&self, text: &str) -> Vec<crate::models::BunsetsuAnalysisReport> {
        self.inspect_bunsetsu_internal(text, None)
    }

    pub fn inspect_bunsetsu_with_dictionary(
        &self,
        text: &str,
        dictionary: &crate::dictionary::lookup::DictionaryEngine,
    ) -> Vec<crate::models::BunsetsuAnalysisReport> {
        self.inspect_bunsetsu_internal(text, Some(dictionary))
    }

    fn inspect_bunsetsu_internal(
        &self,
        text: &str,
        dictionary: Option<&crate::dictionary::lookup::DictionaryEngine>,
    ) -> Vec<crate::models::BunsetsuAnalysisReport> {
        let prepared = ruby::prepare_text(text);
        let prepared_chars: Vec<char> = prepared.text.chars().collect();
        segment_text(&prepared_chars)
            .into_iter()
            .filter(|segment| segment.seg_type == SegmentType::Content)
            .filter_map(|segment| {
                let segment_text: String = prepared_chars
                    [segment.start_char_idx..segment.end_char_idx]
                    .iter()
                    .collect();
                if segment_text.is_empty() {
                    return None;
                }
                let mut morphemes = self.morpheme_analyzer.analyze(&segment_text);
                for morpheme in &mut morphemes {
                    morpheme.char_range.0 += segment.start_char_idx;
                    morpheme.char_range.1 += segment.start_char_idx;
                }
                let formations = self.word_formation_matcher.match_morphemes(&morphemes);
                let lexical_units = dictionary
                    .map(|dictionary| {
                        lexical::match_dictionary_lexical_units(
                            &morphemes,
                            dictionary,
                            &formations.accepted,
                        )
                    })
                    .unwrap_or_default();
                Some(self.bunsetsu_analyzer.analyze_with_lexical(
                    &morphemes,
                    &[],
                    &formations.accepted,
                    &lexical_units.accepted,
                ))
            })
            .collect()
    }

    pub fn nbest_candidates(
        &self,
        source: &AnnotatedToken,
        top_n: usize,
    ) -> Vec<crate::models::SegmentationCandidate> {
        let paths = self
            .morpheme_analyzer
            .analyze_nbest(&source.bunsetsu.surface, top_n);
        candidates::from_lattice(source, paths)
    }

    pub fn nbest_morphemes(&self, text: &str, top_n: usize) -> Vec<morpheme::MorphemeCandidate> {
        self.morpheme_analyzer.analyze_nbest(text, top_n)
    }

    pub fn apply_segmentation_choices(
        &self,
        tokens: &mut [AnnotatedToken],
        choices: &[crate::models::SegmentationChoice],
    ) {
        // 页面 token 数远大于用户选择数。先按表层词建立索引，避免每个 token
        // 都线性扫描全部持久化选择，同时保持同表层词的既有覆盖语义。
        let choices_by_surface: HashMap<_, _> = choices
            .iter()
            .map(|choice| (choice.surface.as_str(), choice))
            .collect();
        for token in tokens {
            if token.display_class != "content" {
                continue;
            }
            let Some(choice) = choices_by_surface.get(token.bunsetsu.surface.as_str()) else {
                continue;
            };
            let offset = token.bunsetsu.char_range.0;
            let mut morphemes = choice.morphemes.clone();
            for morpheme in &mut morphemes {
                morpheme.char_range.0 += offset;
                morpheme.char_range.1 += offset;
            }
            let formations = self.word_formation_matcher.match_morphemes(&morphemes);
            let annotations = formations
                .accepted
                .into_iter()
                .map(|item| item.annotation)
                .collect();
            token.bunsetsu = bunsetsu::build_bunsetsu_with_formations(morphemes, annotations);
            self.grammar_matcher
                .match_patterns(std::slice::from_mut(&mut token.bunsetsu));
        }
    }

    pub fn process_with_progress<F>(
        &self,
        text: &str,
        merge_rules: &[Vec<String>],
        report: &mut F,
    ) -> Vec<AnnotatedToken>
    where
        F: FnMut(AnalysisProgress),
    {
        self.process_internal(text, merge_rules, report, None, None, None)
    }

    pub fn process_with_dictionary_and_progress<F>(
        &self,
        text: &str,
        merge_rules: &[Vec<String>],
        dictionary: &crate::dictionary::lookup::DictionaryEngine,
        report: &mut F,
    ) -> Vec<AnnotatedToken>
    where
        F: FnMut(AnalysisProgress),
    {
        self.process_internal(text, merge_rules, report, None, Some(dictionary), None)
    }

    pub fn process_with_dictionary_and_progress_cancellable<F>(
        &self,
        text: &str,
        merge_rules: &[Vec<String>],
        dictionary: &crate::dictionary::lookup::DictionaryEngine,
        report: &mut F,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<AnnotatedToken>, AnalysisCancelled>
    where
        F: FnMut(AnalysisProgress),
    {
        let tokens = self.process_internal(
            text,
            merge_rules,
            report,
            None,
            Some(dictionary),
            Some(is_cancelled),
        );
        if is_cancelled() {
            Err(AnalysisCancelled)
        } else {
            Ok(tokens)
        }
    }

    /// 性能诊断入口：在各项实际工作完成时累计耗时。
    pub fn process_profiled(
        &self,
        text: &str,
        merge_rules: &[Vec<String>],
        timings: &mut TimingCollector,
    ) -> Vec<AnnotatedToken> {
        self.process_internal(text, merge_rules, &mut |_| {}, Some(timings), None, None)
    }

    pub fn process_profiled_with_dictionary(
        &self,
        text: &str,
        merge_rules: &[Vec<String>],
        dictionary: &crate::dictionary::lookup::DictionaryEngine,
        timings: &mut TimingCollector,
    ) -> Vec<AnnotatedToken> {
        self.process_internal(
            text,
            merge_rules,
            &mut |_| {},
            Some(timings),
            Some(dictionary),
            None,
        )
    }

    fn process_internal<F>(
        &self,
        text: &str,
        merge_rules: &[Vec<String>],
        report: &mut F,
        mut timings: Option<&mut TimingCollector>,
        dictionary: Option<&crate::dictionary::lookup::DictionaryEngine>,
        is_cancelled: Option<&dyn Fn() -> bool>,
    ) -> Vec<AnnotatedToken>
    where
        F: FnMut(AnalysisProgress),
    {
        if cancellation_requested(is_cancelled) {
            return Vec::new();
        }
        if text.is_empty() {
            return Vec::new();
        }

        // 1. Strip author-supplied ruby before NLP, then apply it as the authoritative reading.
        report(AnalysisProgress::stage(
            AnalysisPhase::Preparing,
            2,
            "整理原文与振假名",
        ));
        let preparation_started = Instant::now();
        let prepared = ruby::prepare_text(text);
        let prepared_chars: Vec<char> = prepared.text.chars().collect();
        let document_readings = ruby::build_document_reading_map(&prepared.annotations);
        let segments = segment_text(&prepared_chars);
        if let Some(timings) = timings.as_deref_mut() {
            timings.add("准备原文与切分", preparation_started.elapsed());
        }
        if cancellation_requested(is_cancelled) {
            return Vec::new();
        }

        report(AnalysisProgress::stage(
            AnalysisPhase::Tokenizing,
            5,
            "执行形态素分析",
        ));

        let total_chars = prepared_chars.len().max(1);
        let mut prepared_segments = Vec::with_capacity(segments.len());
        let mut prepared_chars_count = 0;
        let mut dictionary_queries = HashSet::new();

        // 第一遍只生成段级中间产物。词典查询词跨分段汇总后统一进入 SQLite，
        // 避免章节中每个短句分别执行一个小批次查询。
        for segment in segments {
            if cancellation_requested(is_cancelled) {
                return Vec::new();
            }
            let segment_len = segment.end_char_idx - segment.start_char_idx;
            if segment.seg_type != SegmentType::Content {
                prepared_segments.push(SegmentStage::Literal(segment));
                prepared_chars_count += segment_len;
                continue;
            }
            let segment_text: String = prepared_chars[segment.start_char_idx..segment.end_char_idx]
                .iter()
                .collect();
            if segment_text.is_empty() {
                prepared_chars_count += segment_len;
                continue;
            }
            let annotation_start = prepared
                .annotations
                .partition_point(|annotation| annotation.char_range.1 <= segment.start_char_idx);
            let annotation_end = prepared
                .annotations
                .partition_point(|annotation| annotation.char_range.0 < segment.end_char_idx);
            let segment_annotations = &prepared.annotations[annotation_start..annotation_end];

            let started = Instant::now();
            let mut morphemes = self.morpheme_analyzer.analyze(&segment_text);
            if let Some(timings) = timings.as_deref_mut() {
                timings.add("形态素分析", started.elapsed());
            }
            if cancellation_requested(is_cancelled) {
                return Vec::new();
            }

            let started = Instant::now();
            for morpheme in &mut morphemes {
                morpheme.char_range.0 += segment.start_char_idx;
                morpheme.char_range.1 += segment.start_char_idx;
            }
            ruby::override_morpheme_readings_with_chars(
                &prepared_chars,
                &mut morphemes,
                segment_annotations,
            );
            if let Some(timings) = timings.as_deref_mut() {
                timings.add("形态素后处理", started.elapsed());
            }

            let started = Instant::now();
            let formations = self
                .word_formation_matcher
                .match_morphemes(&morphemes)
                .accepted;
            if let Some(timings) = timings.as_deref_mut() {
                timings.add("构词匹配", started.elapsed());
            }

            let lexical_candidates = dictionary.map(|_| {
                let started = Instant::now();
                let candidates = lexical::prepare_dictionary_lexical_candidates(&morphemes);
                candidates.extend_queries(&mut dictionary_queries);
                if let Some(timings) = timings.as_deref_mut() {
                    timings.add("词典候选生成", started.elapsed());
                }
                candidates
            });
            prepared_segments.push(SegmentStage::Content(ContentStage {
                segment,
                annotation_range: (annotation_start, annotation_end),
                morphemes,
                formations,
                lexical_candidates,
            }));
            prepared_chars_count += segment_len;
            let percent = 5 + ((prepared_chars_count * 20) / total_chars) as u8;
            report(AnalysisProgress::counted(
                AnalysisPhase::Tokenizing,
                prepared_chars_count,
                total_chars,
                percent.min(25),
                "执行形态素分析与候选准备",
            ));
        }

        report(AnalysisProgress::stage(
            AnalysisPhase::DictionaryMatching,
            26,
            "查询本地词典词汇",
        ));
        if cancellation_requested(is_cancelled) {
            return Vec::new();
        }
        // SQL 保持单次批量执行；取消只在请求返回后立即生效，不能拆批拖慢热路径。
        let resolved_dictionary_forms = if let Some(dictionary) = dictionary {
            let started = Instant::now();
            let matches = dictionary.resolve_exact_forms_batch(&dictionary_queries);
            if let Some(timings) = timings.as_deref_mut() {
                timings.add("词典批量查询", started.elapsed());
            }
            matches
        } else {
            HashMap::new()
        };
        if cancellation_requested(is_cancelled) {
            return Vec::new();
        }
        report(AnalysisProgress::stage(
            AnalysisPhase::DictionaryMatching,
            64,
            "本地词典查询完成",
        ));

        report(AnalysisProgress::stage(
            AnalysisPhase::Chunking,
            65,
            "解析词典词汇并构建文节",
        ));

        let mut all_tokens = Vec::new();
        let mut completed_chars = 0;
        for stage in prepared_segments {
            if cancellation_requested(is_cancelled) {
                return Vec::new();
            }
            match stage {
                SegmentStage::Literal(segment) => {
                    let started = Instant::now();
                    all_tokens.push(literal_token(&prepared_chars, segment));
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("Token 组装", started.elapsed());
                    }
                    completed_chars += segment.end_char_idx - segment.start_char_idx;
                }
                SegmentStage::Content(stage) => {
                    let segment_annotations =
                        &prepared.annotations[stage.annotation_range.0..stage.annotation_range.1];
                    let lexical_units = if let Some(candidates) = stage.lexical_candidates {
                        let started = Instant::now();
                        let result = lexical::resolve_dictionary_lexical_candidates(
                            &stage.morphemes,
                            candidates,
                            &resolved_dictionary_forms,
                            &stage.formations,
                        );
                        if let Some(timings) = timings.as_deref_mut() {
                            timings.add("词典候选解析", started.elapsed());
                        }
                        result.accepted
                    } else {
                        Vec::new()
                    };

                    let started = Instant::now();
                    let bunsetsus = self.bunsetsu_analyzer.analyze_tokens_with_lexical(
                        &stage.morphemes,
                        merge_rules,
                        &stage.formations,
                        &lexical_units,
                    );
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("文节分析", started.elapsed());
                    }

                    let started = Instant::now();
                    let mut bunsetsus =
                        ruby::merge_annotated_bunsetsus(bunsetsus, segment_annotations);
                    ruby::override_bunsetsu_readings_with_document_map(
                        &prepared_chars,
                        &mut bunsetsus,
                        segment_annotations,
                        &document_readings,
                    );
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("文节振假名", started.elapsed());
                    }

                    let started = Instant::now();
                    self.grammar_matcher.match_patterns(&mut bunsetsus);
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("语法匹配", started.elapsed());
                    }
                    if cancellation_requested(is_cancelled) {
                        return Vec::new();
                    }

                    let started = Instant::now();
                    all_tokens.extend(bunsetsus.into_iter().map(|bunsetsu| AnnotatedToken {
                        bunsetsu,
                        novelty_score: 1.0,
                        is_selected: false,
                        is_known: false,
                        inference_reason: None,
                        expressions: Vec::new(),
                        display_class: "content".to_string(),
                    }));
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("Token 组装", started.elapsed());
                    }
                    completed_chars += stage.segment.end_char_idx - stage.segment.start_char_idx;
                }
            }

            let progress_percent = 65 + ((completed_chars * 20) / total_chars) as u8;
            let (phase, message) = if progress_percent < 75 {
                (AnalysisPhase::Chunking, "解析词典词汇并构建文节")
            } else {
                (AnalysisPhase::GrammarMatching, "匹配语法模式")
            };
            report(AnalysisProgress::counted(
                phase,
                completed_chars,
                total_chars,
                progress_percent.min(85),
                message,
            ));
        }

        report(AnalysisProgress::stage(
            AnalysisPhase::GrammarMatching,
            85,
            "语法模式匹配完成",
        ));
        if cancellation_requested(is_cancelled) {
            return Vec::new();
        }

        let sorting_started = Instant::now();
        all_tokens.sort_by_key(|t| t.bunsetsu.char_range.0);
        grammar::canonicalize_document_coordinates(&mut all_tokens);
        if let Some(timings) = timings.as_deref_mut() {
            timings.add("Token 排序", sorting_started.elapsed());
        }
        if cancellation_requested(is_cancelled) {
            return Vec::new();
        }

        all_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_ipadic_path() -> Option<String> {
        if let Ok(env_path) = std::env::var("KOTOCLIP_TEST_IPADIC") {
            if std::path::Path::new(&env_path).exists() {
                return Some(env_path);
            }
        }
        let candidates = vec![
            "../../ipadic/system.dic",
            "../ipadic/system.dic",
            "ipadic/system.dic",
        ];
        for c in candidates {
            if std::path::Path::new(c).exists() {
                return Some(c.to_string());
            }
        }
        None
    }

    #[test]
    fn test_pipeline_smoke() {
        // 使用用户下载的 IPADIC 二进制字典路径进行测试
        let dict_path = match get_test_ipadic_path() {
            Some(p) => p,
            None => {
                println!("测试跳过：未找到 IPADIC system.dic 字典文件。");
                return;
            }
        };

        let pipeline = Pipeline::new(&dict_path).expect("初始化 NLP Pipeline 失败");

        // 核心测试文本: 食べさせられなかった (吃 + 使役 + 被动 + 过去否定)
        let tokens = pipeline.process("食べさせられなかった", &[]);

        println!("=== DEBUG TOKENS ===");
        for token in &tokens {
            println!("Bunsetsu Surface: {}", token.bunsetsu.surface);
            for m in &token.bunsetsu.morphemes {
                println!(
                    "  Morpheme: surface={}, base_form={}, pos={:?}, conjugation_type={}, conjugation_form={}", 
                    m.surface, m.base_form, m.pos, m.conjugation_type, m.conjugation_form
                );
            }
        }
        println!("====================");

        // 断言其合并为一个文节
        assert_eq!(tokens.len(), 1, "吃/使役/被动/否定 应该合并为单个文节");
        let token = &tokens[0];
        assert_eq!(token.bunsetsu.surface, "食べさせられなかった");

        // 断言其核心自立语被成功智能还原为原型 "食べる"
        assert_eq!(
            token.bunsetsu.head_word.base_form, "食べる",
            "辞书形还原不正确"
        );

        // 断言新活用层保留使役、られる候选、否定和过去四个可追溯事实。
        let concepts = token
            .bunsetsu
            .grammar_occurrences
            .iter()
            .map(|occurrence| occurrence.concept_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert!(
            concepts.contains("morphology.voice.causative"),
            "未识别出使役"
        );
        assert!(
            concepts.contains("morphology.voice.passive_potential"),
            "未保留られる候选"
        );
        assert!(
            concepts.contains("morphology.polarity.negative"),
            "未识别出否定"
        );
        assert!(concepts.contains("morphology.tense.past"), "未识别出过去");
    }

    #[test]
    fn lexical_inflection_expands_display_head_without_changing_lookup_form() {
        let Some(dict_path) = get_test_ipadic_path() else {
            println!("测试跳过：未找到 IPADIC system.dic 字典文件。");
            return;
        };
        let pipeline = Pipeline::new(&dict_path).expect("初始化 NLP Pipeline 失败");
        let tokens = pipeline.process(
            "見えなかった。分類し、静かな場所で読んでくださった。行くな。",
            &[],
        );

        let token = |surface: &str| {
            tokens
                .iter()
                .find(|token| token.bunsetsu.surface == surface)
                .unwrap_or_else(|| panic!("未找到文节：{surface}"))
        };

        let negative = token("見えなかった");
        assert_eq!(negative.bunsetsu.head_word.surface, "見えなかった");
        assert_eq!(negative.bunsetsu.head_word.base_form, "見える");
        assert!(negative.bunsetsu.grammar_tags.is_empty());

        let sahen = token("分類し");
        assert_eq!(sahen.bunsetsu.head_word.surface, "分類し");
        assert_eq!(sahen.bunsetsu.head_word.base_form, "分類");
        assert_eq!(
            sahen.bunsetsu.morphology.chains[0].dictionary_form,
            "分類する"
        );

        let adjective = token("静かな");
        assert_eq!(adjective.bunsetsu.head_word.surface, "静かな");
        assert!(adjective
            .bunsetsu
            .grammar_tags
            .iter()
            .all(|tag| tag.concept_id != "grammar.functional.na"));

        let benefactive = token("読んでくださった");
        assert_eq!(benefactive.bunsetsu.head_word.surface, "読んで");
        let functional_range = benefactive
            .bunsetsu
            .morphology
            .chains
            .iter()
            .find(|chain| chain.role == crate::models::MorphologyChainRole::Functional)
            .expect("补助用言应具有独立活用链")
            .char_range;
        assert!(benefactive.bunsetsu.grammar_tags.iter().any(|tag| {
            tag.concept_id == "grammar.benefactive.te_kudasaru"
                && tag
                    .display_ranges
                    .iter()
                    .any(|range| functional_range.0 >= range.0 && functional_range.1 <= range.1)
        }));

        let terminal = token("行くな");
        assert_eq!(terminal.bunsetsu.head_word.surface, "行く");
        assert!(terminal
            .bunsetsu
            .grammar_tags
            .iter()
            .any(|tag| tag.concept_id == "grammar.sentence_particle.na"));
    }

    #[test]
    fn test_user_example_preserves_complete_text() {
        let dict_path = match get_test_ipadic_path() {
            Some(p) => p,
            None => {
                println!("测试跳过：未找到 IPADIC system.dic 字典文件。");
                return;
            }
        };

        let text = "しらばっくれんなよ、古川。お前の素性は割れてんだ。マガツカミを连れ歩くはぐれ者が、正面から堂々と警察署に乗り込んでくるその度胸だきゃあ褒めてやろう。貴様、マガツカミを使って何をするつもりだった？";
        let pipeline = Pipeline::new(&dict_path).expect("初始化 NLP Pipeline 失败");
        let tokens = pipeline.process(text, &[]);

        assert!(!tokens.is_empty(), "用户例文应生成至少一个 token");
        let reconstructed: String = tokens
            .iter()
            .map(|token| token.bunsetsu.surface.as_str())
            .collect();
        assert_eq!(reconstructed, text, "分词输出必须完整保留输入文本");

        // 标点符号也是 token
        assert!(tokens
            .iter()
            .all(|token| !token.bunsetsu.morphemes.is_empty()));
        assert!(
            tokens
                .iter()
                .any(|token| token.bunsetsu.head_word.base_form == "警察署"),
            "名词接尾必须并入完整词典原形"
        );
    }

    #[test]
    fn test_authoritative_ruby_is_removed_and_overrides_reading() {
        let dict_path = match get_test_ipadic_path() {
            Some(p) => p,
            None => {
                println!("测试跳过：未找到 IPADIC system.dic 字典文件。");
                return;
            }
        };

        let text = "七《なの》日《か》は屈《かが》み、鬼怒川の腰元にぶら下がる键を夺う。\n  ついでに胸ポケットの煙草《たばこ》ももらっておいた。と、突然手首を掴まれる。\n\n  \u{3000}鋭い痛みに顔をしかめながらも、鬼怒川是眼光鋭く、七日を睨《にら》みつけていた。\n\n  「......古《ふる》川《かわ》。俺の名を覚えておけ。貴様、絶対に──」";
        let expected = "七日は屈み、鬼怒川の腰元にぶら下がる键を夺う。\n  ついでに胸ポケットの煙草ももらっておいた。と、突然手首を掴まれる。\n\n  \u{3000}鋭い痛みに顔をしかめながらも、鬼怒川是眼光鋭く、七日を睨みつけていた。\n\n  「......古川。俺の名を覚えておけ。貴様、絶対に──」";
        let pipeline = Pipeline::new(&dict_path).expect("初始化 NLP Pipeline 失败");
        let tokens = pipeline.process(text, &[]);
        let reconstructed: String = tokens
            .iter()
            .map(|token| token.bunsetsu.surface.as_str())
            .collect();

        assert_eq!(reconstructed, expected, "分析结果不得保留 ruby 标记");
        assert!(tokens.iter().all(|token| {
            !token.bunsetsu.surface.contains('《') && !token.bunsetsu.surface.contains('》')
        }));

        let head_reading = |surface: &str| {
            tokens
                .iter()
                .find(|token| token.bunsetsu.head_word.surface == surface)
                .map(|token| token.bunsetsu.head_word.reading.as_str())
        };
        assert_eq!(head_reading("煙草"), Some("タバコ"));
        assert_eq!(head_reading("古川"), Some("フルカワ"));
        assert!(tokens
            .iter()
            .filter(|token| token.bunsetsu.head_word.surface == "七日")
            .all(|token| token.bunsetsu.head_word.reading == "ナノカ"));

        let annotated_morphemes: Vec<(&str, &str)> = tokens
            .iter()
            .flat_map(|token| token.bunsetsu.morphemes.iter())
            .map(|m| (m.surface.as_str(), m.reading.as_str()))
            .collect();
        assert!(annotated_morphemes.contains(&("屈み", "カガミ")));
        assert!(annotated_morphemes.contains(&("睨みつけ", "ニラミツケ")));
    }

    #[test]
    fn test_punctuation_segmentation_and_preservation() {
        let dict_path = match get_test_ipadic_path() {
            Some(p) => p,
            None => {
                println!("测试跳过：未找到 IPADIC system.dic 字典文件。");
                return;
            }
        };
        let pipeline = Pipeline::new(&dict_path).expect("初始化 NLP Pipeline 失败");

        // 测试包含复杂标点的文本，包括行首对话、连续符号组合
        let text = "「甲だ。」「乙か！？」……――○～〜";
        let tokens = pipeline.process(text, &[]);

        // 验证原文可重建且没有漏字符
        let reconstructed: String = tokens.iter().map(|t| t.bunsetsu.surface.as_str()).collect();
        assert_eq!(reconstructed, text);

        // 验证符号在分词前强制切断，形成 display_class = "punctuation"
        assert_eq!(tokens[0].bunsetsu.surface, "「");
        assert_eq!(tokens[0].display_class, "punctuation");

        // 验证相邻词头不含符号
        assert_eq!(tokens[1].bunsetsu.surface, "甲だ");
        assert_eq!(tokens[1].display_class, "content");

        assert_eq!(tokens[2].bunsetsu.surface, "。");
        assert_eq!(tokens[2].display_class, "punctuation");

        assert_eq!(tokens[3].bunsetsu.surface, "」");
        assert_eq!(tokens[3].display_class, "punctuation");

        assert_eq!(tokens[4].bunsetsu.surface, "「");
        assert_eq!(tokens[4].display_class, "punctuation");

        assert_eq!(tokens[5].bunsetsu.surface, "乙か");
        assert_eq!(tokens[5].display_class, "content");

        assert_eq!(tokens[6].bunsetsu.surface, "！？");
        assert_eq!(tokens[6].display_class, "punctuation");

        assert_eq!(tokens[7].bunsetsu.surface, "」");
        assert_eq!(tokens[7].display_class, "punctuation");

        assert_eq!(tokens[8].bunsetsu.surface, "……");
        assert_eq!(tokens[8].display_class, "punctuation");

        assert_eq!(tokens[9].bunsetsu.surface, "――");
        assert_eq!(tokens[9].display_class, "punctuation");

        // ○～〜 保留在 content 分类中且不触发切分
        for t in &tokens[10..] {
            assert_eq!(t.display_class, "content");
        }
    }
}
