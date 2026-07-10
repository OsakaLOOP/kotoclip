pub mod morpheme;
pub mod bunsetsu;
pub mod grammar;
pub mod restore;
pub mod ruby;

use crate::models::AnnotatedToken;
use std::path::Path;

/// NLP 处理引擎管线容器
pub struct Pipeline {
    morpheme_analyzer: morpheme::MorphemeAnalyzer,
    grammar_matcher: grammar::GrammarMatcher,
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
        if text.is_empty() {
            return Vec::new();
        }

        // 1. Strip author-supplied ruby before NLP, then apply it as the authoritative reading.
        let prepared = ruby::prepare_text(text);
        let mut morphemes = self.morpheme_analyzer.analyze(&prepared.text);
        ruby::override_morpheme_readings(
            &prepared.text,
            &mut morphemes,
            &prepared.annotations,
        );
        
        // 2. 文节组块 (包含用户自定义合并规则匹配)
        let bunsetsus = bunsetsu::chunk(&morphemes, merge_rules);
        let mut bunsetsus = ruby::merge_annotated_bunsetsus(bunsetsus, &prepared.annotations);
        ruby::override_bunsetsu_readings(
            &prepared.text,
            &mut bunsetsus,
            &prepared.annotations,
        );

        // 3. 语法模式匹配 (Aho-Corasick)
        self.grammar_matcher.match_patterns(&mut bunsetsus);

        // 4. 组装输出，设置默认状态 (曝光评分、Novelty 将在画像服务做二次加工)
        bunsetsus.into_iter().map(|b| {
            AnnotatedToken {
                bunsetsu: b,
                novelty_score: 1.0, // 默认为生词
                is_selected: false,
                is_known: false,
                inference_reason: None,
            }
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_smoke() {
        // 使用用户下载的 IPADIC 二进制字典路径进行测试
        let dict_path = "D:\\PROJ\\GIT\\kotoclip\\ipadic\\system.dic";
        
        // 如果文件不存在则跳过测试 (以便在缺失字典的环境中能够编译)
        if !std::path::Path::new(dict_path).exists() {
            println!("测试跳过：未找到 IPADIC system.dic 字典文件。");
            return;
        }

        let pipeline = Pipeline::new(dict_path).expect("初始化 NLP Pipeline 失败");
        
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
        assert_eq!(token.bunsetsu.head_word.base_form, "食べる", "辞书形还原不正确");
        
        // 断言其匹配到了 [使役受身] 和 [过去否定] 两个语法标签
        let has_causative_passive = token.bunsetsu.grammar_tags.iter().any(|t| t.pattern_id == "causative_passive");
        let has_past_negative = token.bunsetsu.grammar_tags.iter().any(|t| t.pattern_id == "past_negative");

        assert!(has_causative_passive, "未识别出 [使役受身] 语法结构");
        assert!(has_past_negative, "未识别出 [過去否定] 语法结构");
    }

    #[test]
    fn test_user_example_preserves_complete_text() {
        let dict_path = "D:\\PROJ\\GIT\\kotoclip\\ipadic\\system.dic";
        if !std::path::Path::new(dict_path).exists() {
            println!("测试跳过：未找到 IPADIC system.dic 字典文件。");
            return;
        }

        let text = "しらばっくれんなよ、古川。お前の素性は割れてんだ。マガツカミを連れ歩くはぐれ者が、正面から堂々と警察署に乗り込んでくるその度胸だきゃあ褒めてやろう。貴様、マガツカミを使って何をするつもりだった？";
        let pipeline = Pipeline::new(dict_path).expect("初始化 NLP Pipeline 失败");
        let tokens = pipeline.process(text, &[]);

        assert!(!tokens.is_empty(), "用户例文应生成至少一个 token");
        let reconstructed: String = tokens
            .iter()
            .map(|token| token.bunsetsu.surface.as_str())
            .collect();
        assert_eq!(reconstructed, text, "分词输出必须完整保留输入文本");
        assert!(tokens.iter().all(|token| !token.bunsetsu.morphemes.is_empty()));
        assert!(
            tokens
                .iter()
                .any(|token| token.bunsetsu.head_word.base_form == "警察署"),
            "名词接尾必须并入完整词典原形"
        );
    }

    #[test]
    fn test_authoritative_ruby_is_removed_and_overrides_reading() {
        let dict_path = "D:\\PROJ\\GIT\\kotoclip\\ipadic\\system.dic";
        if !std::path::Path::new(dict_path).exists() {
            println!("测试跳过：未找到 IPADIC system.dic 字典文件。");
            return;
        }

        let text = "七日は屈《かが》み、鬼怒川の腰元にぶら下がる鍵を奪う。\n  ついでに胸ポケットの煙草《たばこ》ももらっておいた。と、突然手首を掴まれる。\n\n  \u{3000}鋭い痛みに顔をしかめながらも、鬼怒川は眼光鋭く、七日を睨《にら》みつけていた。\n\n  「......古《ふる》川《かわ》。俺の名を覚えておけ。貴様、絶対に──」";
        let expected = "七日は屈み、鬼怒川の腰元にぶら下がる鍵を奪う。\n  ついでに胸ポケットの煙草ももらっておいた。と、突然手首を掴まれる。\n\n  \u{3000}鋭い痛みに顔をしかめながらも、鬼怒川は眼光鋭く、七日を睨みつけていた。\n\n  「......古川。俺の名を覚えておけ。貴様、絶対に──」";
        let pipeline = Pipeline::new(dict_path).expect("初始化 NLP Pipeline 失败");
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

        let annotated_morphemes: Vec<(&str, &str)> = tokens
            .iter()
            .flat_map(|token| token.bunsetsu.morphemes.iter())
            .map(|m| (m.surface.as_str(), m.reading.as_str()))
            .collect();
        assert!(annotated_morphemes.contains(&("屈み", "カガミ")));
        assert!(annotated_morphemes.contains(&("睨みつけ", "ニラミツケ")));
    }
}
