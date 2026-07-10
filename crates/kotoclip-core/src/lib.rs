pub mod models;
pub mod pipeline;
pub mod dictionary;
pub mod profile;
pub mod export;
pub mod ffi;

use std::path::Path;
use pipeline::Pipeline;
use dictionary::lookup::DictionaryEngine;
use profile::ProfileEngine;
use models::{AnnotatedToken, DictEntry, SegmentationCandidate};

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
        // 先拉取用户自定义的短语合并规则
        let merge_rules = self.profile.get_merge_rules().unwrap_or_default();
        let tokens = self.pipeline.process(text, &merge_rules);
        // 调用画像引擎打分标注
        let annotated = self.profile.annotate_tokens(tokens)?;
        if record_exposure {
            self.profile.record_token_exposures(&annotated)?;
        }
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
    pub fn lookup_word(&self, word: &str, priority_list: &[String]) -> Vec<DictEntry> {
        let raw_entries = self.dictionary.lookup(word);
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
