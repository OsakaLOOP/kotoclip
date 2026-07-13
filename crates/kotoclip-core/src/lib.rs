pub mod analysis_progress;
pub mod dictionary;
pub mod export;
pub mod ffi;
pub mod llm;
pub mod models;
pub mod performance;
pub mod pipeline;
pub mod profile;
pub mod transport;

use analysis_progress::{AnalysisPhase, AnalysisProgress};
use dictionary::lookup::DictionaryEngine;
use models::{
    AnnotatedToken, DictionaryLookup, ExpressionRule, SegmentationCandidate, SegmentationChoice,
};
use performance::TimingCollector;
use pipeline::Pipeline;
use profile::ProfileEngine;
use std::path::Path;
use std::time::Instant;

/// Kotoclip 核心引擎，粘合了分词管线、词库检索以及用户历史曝光画像
pub struct Engine {
    pipeline: Pipeline,
    dictionary: DictionaryEngine,
    profile: ProfileEngine,
}

impl Engine {
    /// 从对应路径初始化整个引擎 (包括形态素字典、SQLite 词典群目录以及用户数据 SQLite 文件)
    pub fn new<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(
        dict_path: P1,
        dicts_dir: P2,
        user_db_path: P3,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let pipeline = Pipeline::new(dict_path)?;
        let dictionary = DictionaryEngine::new(dicts_dir)?;
        let profile = ProfileEngine::new(user_db_path)?;

        Ok(Self {
            pipeline,
            dictionary,
            profile,
        })
    }

    /// 诊断专用初始化入口，拆分词法模型、外部词典与画像数据库的真实冷启动耗时。
    pub fn new_profiled<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(
        dict_path: P1,
        dicts_dir: P2,
        user_db_path: P3,
    ) -> Result<(Self, TimingCollector), Box<dyn std::error::Error>> {
        let mut timings = TimingCollector::default();

        let started = Instant::now();
        let pipeline = Pipeline::new(dict_path)?;
        timings.add("词法模型与语法规则初始化", started.elapsed());

        let started = Instant::now();
        let dictionary = DictionaryEngine::new(dicts_dir)?;
        timings.add("外部词典初始化", started.elapsed());

        let started = Instant::now();
        let profile = ProfileEngine::new(user_db_path)?;
        timings.add("画像数据库初始化", started.elapsed());

        Ok((
            Self {
                pipeline,
                dictionary,
                profile,
            },
            timings,
        ))
    }

    /// 解析一段文章，执行完整分词、语法提取，并自动从画像库标注生词分值 (Novelty)
    pub fn analyze_text(
        &self,
        text: &str,
    ) -> Result<Vec<AnnotatedToken>, Box<dyn std::error::Error>> {
        self.analyze_text_with_exposure(text, true)
    }

    /// Analyze text and optionally record the rendered lexical tokens as exposures.
    /// Internal refreshes (for example after registering a merge rule) disable recording.
    pub fn analyze_text_with_exposure(
        &self,
        text: &str,
        record_exposure: bool,
    ) -> Result<Vec<AnnotatedToken>, Box<dyn std::error::Error>> {
        self.analyze_text_with_progress(text, record_exposure, |_| {})
    }

    /// 执行分析并在真实管线边界及逐 token 阶段报告进度。
    pub fn analyze_text_with_progress<F>(
        &self,
        text: &str,
        record_exposure: bool,
        mut report: F,
    ) -> Result<Vec<AnnotatedToken>, Box<dyn std::error::Error>>
    where
        F: FnMut(AnalysisProgress),
    {
        report(AnalysisProgress::stage(
            AnalysisPhase::Preparing,
            1,
            "准备分析与表达规则",
        ));
        // 历史 user_merge_rules 不再进入正式 Pipeline。文节边界保持由 NLP 与
        // 词典边界解析器决定；用户拖拽仅创建独立的跨文节表达注解。
        let mut tokens = self.pipeline.process_with_progress(text, &[], &mut report);
        let segmentation_choices = self.profile.get_segmentation_choices()?;
        self.pipeline
            .apply_segmentation_choices(&mut tokens, &segmentation_choices);
        let token_count = tokens.len();
        let report_step = (token_count / 100).max(1);
        report(AnalysisProgress::counted(
            AnalysisPhase::DictionaryMatching,
            0,
            token_count,
            40,
            "按词典校正词汇边界",
        ));
        for (index, token) in tokens.iter_mut().enumerate() {
            if token.display_class != "content" {
                let completed = index + 1;
                if completed == token_count || completed % report_step == 0 {
                    let percent = 40 + ((completed * 6 / token_count.max(1)) as u8);
                    report(AnalysisProgress::counted(
                        AnalysisPhase::DictionaryMatching,
                        completed,
                        token_count,
                        percent,
                        "按词典校正词汇边界",
                    ));
                }
                continue;
            }
            pipeline::bunsetsu::resolve_lexical_boundaries(
                std::slice::from_mut(&mut token.bunsetsu),
                |word| self.dictionary.contains_exact(word),
            );
            let completed = index + 1;
            if completed == token_count || completed % report_step == 0 {
                let percent = 40 + ((completed * 6 / token_count.max(1)) as u8);
                report(AnalysisProgress::counted(
                    AnalysisPhase::DictionaryMatching,
                    completed,
                    token_count,
                    percent,
                    "按词典校正词汇边界",
                ));
            }
        }
        // 调用画像引擎打分标注
        let mut annotated =
            self.profile
                .annotate_tokens_with_progress(tokens, |completed, total| {
                    let percent = 46 + ((completed * 1 / total.max(1)) as u8);
                    report(AnalysisProgress::counted(
                        AnalysisPhase::ProfileScoring,
                        completed,
                        total,
                        percent,
                        "计算词汇熟悉度",
                    ));
                })?;
        // 表达层是当前回测的主要剩余耗时，按真实阶段拆分进度，避免画像评分
        // 阶段长时间显示 61%~96% 而实际工作已经完成。
        report(AnalysisProgress::stage(
            AnalysisPhase::ExpressionMatching,
            47,
            "应用自定义表达规则",
        ));
        self.profile.apply_expression_rules(&mut annotated)?;
        report(AnalysisProgress::stage(
            AnalysisPhase::ExpressionMatching,
            50,
            "匹配内置表达",
        ));
        pipeline::expressions::apply_builtin_expressions(&mut annotated);
        report(AnalysisProgress::stage(
            AnalysisPhase::ExpressionMatching,
            92,
            "匹配呼应表达",
        ));
        pipeline::expressions::apply_correlative_expressions(&mut annotated);
        report(AnalysisProgress::stage(
            AnalysisPhase::ExpressionMatching,
            96,
            "整理表达边界",
        ));
        pipeline::expressions::resolve_expression_conflicts(&mut annotated);
        if record_exposure {
            self.profile
                .record_token_exposures_with_progress(&annotated, |completed, total| {
                    let percent = 98 + ((completed * 2 / total.max(1)) as u8);
                    report(AnalysisProgress::counted(
                        AnalysisPhase::RecordingExposure,
                        completed,
                        total,
                        percent,
                        "记录本次词汇曝光",
                    ));
                })?;
        }
        report(AnalysisProgress::stage(
            AnalysisPhase::Completed,
            100,
            "分析完成",
        ));
        Ok(annotated)
    }

    /// 诊断专用的完整分析入口。计时紧贴真实函数调用边界，
    /// 不依赖 UI 进度百分比推断阶段耗时。
    pub fn analyze_text_profiled(
        &self,
        text: &str,
        record_exposure: bool,
    ) -> Result<(Vec<AnnotatedToken>, TimingCollector), Box<dyn std::error::Error>> {
        let mut timings = TimingCollector::default();

        let started = Instant::now();
        let mut tokens = self.pipeline.process_profiled(text, &[], &mut timings);
        timings.add("分析管线总计", started.elapsed());

        let started = Instant::now();
        let segmentation_choices = self.profile.get_segmentation_choices()?;
        timings.add("分词选择读取", started.elapsed());

        let started = Instant::now();
        self.pipeline
            .apply_segmentation_choices(&mut tokens, &segmentation_choices);
        timings.add("分词选择应用", started.elapsed());

        let started = Instant::now();
        for token in &mut tokens {
            if token.display_class != "content" {
                continue;
            }
            pipeline::bunsetsu::resolve_lexical_boundaries(
                std::slice::from_mut(&mut token.bunsetsu),
                |word| self.dictionary.contains_exact(word),
            );
        }
        timings.add("词典边界", started.elapsed());

        let started = Instant::now();
        let mut annotated = self
            .profile
            .annotate_tokens_profiled(tokens, &mut timings)?;
        timings.add("画像评分总计", started.elapsed());

        let started = Instant::now();
        self.profile.apply_expression_rules(&mut annotated)?;
        timings.add("自定义表达", started.elapsed());

        let started = Instant::now();
        pipeline::expressions::apply_builtin_expressions(&mut annotated);
        timings.add("内置表达", started.elapsed());

        let started = Instant::now();
        pipeline::expressions::apply_correlative_expressions(&mut annotated);
        timings.add("呼应表达", started.elapsed());

        let started = Instant::now();
        pipeline::expressions::resolve_expression_conflicts(&mut annotated);
        timings.add("表达边界", started.elapsed());

        if record_exposure {
            self.profile
                .record_token_exposures_profiled(&annotated, &mut timings)?;
        }

        Ok((annotated, timings))
    }

    /// 记录生词的曝光历史 (由阅读流自动驱动)
    pub fn record_exposure(
        &self,
        base_form: &str,
        reading: &str,
        pos: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.profile.record_exposure(base_form, reading, pos)?;
        Ok(())
    }

    /// 主动标记单词为“已知” (脱下胶囊)
    pub fn mark_known(
        &self,
        base_form: &str,
        reading: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.profile.mark_known(base_form, reading)?;
        Ok(())
    }

    /// 主动标记单词为“未知”
    pub fn mark_unknown(
        &self,
        base_form: &str,
        reading: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.profile.mark_unknown(base_form, reading)?;
        Ok(())
    }

    /// 从词库中查询单词释义，并按传入的词典优先级顺序列表进行重排聚合
    pub fn lookup_word(
        &self,
        word: &str,
        reading: Option<&str>,
        priority_list: &[String],
    ) -> DictionaryLookup {
        let query_key = dictionary_query_key(word, reading);
        let initial_entries = self.dictionary.lookup(word, reading);
        let mut candidates = if is_kana(word) {
            initial_entries
                .iter()
                .filter(|entry| entry.headword == word)
                .flat_map(|entry| {
                    entry
                        .links
                        .iter()
                        .filter(|link| link.relation == "candidate")
                        .cloned()
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut seen_candidates = std::collections::HashSet::new();
        candidates.retain(|candidate| seen_candidates.insert(candidate.target.clone()));
        let selected_target = self.profile.dictionary_choice(&query_key).filter(|target| {
            candidates
                .iter()
                .any(|candidate| &candidate.target == target)
        });
        let entries = selected_target
            .as_deref()
            .map(|target| self.dictionary.lookup(target, reading))
            .filter(|entries| !entries.is_empty())
            .unwrap_or(initial_entries);
        DictionaryLookup {
            query: word.to_string(),
            reading: reading.map(str::to_string),
            selected_target,
            candidates,
            entries: dictionary::aggregate::sort_definitions(entries, priority_list),
        }
    }

    pub fn choose_dictionary_target(
        &self,
        query: &str,
        reading: Option<&str>,
        target: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.profile
            .set_dictionary_choice(&dictionary_query_key(query, reading), target)?;
        Ok(())
    }

    /// 记录用户自定义分词合并规则
    pub fn add_merge_rule(&self, parts: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        self.profile.add_merge_rule(parts)
    }

    /// 获取所有用户自定义分词合并规则
    pub fn get_merge_rules(&self) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
        self.profile.get_merge_rules()
    }

    pub fn add_expression_rule(
        &self,
        tokens: &[AnnotatedToken],
        label: Option<&str>,
        description: Option<&str>,
        bunsetsu_states: &[String],
        morpheme_masks: &[Vec<bool>],
        gap_after: Option<usize>,
    ) -> Result<ExpressionRule, Box<dyn std::error::Error>> {
        self.profile.add_expression_rule(
            tokens,
            label,
            description,
            bunsetsu_states,
            morpheme_masks,
            gap_after,
            if gap_after.is_some() {
                "correlative"
            } else {
                "grammar_construction"
            },
            50,
            "annotate_only",
        )
    }

    pub fn add_configured_expression_rule(
        &self,
        tokens: &[AnnotatedToken],
        label: Option<&str>,
        description: Option<&str>,
        bunsetsu_states: &[String],
        morpheme_masks: &[Vec<bool>],
        gap_after: Option<usize>,
        expression_type: &str,
        priority: i32,
        boundary_effect: &str,
    ) -> Result<ExpressionRule, Box<dyn std::error::Error>> {
        self.profile.add_expression_rule(
            tokens,
            label,
            description,
            bunsetsu_states,
            morpheme_masks,
            gap_after,
            expression_type,
            priority,
            boundary_effect,
        )
    }

    pub fn get_expression_rules(&self) -> Result<Vec<ExpressionRule>, Box<dyn std::error::Error>> {
        self.profile.get_expression_rules()
    }

    /// 仅刷新自定义表达注解，复用已有分词、语法、画像及词典分析结果。
    pub fn refresh_expression_annotations(
        &self,
        mut tokens: Vec<AnnotatedToken>,
    ) -> Result<Vec<AnnotatedToken>, Box<dyn std::error::Error>> {
        for token in &mut tokens {
            token
                .expressions
                .retain(|expression| expression.origin != "custom");
        }
        self.profile.apply_expression_rules(&mut tokens)?;
        Ok(tokens)
    }

    pub fn delete_expression_rule(&self, id: i64) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(self.profile.delete_expression_rule(id)?)
    }

    pub fn get_candidates(
        &self,
        token: &AnnotatedToken,
        top_n: usize,
    ) -> Vec<SegmentationCandidate> {
        if top_n == 0 {
            return Vec::new();
        }
        // 先保留较宽的真实 lattice 候选池，再用外部词典证据重排。
        // 原始 Vibrato rank/cost 始终保留，词典层不伪装成 Viterbi 成本。
        let pool_size = top_n.saturating_mul(4).max(top_n);
        let mut candidates = self.pipeline.nbest_candidates(token, pool_size);
        for candidate in &mut candidates {
            let mut bonus = 0_i64;
            for item in &candidate.tokens {
                let head = &item.bunsetsu.head_word;
                let chars = head.base_form.chars().count();
                if chars >= 2 && self.dictionary.contains_exact(&head.base_form) {
                    candidate.dictionary_evidence.push(head.base_form.clone());
                    bonus += (chars * chars) as i64 * 1800;
                }
            }
            candidate.dictionary_evidence.sort();
            candidate.dictionary_evidence.dedup();
            candidate.rank_score = i64::from(candidate.total_cost) - bonus;
        }
        candidates.sort_by(|left, right| {
            left.rank_score
                .cmp(&right.rank_score)
                .then_with(|| left.total_cost.cmp(&right.total_cost))
                .then_with(|| left.vibrato_rank.cmp(&right.vibrato_rank))
        });
        candidates.truncate(top_n);
        candidates
    }

    pub fn choose_segmentation(
        &self,
        source: &AnnotatedToken,
        candidate: &SegmentationCandidate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.profile.set_segmentation_choice(source, candidate)
    }

    pub fn get_segmentation_choices(
        &self,
    ) -> Result<Vec<SegmentationChoice>, Box<dyn std::error::Error>> {
        self.profile.get_segmentation_choices()
    }

    pub fn delete_segmentation_choice(
        &self,
        surface: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(self.profile.delete_segmentation_choice(surface)?)
    }
}

fn dictionary_query_key(word: &str, reading: Option<&str>) -> String {
    format!("{}\u{1f}{}", word.trim(), reading.unwrap_or("*"))
}

fn is_kana(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| ('\u{3041}'..='\u{30ff}').contains(&character) || character == 'ー')
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reports_monotonic_real_pipeline_phases() {
        let dict_path = [
            "../../ipadic/system.dic",
            "../ipadic/system.dic",
            "ipadic/system.dic",
        ]
        .into_iter()
        .find(|path| std::path::Path::new(path).is_file());
        let Some(dict_path) = dict_path else {
            println!("测试跳过：未找到 IPADIC system.dic 字典文件。");
            return;
        };

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("kotoclip-progress-{nonce}"));
        let dictionary_directory = directory.join("dicts");
        std::fs::create_dir_all(&dictionary_directory).unwrap();
        let engine = Engine::new(
            dict_path,
            &dictionary_directory,
            directory.join("profile.sqlite"),
        )
        .unwrap();
        let mut events = Vec::new();
        let tokens = engine
            .analyze_text_with_progress("七日は警察署へ向かった。", true, |event| {
                events.push(event)
            })
            .unwrap();

        assert!(!tokens.is_empty());
        assert!(events
            .windows(2)
            .all(|pair| pair[0].percent <= pair[1].percent));
        for phase in [
            AnalysisPhase::Preparing,
            AnalysisPhase::Tokenizing,
            AnalysisPhase::Chunking,
            AnalysisPhase::GrammarMatching,
            AnalysisPhase::DictionaryMatching,
            AnalysisPhase::ProfileScoring,
            AnalysisPhase::RecordingExposure,
            AnalysisPhase::Completed,
        ] {
            assert!(
                events.iter().any(|event| event.phase == phase),
                "缺少进度阶段：{phase:?}"
            );
        }
        assert_eq!(events.last().unwrap().percent, 100);

        drop(engine);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
