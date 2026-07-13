use crate::models::{Morpheme, PosTag};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use vibrato::{Dictionary, Tokenizer};

/// 形态素分析器，基于 vibrato 库
pub struct MorphemeAnalyzer {
    tokenizer: Tokenizer,
}

pub struct MorphemeCandidate {
    pub morphemes: Vec<Morpheme>,
    pub total_cost: i32,
}

fn parse_morpheme(surface: &str, feature: &str, char_range: (usize, usize)) -> Morpheme {
    let fields: Vec<&str> = feature.split(',').collect();
    let pos = PosTag {
        major: fields.first().copied().unwrap_or("*").to_string(),
        sub1: fields.get(1).copied().unwrap_or("*").to_string(),
        sub2: fields.get(2).copied().unwrap_or("*").to_string(),
        sub3: fields.get(3).copied().unwrap_or("*").to_string(),
    };
    let mut base_form = fields.get(6).copied().unwrap_or("*").to_string();
    if base_form == "*" || base_form.is_empty() {
        base_form = surface.to_string();
    }
    Morpheme {
        surface: surface.to_string(),
        pos,
        base_form,
        reading: fields.get(7).copied().unwrap_or("*").to_string(),
        conjugation_type: fields.get(4).copied().unwrap_or("*").to_string(),
        conjugation_form: fields.get(5).copied().unwrap_or("*").to_string(),
        char_range,
    }
}

/// 修正系统词典无法表达、且 N-best 中不存在正确词性的已确认口语形。
/// 完整的口语音变与词汇别名处理应由后续独立模块接管。
fn apply_tokenization_compatibility(morphemes: Vec<Morpheme>) -> Vec<Morpheme> {
    let mut corrected = Vec::with_capacity(morphemes.len());
    let mut index = 0;
    while index < morphemes.len() {
        if index + 1 < morphemes.len()
            && morphemes[index].surface == "だっせ"
            && morphemes[index].base_form == "だっする"
            && morphemes[index].pos.major == "動詞"
            && morphemes[index + 1].surface == "え"
            && matches!(
                morphemes[index + 1].pos.major.as_str(),
                "フィラー" | "感動詞"
            )
        {
            corrected.push(Morpheme {
                surface: "だっせえ".to_string(),
                pos: PosTag {
                    major: "形容詞".to_string(),
                    sub1: "自立".to_string(),
                    sub2: "*".to_string(),
                    sub3: "*".to_string(),
                },
                base_form: "ダサい".to_string(),
                reading: "ダッセエ".to_string(),
                conjugation_type: "形容詞・アウオ段".to_string(),
                conjugation_form: "基本形".to_string(),
                char_range: (
                    morphemes[index].char_range.0,
                    morphemes[index + 1].char_range.1,
                ),
            });
            index += 2;
            continue;
        }
        corrected.push(morphemes[index].clone());
        index += 1;
    }
    corrected
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
            let range = token.range_char();
            morphemes.push(parse_morpheme(
                token.surface(),
                token.feature(),
                (range.start, range.end),
            ));
        }

        apply_tokenization_compatibility(morphemes)
    }

    /// 从 Vibrato lattice 获取带真实路径成本的 N-best 形态素序列。
    pub fn analyze_nbest(&self, text: &str, n: usize) -> Vec<MorphemeCandidate> {
        if text.is_empty() || n == 0 {
            return Vec::new();
        }
        let mut worker = self.tokenizer.new_worker();
        worker.reset_sentence(text);
        worker.tokenize_nbest(n);
        (0..worker.num_candidates())
            .map(|candidate| {
                let morphemes = (0..worker.candidate_num_tokens(candidate))
                    .map(|index| {
                        let token = worker.candidate_token(candidate, index);
                        let range = token.range_char();
                        parse_morpheme(token.surface(), token.feature(), (range.start, range.end))
                    })
                    .collect();
                MorphemeCandidate {
                    morphemes,
                    total_cost: worker.candidate_cost(candidate),
                }
            })
            .collect()
    }
}
