pub mod bunsetsu;
pub mod candidates;
pub mod expressions;
pub mod grammar;
pub mod morpheme;
pub mod restore;
pub mod ruby;

use crate::analysis_progress::{AnalysisPhase, AnalysisProgress};
use crate::models::AnnotatedToken;
use crate::performance::TimingCollector;
use std::path::Path;
use std::time::Instant;

/// NLP 处理引擎管线容器
pub struct Pipeline {
    morpheme_analyzer: morpheme::MorphemeAnalyzer,
    grammar_matcher: grammar::GrammarMatcher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentType {
    Content,
    Punctuation,
    LineBreak,
}

struct TextSegment {
    seg_type: SegmentType,
    start_char_idx: usize,
    end_char_idx: usize,
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

impl Pipeline {
    /// 从指定字典文件路径初始化 NLP 管线
    pub fn new<P: AsRef<Path>>(dict_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let morpheme_analyzer = morpheme::MorphemeAnalyzer::new(dict_path)?;
        let grammar_matcher = grammar::GrammarMatcher::new()?;
        Ok(Self {
            morpheme_analyzer,
            grammar_matcher,
        })
    }

    /// 执行完整的 NLP 管线：形態素解析 -> 文節组块 -> 语法匹配 -> 辞書形还原 (应用自定义合并规则)
    pub fn process(&self, text: &str, merge_rules: &[Vec<String>]) -> Vec<AnnotatedToken> {
        self.process_with_progress(text, merge_rules, &mut |_| {})
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
        for token in tokens {
            if token.display_class != "content" {
                continue;
            }
            let Some(choice) = choices
                .iter()
                .find(|choice| choice.surface == token.bunsetsu.surface)
            else {
                continue;
            };
            let offset = token.bunsetsu.char_range.0;
            let mut morphemes = choice.morphemes.clone();
            for morpheme in &mut morphemes {
                morpheme.char_range.0 += offset;
                morpheme.char_range.1 += offset;
            }
            token.bunsetsu = bunsetsu::build_bunsetsu(morphemes);
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
        self.process_internal(text, merge_rules, report, None)
    }

    /// 性能诊断入口：在各项实际工作完成时累计耗时。
    pub fn process_profiled(
        &self,
        text: &str,
        merge_rules: &[Vec<String>],
        timings: &mut TimingCollector,
    ) -> Vec<AnnotatedToken> {
        self.process_internal(text, merge_rules, &mut |_| {}, Some(timings))
    }

    fn process_internal<F>(
        &self,
        text: &str,
        merge_rules: &[Vec<String>],
        report: &mut F,
        mut timings: Option<&mut TimingCollector>,
    ) -> Vec<AnnotatedToken>
    where
        F: FnMut(AnalysisProgress),
    {
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

        // 2. 扫描并分割 text 为 content、punctuation、line_break 段
        let segments = segment_text(&prepared_chars);
        let mut all_tokens: Vec<AnnotatedToken> = Vec::new();
        if let Some(timings) = timings.as_deref_mut() {
            timings.add("准备原文与切分", preparation_started.elapsed());
        }

        report(AnalysisProgress::stage(
            AnalysisPhase::Tokenizing,
            3,
            "执行形态素分析",
        ));

        let total_chars = prepared_chars.len().max(1);
        let mut processed_chars = 0;

        for seg in &segments {
            let seg_len = seg.end_char_idx - seg.start_char_idx;
            match seg.seg_type {
                SegmentType::Content => {
                    let seg_text: String = prepared_chars[seg.start_char_idx..seg.end_char_idx]
                        .iter()
                        .collect();
                    if seg_text.is_empty() {
                        processed_chars += seg_len;
                        continue;
                    }

                    let tokenizing_started = Instant::now();
                    let mut morphemes = self.morpheme_analyzer.analyze(&seg_text);
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("形态素分析", tokenizing_started.elapsed());
                    }
                    let morpheme_postprocess_started = Instant::now();
                    let offset = seg.start_char_idx;
                    for m in &mut morphemes {
                        m.char_range.0 += offset;
                        m.char_range.1 += offset;
                    }

                    ruby::override_morpheme_readings_with_chars(
                        &prepared_chars,
                        &mut morphemes,
                        &prepared.annotations,
                    );
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("形态素后处理", morpheme_postprocess_started.elapsed());
                    }

                    let chunking_started = Instant::now();
                    let bunsetsus = bunsetsu::chunk(&morphemes, merge_rules);
                    let mut bunsetsus =
                        ruby::merge_annotated_bunsetsus(bunsetsus, &prepared.annotations);
                    ruby::override_bunsetsu_readings_with_chars(
                        &prepared_chars,
                        &mut bunsetsus,
                        &prepared.annotations,
                    );
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("文节与振假名", chunking_started.elapsed());
                    }

                    let grammar_started = Instant::now();
                    self.grammar_matcher.match_patterns(&mut bunsetsus);
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("语法匹配", grammar_started.elapsed());
                    }

                    let assembling_started = Instant::now();
                    for b in bunsetsus {
                        all_tokens.push(AnnotatedToken {
                            bunsetsu: b,
                            novelty_score: 1.0,
                            is_selected: false,
                            is_known: false,
                            inference_reason: None,
                            expressions: Vec::new(),
                            display_class: "content".to_string(),
                        });
                    }
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("Token 组装", assembling_started.elapsed());
                    }
                }
                SegmentType::Punctuation | SegmentType::LineBreak => {
                    let assembling_started = Instant::now();
                    let seg_text: String = prepared_chars[seg.start_char_idx..seg.end_char_idx]
                        .iter()
                        .collect();

                    let major_pos = if seg.seg_type == SegmentType::Punctuation {
                        "記号".to_string()
                    } else {
                        "改行".to_string()
                    };

                    let p_morpheme = crate::models::Morpheme {
                        surface: seg_text.clone(),
                        pos: crate::models::PosTag {
                            major: major_pos.clone(),
                            sub1: "*".to_string(),
                            sub2: "*".to_string(),
                            sub3: "*".to_string(),
                        },
                        base_form: seg_text.clone(),
                        reading: "".to_string(),
                        conjugation_type: "*".to_string(),
                        conjugation_form: "*".to_string(),
                        char_range: (seg.start_char_idx, seg.end_char_idx),
                    };

                    let p_bunsetsu = crate::models::Bunsetsu {
                        morphemes: vec![p_morpheme.clone()],
                        surface: seg_text.clone(),
                        head_word: crate::models::HeadWord {
                            surface: seg_text.clone(),
                            base_form: seg_text.clone(),
                            reading: "".to_string(),
                            pos: crate::models::PosTag {
                                major: major_pos,
                                sub1: "*".to_string(),
                                sub2: "*".to_string(),
                                sub3: "*".to_string(),
                            },
                        },
                        grammar_tags: Vec::new(),
                        char_range: (seg.start_char_idx, seg.end_char_idx),
                    };

                    let disp = if seg.seg_type == SegmentType::Punctuation {
                        "punctuation".to_string()
                    } else {
                        "line_break".to_string()
                    };

                    all_tokens.push(AnnotatedToken {
                        bunsetsu: p_bunsetsu,
                        novelty_score: 0.0,
                        is_selected: false,
                        is_known: true,
                        inference_reason: None,
                        expressions: Vec::new(),
                        display_class: disp,
                    });
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.add("Token 组装", assembling_started.elapsed());
                    }
                }
            }

            processed_chars += seg_len;

            // 实时平滑上报进度，避免卡在 3%
            let progress_percent = 3 + ((processed_chars * 51) / total_chars) as u8;
            let (phase, msg) = if progress_percent < 25 {
                (AnalysisPhase::Tokenizing, "执行形态素分析与分词")
            } else if progress_percent < 50 {
                (AnalysisPhase::Chunking, "构建文节组块")
            } else {
                (AnalysisPhase::GrammarMatching, "匹配语法模式")
            };

            report(AnalysisProgress::counted(
                phase,
                processed_chars,
                total_chars,
                progress_percent.min(54),
                msg,
            ));
        }

        let sorting_started = Instant::now();
        all_tokens.sort_by_key(|t| t.bunsetsu.char_range.0);
        if let Some(timings) = timings.as_deref_mut() {
            timings.add("Token 排序", sorting_started.elapsed());
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

        // 断言其匹配到了 [使役受身] 和 [过去否定] 两个语法标签
        let has_causative_passive = token
            .bunsetsu
            .grammar_tags
            .iter()
            .any(|t| t.pattern_id == "causative_passive");
        let has_past_negative = token
            .bunsetsu
            .grammar_tags
            .iter()
            .any(|t| t.pattern_id == "past_negative");

        assert!(has_causative_passive, "未识别出 [使役受身] 语法结构");
        assert!(has_past_negative, "未识别出 [過去否定] 语法结构");
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
