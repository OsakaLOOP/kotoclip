use crate::models::Bunsetsu;

/// 智能还原：根据文节形态素组成，提取并重构最适合查词的核心辞书形
pub fn restore_base_form(bunsetsu: &Bunsetsu) -> String {
    let morphemes = &bunsetsu.morphemes;
    if morphemes.is_empty() {
        return String::new();
    }

    // 1. 检查萨变动词复合结构 (如 "勉強" + "する")
    // 若第一个形态素是 [名词, サ変接続]，且后续跟着动词 "する"
    if morphemes.len() >= 2 {
        let first = &morphemes[0];
        let second = &morphemes[1];
        if first.pos.major == "名詞" && first.pos.sub1 == "サ変接続" && second.base_form == "する"
        {
            // 返回名词形式 "勉強"，在 MDict 中查名词释义比查 "勉強する" 成功率更高
            return first.base_form.clone();
        }
    }

    // 2. 检查复合动词结构 (如 "走り" + "抜ける" -> "走り抜ける")
    // 若文节内包含多个自立动词，且最后一个是动词，前面的动词通常为连用形
    let verb_indices: Vec<usize> = morphemes
        .iter()
        .enumerate()
        .filter(|(_, m)| m.pos.major == "動詞" && m.pos.sub1 == "自立")
        .map(|(i, _)| i)
        .collect();

    if verb_indices.len() >= 2 {
        // 拼接前面动词的表层形与最后一个动词的辞书形
        let mut composite_base = String::new();
        let last_idx = *verb_indices.last().unwrap();
        for (i, m) in morphemes.iter().enumerate() {
            if i < last_idx {
                // 如果是最后一个动词之前的动词，使用它的表层形 (如 "走り")
                composite_base.push_str(&m.surface);
            } else if i == last_idx {
                // 最后一个动词使用它的原形/辞书形 (如 "抜ける")
                composite_base.push_str(&m.base_form);
                break;
            }
        }
        return composite_base;
    }

    // 3. 形容动词的处理
    // IPADIC 将 "静か" 标为 [名词, 形容动词语干]，其辞书形应当是 "静か" 或 "静かだ"
    let head = &bunsetsu.head_word;
    if head.pos.major == "名詞" && head.pos.sub1 == "形容動詞語幹" {
        return head.base_form.clone();
    }

    // 默认情况：返回文节提取出的核心词基本形
    head.base_form.clone()
}
