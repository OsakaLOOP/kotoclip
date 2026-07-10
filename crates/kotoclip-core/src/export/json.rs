use crate::models::ExportEntry;
use serde::Serialize;

/// 整个导出 JSON 的根包装结构
#[derive(Debug, Clone, Serialize)]
pub struct ExportRoot {
    pub version: String,
    pub exported_at: String,
    pub entries: Vec<ExportEntry>,
}

/// 导出所选生词条目为结构化且与 Anki 兼容的 JSON 字符串
pub fn export_to_json(entries: Vec<ExportEntry>) -> Result<String, serde_json::Error> {
    let root = ExportRoot {
        version: "1.0".to_string(),
        // 使用 RFC 3339 格式获取当前 ISO 时间
        exported_at: chrono::Utc::now().to_rfc3339(),
        entries,
    };
    serde_json::to_string_pretty(&root)
}
