use crate::models::ExportEntry;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// 整个导出 JSON 的根包装结构
#[derive(Debug, Clone, Serialize)]
pub struct ExportRoot {
    pub version: String,
    pub exported_at: String,
    pub source_text_hash: String,
    pub entries: Vec<ExportEntryJson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportEntryJson {
    pub surface: String,
    pub base_form: String,
    pub reading: String,
    pub pos: String,
    pub grammar_tags: Vec<String>,
    pub jlpt_levels: Vec<u8>,
    pub context: ExportContext,
    pub definitions: Vec<crate::models::DictEntry>,
    pub user_note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportContext {
    pub sentence: String,
    pub highlight_range: (usize, usize),
}

fn extract_sentence_and_offset(text: &str, range: (usize, usize)) -> (String, (usize, usize)) {
    let chars: Vec<char> = text.chars().collect();
    let start_char = range.0;
    let end_char = range.1;

    if start_char >= chars.len() || end_char > chars.len() || start_char > end_char {
        return (text.to_string(), (0, text.chars().count()));
    }

    let terminators = ['。', '！', '?', '？', '!', '\n', '\r'];

    // 向前寻找句子的起始点
    let mut sentence_start = 0;
    for i in (0..start_char).rev() {
        if terminators.contains(&chars[i]) {
            sentence_start = i + 1;
            break;
        }
    }

    // 向后寻找句子的结束点
    let mut sentence_end = chars.len();
    for i in end_char..chars.len() {
        if terminators.contains(&chars[i]) {
            sentence_end = i;
            break;
        }
    }

    // 包含末尾终止符（如果它不是换行符）
    let mut actual_end = sentence_end;
    if actual_end < chars.len() && chars[actual_end] != '\n' && chars[actual_end] != '\r' {
        actual_end += 1;
    }

    let sentence: String = chars[sentence_start..actual_end].iter().collect();
    let rel_start = start_char - sentence_start;
    let rel_end = end_char - sentence_start;

    (sentence, (rel_start, rel_end))
}

/// 导出所选生词条目为结构化且与 Anki 兼容的 JSON 字符串
pub fn export_to_json(
    source_text: &str,
    entries: Vec<ExportEntry>,
) -> Result<String, serde_json::Error> {
    let canonical_text = crate::pipeline::ruby::prepare_text(source_text).text;

    // 1. 去重 (base_form, reading)，保留单上下文
    let mut seen = std::collections::HashSet::new();
    let mut unique_entries = Vec::new();
    for entry in entries {
        let key = (entry.base_form.clone(), entry.reading.clone());
        if seen.insert(key) {
            unique_entries.push(entry);
        }
    }

    let mut hasher = Sha256::new();
    hasher.update(canonical_text.as_bytes());
    let hash = format!("sha256:{:x}", hasher.finalize());
    let root = ExportRoot {
        version: "1.0".to_string(),
        // 使用 RFC 3339 格式获取当前 ISO 时间
        exported_at: chrono::Utc::now().to_rfc3339(),
        source_text_hash: hash,
        entries: unique_entries
            .into_iter()
            .map(|entry| {
                let (sentence, highlight) = if let Some(range) = entry.char_range {
                    extract_sentence_and_offset(&canonical_text, range)
                } else {
                    (entry.context_sentence.clone(), entry.context_highlight)
                };

                let mut jlpt_levels = entry.jlpt_levels.clone();
                jlpt_levels.sort_unstable();
                jlpt_levels.dedup();

                ExportEntryJson {
                    surface: entry.surface,
                    base_form: entry.base_form,
                    reading: entry.reading,
                    pos: entry.pos,
                    grammar_tags: entry.grammar_tags,
                    jlpt_levels,
                    context: ExportContext {
                        sentence,
                        highlight_range: highlight,
                    },
                    definitions: entry.definitions,
                    user_note: entry.user_note,
                }
            })
            .collect(),
    };
    serde_json::to_string_pretty(&root)
}
