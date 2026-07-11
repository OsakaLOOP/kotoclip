pub mod models;
pub mod pipeline;
pub mod dictionary;
pub mod profile;
pub mod export;
pub mod ffi;
pub mod analysis_progress;

use std::path::Path;
use pipeline::Pipeline;
use dictionary::lookup::DictionaryEngine;
use profile::ProfileEngine;
use models::{AnnotatedToken, DictEntry, ExpressionRule, SegmentationCandidate};
use analysis_progress::{AnalysisPhase, AnalysisProgress};

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

    /// 解析一段文章，执行完整分词、语法提取，并自动从画像库标注生词分值 (Novelty)
    pub fn analyze_text(&self, text: &str) -> Result<Vec<AnnotatedToken>, Box<dyn std::error::Error>> {
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
        report(AnalysisProgress::stage(AnalysisPhase::Preparing, 1, "准备分析与表达规则"));
        // 历史 user_merge_rules 不再进入正式 Pipeline。文节边界保持由 NLP 与
        // 词典边界解析器决定；用户拖拽仅创建独立的跨文节表达注解。
        let mut tokens = self.pipeline.process_with_progress(text, &[], &mut report);
        let token_count = tokens.len();
        let report_step = (token_count / 100).max(1);
        report(AnalysisProgress::counted(
            AnalysisPhase::DictionaryMatching,
            0,
            token_count,
            55,
            "按词典校正词汇边界",
        ));
        for (index, token) in tokens.iter_mut().enumerate() {
            pipeline::bunsetsu::resolve_lexical_boundaries(
                std::slice::from_mut(&mut token.bunsetsu),
                |word| self.dictionary.contains_exact(word),
            );
            let completed = index + 1;
            if completed == token_count || completed % report_step == 0 {
                let percent = 55 + ((completed * 5 / token_count.max(1)) as u8);
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
        let mut annotated = self.profile.annotate_tokens_with_progress(tokens, |completed, total| {
            let percent = 61 + ((completed * 35 / total.max(1)) as u8);
            report(AnalysisProgress::counted(
                AnalysisPhase::ProfileScoring,
                completed,
                total,
                percent,
                "计算词汇熟悉度",
            ));
        })?;
        // 跨文节表达是独立注解层：画像评分完成后应用，不重写 NLP 文节结构。
        self.profile.apply_expression_rules(&mut annotated)?;
        pipeline::expressions::apply_builtin_expressions(&mut annotated);
        if record_exposure {
            self.profile.record_token_exposures_with_progress(&annotated, |completed, total| {
                let percent = 97 + ((completed * 2 / total.max(1)) as u8);
                report(AnalysisProgress::counted(
                    AnalysisPhase::RecordingExposure,
                    completed,
                    total,
                    percent,
                    "记录本次词汇曝光",
                ));
            })?;
        }
        report(AnalysisProgress::stage(AnalysisPhase::Completed, 100, "分析完成"));
        Ok(annotated)
    }

    /// 记录生词的曝光历史 (由阅读流自动驱动)
    pub fn record_exposure(&self, base_form: &str, reading: &str, pos: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.profile.record_exposure(base_form, reading, pos)?;
        Ok(())
    }

    /// 主动标记单词为“已知” (脱下胶囊)
    pub fn mark_known(&self, base_form: &str, reading: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.profile.mark_known(base_form, reading)?;
        Ok(())
    }

    /// 主动标记单词为“未知”
    pub fn mark_unknown(&self, base_form: &str, reading: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.profile.mark_unknown(base_form, reading)?;
        Ok(())
    }

    /// 从词库中查询单词释义，并按传入的词典优先级顺序列表进行重排聚合
    pub fn lookup_word(&self, word: &str, reading: Option<&str>, priority_list: &[String]) -> Vec<DictEntry> {
        let raw_entries = self.dictionary.lookup(word, reading);
        dictionary::aggregate::sort_definitions(raw_entries, priority_list)
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
        slot_indices: &[usize],
    ) -> Result<ExpressionRule, Box<dyn std::error::Error>> {
        self.profile.add_expression_rule(tokens, label, description, slot_indices)
    }

    pub fn get_expression_rules(&self) -> Result<Vec<ExpressionRule>, Box<dyn std::error::Error>> {
        self.profile.get_expression_rules()
    }

    pub fn delete_expression_rule(&self, id: i64) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(self.profile.delete_expression_rule(id)?)
    }

    pub fn split_token(&self, token: &AnnotatedToken) -> Vec<AnnotatedToken> {
        pipeline::candidates::split_token(token)
    }

    pub fn get_candidates(
        &self,
        token: &AnnotatedToken,
        top_n: usize,
    ) -> Vec<SegmentationCandidate> {
        pipeline::candidates::get_candidates(token, top_n)
    }
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

        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let directory = std::env::temp_dir().join(format!("kotoclip-progress-{nonce}"));
        let dictionary_directory = directory.join("dicts");
        std::fs::create_dir_all(&dictionary_directory).unwrap();
        let engine = Engine::new(dict_path, &dictionary_directory, directory.join("profile.sqlite")).unwrap();
        let mut events = Vec::new();
        let tokens = engine
            .analyze_text_with_progress("七日は警察署へ向かった。", true, |event| events.push(event))
            .unwrap();

        assert!(!tokens.is_empty());
        assert!(events.windows(2).all(|pair| pair[0].percent <= pair[1].percent));
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
            assert!(events.iter().any(|event| event.phase == phase), "缺少进度阶段：{phase:?}");
        }
        assert_eq!(events.last().unwrap().percent, 100);

        drop(engine);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
