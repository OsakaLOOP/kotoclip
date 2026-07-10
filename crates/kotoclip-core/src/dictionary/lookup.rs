use crate::models::DictEntry;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

/// SQLite 词库查词引擎，管理多个已转录的 MDict SQLite 数据库连接
pub struct DictionaryEngine {
    /// 包含 (数据库名, 只读连接) 的列表
    connections: Vec<(String, Connection)>,
}

impl DictionaryEngine {
    /// 构造函数：扫描指定目录下的所有 `.db` 和 `.sqlite` 文件并初始化只读连接
    pub fn new<P: AsRef<Path>>(dicts_dir: P) -> Result<Self, Box<dyn std::error::Error>> {
        let mut connections = Vec::new();
        let path = dicts_dir.as_ref();

        // 确保目录存在。如果不存在，则静默创建它，保证引擎能正常启动 (即使开始时没有任何字典)
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }

        if path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if file_path.is_file() {
                        let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");
                        if ext == "db" || ext == "sqlite" {
                            let db_name = file_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            
                            // 以只读模式 (Read-Only) 安全地打开词典数据库，避免意外篡改词库数据
                            match Connection::open_with_flags(&file_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
                                Ok(conn) => {
                                    connections.push((db_name, conn));
                                }
                                Err(e) => {
                                    log::warn!("无法打开词典数据库 {:?}: {}", file_path, e);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(Self { connections })
    }

    /// 执行查词：首选精确查询，若无结果，再进行全文检索 (FTS5 trigram) 模糊查询
    pub fn lookup(&self, word: &str) -> Vec<DictEntry> {
        if word.is_empty() {
            return Vec::new();
        }

        let mut results = self.lookup_exact(word);
        if results.is_empty() {
            results = self.lookup_fuzzy(word);
        }
        results
    }

    /// 1. 精确查询 (主路径)
    fn lookup_exact(&self, word: &str) -> Vec<DictEntry> {
        let mut results = Vec::new();
        
        for (dict_name, conn) in &self.connections {
            let sql = "SELECT headword, definition, dict_name FROM entries WHERE headword = ?1";
            let mut stmt = match conn.prepare(sql) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let rows = stmt.query_map([word], |row| {
                Ok(DictEntry {
                    dict_name: row.get::<_, String>(2).unwrap_or_else(|_| dict_name.clone()),
                    headword: row.get(0)?,
                    definition_html: row.get(1)?,
                })
            });

            if let Ok(mapped_rows) = rows {
                for item in mapped_rows.flatten() {
                    results.push(item);
                }
            }
        }
        
        results
    }

    /// 2. 基于 SQLite FTS5 trigram 全文检索的回退模糊检索 (限制每本词典返回前 5 条)
    fn lookup_fuzzy(&self, word: &str) -> Vec<DictEntry> {
        let mut results = Vec::new();
        
        // 构造 FTS5 MATCH 查询词条，FTS5 检索词通常双引号包裹以进行子串词匹配
        let query = format!("\"{}\"", word.replace('"', ""));

        for (dict_name, conn) in &self.connections {
            // 使用外部内容表的 JOIN 查询防止可能的数据不同步导致出错，限制返回 5 项防止溢出
            let sql = "
                SELECT e.headword, e.definition, e.dict_name 
                FROM entries_fts f 
                JOIN entries e ON e.id = f.rowid 
                WHERE f.headword MATCH ?1 
                LIMIT 5
            ";
            
            let mut stmt = match conn.prepare(sql) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let rows = stmt.query_map([&query], |row| {
                Ok(DictEntry {
                    dict_name: row.get::<_, String>(2).unwrap_or_else(|_| dict_name.clone()),
                    headword: row.get(0)?,
                    definition_html: row.get(1)?,
                })
            });

            if let Ok(mapped_rows) = rows {
                for item in mapped_rows.flatten() {
                    results.push(item);
                }
            }
        }
        
        results
    }
}
