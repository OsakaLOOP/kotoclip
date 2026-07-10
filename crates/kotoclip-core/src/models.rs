use serde::{Serialize, Deserialize};

/// 单个形态素 (vibrato 输出的最细粒度 token)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Morpheme {
    /// 表层形 (原文中的文字)
    pub surface: String,
    /// 结构化词性标签
    pub pos: PosTag,
    /// 辞书形 / 原形
    pub base_form: String,
    /// 読み (片假名)
    pub reading: String,
    /// 活用型 (如 "五段・カ行" 等)
    pub conjugation_type: String,
    /// 活用形 (如 "連用形・一般" 等)
    pub conjugation_form: String,
    /// 在原文中的字符偏移范围 [start, end)
    pub char_range: (usize, usize),
}

/// 结构化词性标签
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PosTag {
    /// 品词 (如 名词, 动词, 助词)
    pub major: String,
    /// 品词细分类 1
    pub sub1: String,
    /// 品词细分类 2
    pub sub2: String,
    /// 品词细分类 3
    pub sub3: String,
}

/// 文节 (Bunsetsu) — 自立语 + 附属语的聚合块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bunsetsu {
    /// 构成该文节的全部形态素
    pub morphemes: Vec<Morpheme>,
    /// 文节整体的表层形 (拼接所得)
    pub surface: String,
    /// 核心自立语 (用于查词的本体)
    pub head_word: HeadWord,
    /// 匹配到的语法标签
    pub grammar_tags: Vec<GrammarTag>,
    /// 原文字符偏移范围 [start, end)
    pub char_range: (usize, usize),
}

/// 核心自立语
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadWord {
    /// 核心词的表层形
    pub surface: String,
    /// 辞书形 (用于词典检索)
    pub base_form: String,
    /// 読み
    pub reading: String,
    /// 词性
    pub pos: PosTag,
}

/// 语法标签
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarTag {
    /// 模式 ID (如 "causative_passive")
    pub pattern_id: String,
    /// 日文名 (如 "使役受身")
    pub name_ja: String,
    /// 英文名 (如 "Causative Passive")
    pub name_en: String,
    pub jlpt_level: Option<u8>,
    /// 简要说明
    pub description: String,
}

/// 带用户画像评分的最终输出 Token (给前端渲染)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedToken {
    /// 基础文节数据
    pub bunsetsu: Bunsetsu,
    /// 新颖度评分 / 生词权重 (0.0 = 已知, 1.0 = 完全未知)
    pub novelty_score: f32,
    /// 用户是否选中
    pub is_selected: bool,
    /// 用户是否标记为已知
    pub is_known: bool,
    /// 推断理由
    pub inference_reason: Option<String>,
}

/// 导出用的选词条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportEntry {
    /// 原文表层形
    pub surface: String,
    /// 辞书形
    pub base_form: String,
    /// 読み
    pub reading: String,
    /// 词性简写
    pub pos: String,
    /// 语法标签列表
    pub grammar_tags: Vec<String>,
    /// 所在整句原文
    pub context_sentence: String,
    /// 句中高亮偏移范围 [start, end)
    pub context_highlight: (usize, usize),
    /// 词典释义列表
    pub definitions: Vec<DictEntry>,
}

/// 词典条目 (来自 MDict SQLite)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictEntry {
    /// 词典名 (如 大辞林, 新明解)
    pub dict_name: String,
    /// 词头
    pub headword: String,
    /// HTML 格式释义
    pub definition_html: String,
}
