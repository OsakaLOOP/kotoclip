use crate::models::DictEntry;
use ammonia::Builder;
use rusqlite::{Connection, OpenFlags};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

const MAX_DEFINITION_BYTES: usize = 512 * 1024;

pub struct DictionaryEngine {
    connections: Vec<(String, Connection)>,
    exists_cache: Mutex<HashMap<String, bool>>,
}

impl DictionaryEngine {
    pub fn new<P: AsRef<Path>>(dicts_dir: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path = dicts_dir.as_ref();
        std::fs::create_dir_all(path)?;
        let mut connections = Vec::new();
        for entry in std::fs::read_dir(path)?.flatten() {
            let file_path = entry.path();
            let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if !file_path.is_file() || !matches!(ext, "db" | "sqlite") { continue; }
            let name = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
            match Connection::open_with_flags(&file_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
                Ok(conn) => {
                    let compatible = conn.query_row("SELECT 1 FROM pragma_table_info('entries') WHERE name = 'reading'", [], |_| Ok(()));
                    if compatible.is_err() {
                        return Err(format!("词典 {:?} 缺少 reading 列，请先运行 scripts/migrate_dictionary_schema.py", file_path).into());
                    }
                    connections.push((name, conn));
                }
                Err(error) => log::warn!("无法打开词典 {:?}: {}", file_path, error),
            }
        }
        Ok(Self { connections, exists_cache: Mutex::new(HashMap::new()) })
    }

    pub fn contains_exact(&self, word: &str) -> bool {
        if let Some(value) = self.exists_cache.lock().ok().and_then(|cache| cache.get(word).copied()) { return value; }
        let value = self.connections.iter().any(|(_, conn)| conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM entries WHERE headword = ?1)", [word], |row| row.get::<_, bool>(0)
        ).unwrap_or(false));
        if let Ok(mut cache) = self.exists_cache.lock() { cache.insert(word.to_string(), value); }
        value
    }

    pub fn lookup(&self, headword: &str, reading: Option<&str>) -> Vec<DictEntry> {
        if headword.is_empty() { return Vec::new(); }
        let mut results = self.query_exact("headword", headword, "headword");
        if results.is_empty() {
            if let Some(reading) = reading.filter(|value| !value.is_empty()) {
                results = self.query_exact("reading", &normalize_reading(reading), "reading");
            }
        }
        if results.is_empty() { results = self.lookup_fuzzy(headword); }
        results
    }

    fn query_exact(&self, column: &str, value: &str, match_type: &str) -> Vec<DictEntry> {
        let sql = format!("SELECT headword, definition, dict_name, reading FROM entries WHERE {column} = ?1 ORDER BY dict_name");
        let mut results = Vec::new();
        for (fallback_name, conn) in &self.connections {
            let Ok(mut stmt) = conn.prepare(&sql) else { continue };
            let Ok(rows) = stmt.query_map([value], |row| Ok(self.entry(
                row.get(2).unwrap_or_else(|_| fallback_name.clone()), row.get(0)?, row.get(1)?, match_type
            ))) else { continue };
            results.extend(rows.flatten());
        }
        results
    }

    fn lookup_fuzzy(&self, word: &str) -> Vec<DictEntry> {
        let query = format!("\"{}\"", word.replace('"', ""));
        let mut results = Vec::new();
        for (fallback_name, conn) in &self.connections {
            let sql = "SELECT e.headword, e.definition, e.dict_name FROM entries_fts f JOIN entries e ON e.id = f.rowid WHERE f.headword MATCH ?1 LIMIT 5";
            let Ok(mut stmt) = conn.prepare(sql) else { continue };
            let Ok(rows) = stmt.query_map([&query], |row| Ok(self.entry(
                row.get(2).unwrap_or_else(|_| fallback_name.clone()), row.get(0)?, row.get(1)?, "fuzzy"
            ))) else { continue };
            results.extend(rows.flatten());
        }
        results
    }

    fn entry(&self, dict_name: String, headword: String, definition: String, match_type: &str) -> DictEntry {
        let allowed: HashSet<&str> = ["p", "div", "span", "br", "ruby", "rt", "rp", "b", "strong", "i", "em", "ul", "ol", "li", "dl", "dt", "dd", "a"].into_iter().collect();
        let clean = Builder::default().tags(allowed).clean(&definition).to_string();
        let definition_html = if clean.len() > MAX_DEFINITION_BYTES {
            let mut truncated = clean[..MAX_DEFINITION_BYTES].to_string();
            while !truncated.is_char_boundary(truncated.len()) { truncated.pop(); }
            truncated.push_str("… [内容已截断]");
            truncated
        } else { clean };
        DictEntry { dict_name, headword, definition_html, match_type: match_type.to_string() }
    }
}

fn normalize_reading(value: &str) -> String {
    value.nfkc().flat_map(|c| {
        if ('\u{3041}'..='\u{3096}').contains(&c) { char::from_u32(c as u32 + 0x60).into_iter().collect::<Vec<_>>() } else { vec![c] }
    }).collect()
}
