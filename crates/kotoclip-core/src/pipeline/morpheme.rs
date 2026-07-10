use crate::models::{Morpheme, PosTag};
use vibrato::{Dictionary, Tokenizer};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// 形态素分析器，基于 vibrato 库
pub struct MorphemeAnalyzer {
    tokenizer: Tokenizer,
}

impl MorphemeAnalyzer {
    /// 构造函数，加载编译好的 Vibrato 二进制字典 (如 system.dic)
    pub fn new<P: AsRef<Path>>(dict_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(dict_path)?;
        let reader = BufReader::new(file);
        let dict = Dictionary::read(reader)?;
        let tokenizer = Tokenizer::new(dict);
        Ok(Self { tokenizer })
    }

    /// 对整句/整段文本执行形态素切分，返回结构化的 Morpheme 列表
    pub fn analyze(&self, text: &str) -> Vec<Morpheme> {
        let mut worker = self.tokenizer.new_worker();
        // 将文本重置到 worker 中
        worker.reset_sentence(text);
        worker.tokenize();

        let mut morphemes = Vec::with_capacity(worker.num_tokens());
        
        // 遍历所有 token
        for i in 0..worker.num_tokens() {
            let token = worker.token(i);
            let surface = token.surface().to_string();
            let feature = token.feature();
            
            // IPADIC 字典的列是用逗号分隔的
            let fields: Vec<&str> = feature.split(',').collect();
            
            let major = fields.get(0).cloned().unwrap_or("*").to_string();
            let sub1 = fields.get(1).cloned().unwrap_or("*").to_string();
            let sub2 = fields.get(2).cloned().unwrap_or("*").to_string();
            let sub3 = fields.get(3).cloned().unwrap_or("*").to_string();
            
            let pos = PosTag {
                major,
                sub1,
                sub2,
                sub3,
            };

            let conjugation_type = fields.get(4).cloned().unwrap_or("*").to_string();
            let conjugation_form = fields.get(5).cloned().unwrap_or("*").to_string();
            
            // 原形在 IPADIC 中一般是第 6 列。若为空或缺失则默认回退到 surface
            let mut base_form = fields.get(6).cloned().unwrap_or("*").to_string();
            if base_form == "*" || base_form.is_empty() {
                base_form = surface.clone();
            }

            // 読み在 IPADIC 中一般是第 7 列
            let reading = fields.get(7).cloned().unwrap_or("*").to_string();

            // 字符范围 [start, end)
            let char_range = (token.range_char().start, token.range_char().end);

            morphemes.push(Morpheme {
                surface,
                pos,
                base_form,
                reading,
                conjugation_type,
                conjugation_form,
                char_range,
            });
        }

        morphemes
    }
}
