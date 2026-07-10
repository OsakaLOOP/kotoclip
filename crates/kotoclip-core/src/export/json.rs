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

/// 导出所选生词条目为结构化且与 Anki 兼容的 JSON 字符串
pub fn export_to_json(source_text: &str, entries: Vec<ExportEntry>) -> Result<String, serde_json::Error> {
    let canonical_text = crate::pipeline::ruby::prepare_text(source_text).text;
    let mut hasher = Sha256::new();
    hasher.update(canonical_text.as_bytes());
    let hash = format!("sha256:{:x}", hasher.finalize());
    let root = ExportRoot {
        version: "1.0".to_string(),
        // 使用 RFC 3339 格式获取当前 ISO 时间
        exported_at: chrono::Utc::now().to_rfc3339(),
        source_text_hash: hash,
        entries: entries.into_iter().map(|entry| ExportEntryJson {
            surface: entry.surface,
            base_form: entry.base_form,
            reading: entry.reading,
            pos: entry.pos,
            grammar_tags: entry.grammar_tags,
            jlpt_levels: entry.jlpt_levels,
            context: ExportContext { sentence: entry.context_sentence, highlight_range: entry.context_highlight },
            definitions: entry.definitions,
            user_note: entry.user_note,
        }).collect(),
    };
    serde_json::to_string_pretty(&root)
}
