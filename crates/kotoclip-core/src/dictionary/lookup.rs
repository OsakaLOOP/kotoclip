use crate::models::{DictEntry, DictionaryLink};
use ammonia::Builder;
use regex::Regex;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;
use unicode_normalization::UnicodeNormalization;

const MAX_DEFINITION_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct DictionaryStats {
    pub file_name: String,
    pub entry_count: usize,
    pub form_count: usize,
    pub reading_count: usize,
    pub schema_version: Option<u32>,
}

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
            if !file_path.is_file() || !matches!(ext, "db" | "sqlite") {
                continue;
            }
            let name = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            match Connection::open_with_flags(&file_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
                Ok(conn) => {
                    let compatible = conn.query_row(
                        "SELECT 1 FROM pragma_table_info('entries') WHERE name = 'reading'",
                        [],
                        |_| Ok(()),
                    );
                    if compatible.is_err() {
                        return Err(format!("词典 {:?} 缺少 reading 列，请先运行 scripts/migrate_dictionary_schema.py", file_path).into());
                    }
                    connections.push((name, conn));
                }
                Err(error) => log::warn!("无法打开词典 {:?}: {}", file_path, error),
            }
        }
        Ok(Self {
            connections,
            exists_cache: Mutex::new(HashMap::new()),
        })
    }

    pub fn stats(&self) -> Vec<DictionaryStats> {
        self.connections
            .iter()
            .map(|(name, conn)| {
                let count = |table: &str| -> usize {
                    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .unwrap_or(0)
                };
                DictionaryStats {
                    file_name: name.clone(),
                    entry_count: count("entries"),
                    form_count: count("entry_forms"),
                    reading_count: count("entry_readings"),
                    schema_version: conn
                        .query_row(
                            "SELECT schema_version FROM metadata ORDER BY rowid DESC LIMIT 1",
                            [],
                            |row| row.get(0),
                        )
                        .ok(),
                }
            })
            .collect()
    }

    pub fn match_kind(&self, headword: &str, reading: Option<&str>) -> Option<String> {
        if headword.is_empty() {
            return None;
        }
        let normalized = normalize_form(headword);
        if self.any_exists(
            "SELECT EXISTS(SELECT 1 FROM entry_forms WHERE normalized_form = ?1)",
            &normalized,
        ) || self.any_exists(
            "SELECT EXISTS(SELECT 1 FROM entries WHERE headword = ?1)",
            headword,
        ) {
            return Some("headword".to_string());
        }
        if is_kana_query(headword)
            && self.any_exists(
                "SELECT EXISTS(SELECT 1 FROM entry_readings WHERE normalized_reading = ?1)",
                &normalize_reading(headword),
            )
        {
            return Some("reading".to_string());
        }
        if let Some(reading) = reading.filter(|value| !value.is_empty() && *value != "*") {
            for normalized_reading in reading_candidates(headword, reading) {
                if self.any_exists(
                    "SELECT EXISTS(SELECT 1 FROM entry_readings WHERE normalized_reading = ?1)",
                    &normalized_reading,
                ) || self.any_exists(
                    "SELECT EXISTS(SELECT 1 FROM entries WHERE reading = ?1)",
                    &normalized_reading,
                ) {
                    return Some("reading".to_string());
                }
            }
        }
        None
    }

    fn any_exists(&self, sql: &str, value: &str) -> bool {
        self.connections.iter().any(|(_, conn)| {
            conn.query_row(sql, [value], |row| row.get::<_, bool>(0))
                .unwrap_or(false)
        })
    }

    pub fn contains_exact(&self, word: &str) -> bool {
        let normalized = normalize_form(word);
        if let Some(value) = self
            .exists_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&normalized).copied())
        {
            return value;
        }
        let value = self.connections.iter().any(|(_, conn)| {
            let structured = conn
                .prepare_cached(
                    "SELECT EXISTS(SELECT 1 FROM entry_forms WHERE normalized_form = ?1)",
                )
                .and_then(|mut statement| {
                    statement.query_row([&normalized], |row| row.get::<_, bool>(0))
                })
                .unwrap_or(false);
            structured
                || conn
                    .prepare_cached("SELECT EXISTS(SELECT 1 FROM entries WHERE headword = ?1)")
                    .and_then(|mut statement| {
                        statement.query_row([word], |row| row.get::<_, bool>(0))
                    })
                    .unwrap_or(false)
        });
        if let Ok(mut cache) = self.exists_cache.lock() {
            cache.insert(normalized, value);
        }
        value
    }

    /// 批量判断词条是否存在，避免章节扫描为每个候选分别往返 SQLite。
    pub fn contains_exact_batch(&self, words: &HashSet<String>) -> HashSet<String> {
        const BATCH_SIZE: usize = 2_000;
        let mut matched = HashSet::new();
        let candidates: Vec<_> = words
            .iter()
            .map(|word| (word.as_str(), normalize_form(word)))
            .collect();

        for batch in candidates.chunks(BATCH_SIZE) {
            let Ok(payload) = serde_json::to_string(batch) else {
                continue;
            };
            for (_, connection) in &self.connections {
                let sql =
                    "WITH candidates(word, normalized) AS (\
                         SELECT json_extract(value, '$[0]'), json_extract(value, '$[1]') \
                         FROM json_each(?1)\
                     ) \
                     SELECT DISTINCT candidates.word \
                     FROM candidates JOIN entry_forms \
                       ON entry_forms.normalized_form = candidates.normalized \
                     UNION \
                     SELECT DISTINCT candidates.word \
                     FROM candidates JOIN entries ON entries.headword = candidates.word";
                let Ok(mut statement) = connection.prepare(sql) else {
                    continue;
                };
                let Ok(rows) = statement.query_map([&payload], |row| row.get::<_, String>(0))
                else {
                    continue;
                };
                matched.extend(rows.flatten());
            }
        }

        matched
    }

    pub fn lookup(&self, headword: &str, reading: Option<&str>) -> Vec<DictEntry> {
        if headword.is_empty() {
            return Vec::new();
        }
        if let Some(target) = self.redirect_target(headword) {
            let redirected = self.lookup_direct(&target, reading);
            if !redirected.is_empty() {
                return redirected;
            }
        }
        self.lookup_direct(headword, reading)
    }

    fn lookup_direct(&self, headword: &str, reading: Option<&str>) -> Vec<DictEntry> {
        let normalized_headword = normalize_form(headword);
        let mut results = self.query_form(&normalized_headword);
        if results.is_empty() && is_kana_query(headword) {
            results = self.query_structured_reading(&normalize_reading(headword));
        }
        if results.is_empty() {
            results = self.query_exact("headword", headword, "headword");
        }
        if results.is_empty() {
            if let Some(reading) = reading.filter(|value| !value.is_empty()) {
                for normalized_reading in reading_candidates(headword, reading) {
                    results = self.query_structured_reading(&normalized_reading);
                    if results.is_empty() {
                        results = self.query_exact("reading", &normalized_reading, "reading");
                    }
                    if !results.is_empty() {
                        break;
                    }
                }
            }
        }
        if results.is_empty() {
            results = self.lookup_fuzzy(headword);
        }
        results
    }

    fn redirect_target(&self, headword: &str) -> Option<String> {
        for (_, conn) in &self.connections {
            let target = conn.query_row(
                "SELECT substr(definition, 9) FROM entries WHERE headword = ?1 AND definition LIKE '@@@LINK=%' LIMIT 1",
                [headword],
                |row| row.get::<_, String>(0),
            );
            if let Ok(target) = target {
                return Some(target);
            }
        }
        None
    }

    fn query_form(&self, value: &str) -> Vec<DictEntry> {
        let sql = "SELECT f.form, e.definition, e.dict_name \
                   FROM entry_forms f JOIN entries e ON e.id = f.entry_id \
                   WHERE f.normalized_form = ?1 AND e.definition NOT LIKE '@@@LINK=%' \
                   ORDER BY f.is_primary DESC, e.dict_name LIMIT 10";
        self.query_structured(sql, value, "headword")
    }

    fn query_structured_reading(&self, value: &str) -> Vec<DictEntry> {
        let sql = "SELECT COALESCE(\
                       (SELECT f.form FROM entry_forms f \
                        WHERE f.entry_id = e.id ORDER BY f.is_primary DESC LIMIT 1),\
                       e.headword), e.definition, e.dict_name \
                   FROM entry_readings r JOIN entries e ON e.id = r.entry_id \
                   WHERE r.normalized_reading = ?1 AND e.definition NOT LIKE '@@@LINK=%' \
                   ORDER BY r.is_primary DESC, e.dict_name LIMIT 10";
        self.query_structured(sql, value, "reading")
    }

    fn query_structured(&self, sql: &str, value: &str, match_type: &str) -> Vec<DictEntry> {
        let mut results = Vec::new();
        for (fallback_name, conn) in &self.connections {
            let Ok(mut stmt) = conn.prepare(sql) else {
                continue;
            };
            let Ok(rows) = stmt.query_map([value], |row| {
                Ok(self.entry(
                    row.get(2).unwrap_or_else(|_| fallback_name.clone()),
                    row.get(0)?,
                    row.get(1)?,
                    match_type,
                ))
            }) else {
                continue;
            };
            results.extend(rows.flatten());
        }
        results
    }

    fn query_exact(&self, column: &str, value: &str, match_type: &str) -> Vec<DictEntry> {
        let sql = format!("SELECT headword, definition, dict_name, reading FROM entries WHERE {column} = ?1 ORDER BY dict_name");
        let mut results = Vec::new();
        for (fallback_name, conn) in &self.connections {
            let Ok(mut stmt) = conn.prepare(&sql) else {
                continue;
            };
            let Ok(rows) = stmt.query_map([value], |row| {
                Ok(self.entry(
                    row.get(2).unwrap_or_else(|_| fallback_name.clone()),
                    row.get(0)?,
                    row.get(1)?,
                    match_type,
                ))
            }) else {
                continue;
            };
            results.extend(rows.flatten());
        }
        results
    }

    fn lookup_fuzzy(&self, word: &str) -> Vec<DictEntry> {
        let query = format!("\"{}\"", word.replace('"', ""));
        let mut results = Vec::new();
        for (fallback_name, conn) in &self.connections {
            let sql = "SELECT e.headword, e.definition, e.dict_name FROM entries_fts f JOIN entries e ON e.id = f.rowid WHERE f.headword MATCH ?1 LIMIT 5";
            let Ok(mut stmt) = conn.prepare(sql) else {
                continue;
            };
            let Ok(rows) = stmt.query_map([&query], |row| {
                Ok(self.entry(
                    row.get(2).unwrap_or_else(|_| fallback_name.clone()),
                    row.get(0)?,
                    row.get(1)?,
                    "fuzzy",
                ))
            }) else {
                continue;
            };
            results.extend(rows.flatten());
        }
        results
    }

    fn entry(
        &self,
        dict_name: String,
        headword: String,
        definition: String,
        match_type: &str,
    ) -> DictEntry {
        let links = extract_dictionary_links(&definition);
        let allowed: HashSet<&str> = [
            "p", "div", "span", "br", "ruby", "rt", "rp", "b", "strong", "i", "em", "ul", "ol",
            "li", "dl", "dt", "dd", "a", "hy", "table", "thead", "tbody", "tr", "th", "td",
            "sup", "sub", "small", "blockquote", "code", "mark", "vert", "v",
        ]
        .into_iter()
        .collect();
        let managed_definition = remove_managed_links(&definition, &headword, links.len());
        let link_re = Regex::new(r#"href=(['\"])entry://([^'\"]+)['\"]"#).unwrap();
        let navigable = link_re
            .replace_all(&managed_definition, |captures: &regex::Captures<'_>| {
                format!("href=\"https://kotoclip.invalid/entry/{}\"", &captures[2])
            })
            .into_owned();
        let clean = Builder::default()
            .tags(allowed)
            .add_tag_attributes("span", &["class"])
            .add_tag_attributes("div", &["class"])
            .add_tag_attributes("p", &["class"])
            .clean(&navigable)
            .to_string();
        let definition_html = if clean.len() > MAX_DEFINITION_BYTES {
            let mut limit = MAX_DEFINITION_BYTES;
            while limit > 0 && !clean.is_char_boundary(limit) {
                limit -= 1;
            }
            let mut truncated = clean[..limit].to_string();
            truncated.push_str("… [内容已截断]");
            truncated
        } else {
            clean
        };
        DictEntry {
            entry_key: format!("{dict_name}\u{1f}{headword}"),
            dict_name,
            headword,
            definition_html,
            match_type: match_type.to_string(),
            links,
        }
    }
}

fn extract_dictionary_links(definition: &str) -> Vec<DictionaryLink> {
    let redirect = definition.strip_prefix("@@@LINK=");
    if let Some(target) = redirect {
        return vec![DictionaryLink {
            target: target.trim().to_string(),
            label: target.trim().to_string(),
            relation: "redirect".to_string(),
        }];
    }
    let link_re = Regex::new(r#"<a\s+[^>]*href=(?:['\"])entry://([^'\"]+)(?:['\"])[^>]*>(.*?)</a>"#).unwrap();
    let tag_re = Regex::new(r"<[^>]+>").unwrap();
    let mut seen = HashSet::new();
    link_re
        .captures_iter(definition)
        .filter_map(|captures| {
            let target = captures.get(1)?.as_str().trim().to_string();
            if target.is_empty() || !seen.insert(target.clone()) {
                return None;
            }
            let label = tag_re.replace_all(captures.get(2)?.as_str(), "").trim().to_string();
            let before = &definition[..captures.get(0)?.start()];
            let relation = classify_link_relation(before);
            Some(DictionaryLink { target, label, relation: relation.to_string() })
        })
        .collect()
}

fn classify_link_relation(before: &str) -> &'static str {
    let last_parent = before.rfind("親項目");
    let last_child = before.rfind("子項目");
    if last_parent.is_some() && last_parent > last_child {
        return "parent";
    }
    if last_child.is_some() && last_child > last_parent {
        return "child";
    }
    let context: String = before
        .chars()
        .rev()
        .take(48)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if context.contains("対義") || context.contains("反義") || context.contains('⇔') {
        "antonym"
    } else if context.contains("類語") || context.contains("同義") || context.contains("同意") {
        "synonym"
    } else if context.contains('→') || context.contains('⇒') || context.contains("参照") {
        "reference"
    } else {
        "related"
    }
}

fn remove_managed_links(definition: &str, headword: &str, link_count: usize) -> String {
    if link_count >= 2 && is_kana_query(headword) {
        return String::new();
    }
    let structural = Regex::new(r"(?s)〈(?:親項目|子項目)〉.*?(</p>|$)").unwrap();
    let without_structural = structural.replace_all(definition, "$1");
    let anchors = Regex::new(r#"(?s)(?:⇔|→|⇒|☞)?\s*<a\s+[^>]*href=(?:['\"])entry://[^'\"]+(?:['\"])[^>]*>.*?</a>"#).unwrap();
    let without_anchors = anchors.replace_all(&without_structural, "");
    let separators = Regex::new(r"(?:\s|&nbsp;|；|;)+(</?(?:br|p)[^>]*>)").unwrap();
    separators.replace_all(&without_anchors, "$1").into_owned()
}

fn normalize_reading(value: &str) -> String {
    normalize_form(value)
        .chars()
        .flat_map(|c| {
            if ('\u{3041}'..='\u{3096}').contains(&c) {
                char::from_u32(c as u32 + 0x60)
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

fn normalize_form(value: &str) -> String {
    value
        .nfkc()
        .map(|character| match character {
            // 大辞林使用「繫」，现代常用输入及 IPADIC 常输出「繋」。
            '繋' => '繫',
            _ => character,
        })
        .filter(|c| {
            !c.is_whitespace() && !matches!(c, '・' | '･' | '-' | '‐' | '‑' | '‒' | '–' | '—' | '―')
        })
        .collect()
}

fn is_kana_query(value: &str) -> bool {
    let mut has_kana = false;
    for character in normalize_form(value).chars() {
        if ('\u{3041}'..='\u{30ff}').contains(&character) || character == 'ー' {
            has_kana = true;
        } else {
            return false;
        }
    }
    has_kana
}

fn reading_candidates(headword: &str, reading: &str) -> Vec<String> {
    let normalized = normalize_reading(reading);
    let mut candidates = vec![normalized.clone()];
    let Some(base_ending) = headword
        .chars()
        .last()
        .filter(|character| ('\u{3041}'..='\u{30ff}').contains(character))
    else {
        return candidates;
    };
    let normalized_ending = normalize_reading(&base_ending.to_string());
    let Some(ending) = normalized_ending.chars().next() else {
        return candidates;
    };
    let mut chars: Vec<char> = normalized.chars().collect();
    if let Some(last) = chars.last_mut() {
        if ('\u{30a1}'..='\u{30ff}').contains(last) && *last != ending {
            let inflection_row = match ending {
                'ウ' => "ワイウエオッ",
                'ク' => "カキクケコイッ",
                'グ' => "ガギグゲゴイッ",
                'ス' => "サシスセソ",
                'ツ' => "タチツテトッ",
                'ヌ' => "ナニヌネノン",
                'ブ' => "バビブベボン",
                'ム' => "マミムメモン",
                'ル' => "ラリルレロッ",
                _ => "",
            };
            let candidate = if inflection_row.contains(*last) {
                *last = ending;
                chars.into_iter().collect()
            } else {
                format!("{normalized}{ending}")
            };
            candidates.push(candidate);
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::DictionaryEngine;
    use rusqlite::Connection;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn structured_forms_readings_and_variants_resolve_real_entries() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("kotoclip-dictionary-{nonce}"));
        std::fs::create_dir_all(&directory).unwrap();
        let database = directory.join("test.sqlite");
        let connection = Connection::open(&database).unwrap();
        connection.execute_batch(
            r#"CREATE TABLE entries (id INTEGER PRIMARY KEY, headword TEXT NOT NULL, reading TEXT, definition TEXT NOT NULL, dict_name TEXT NOT NULL);
             CREATE TABLE metadata (schema_version INTEGER NOT NULL, source_name TEXT NOT NULL, imported_at TEXT NOT NULL);
             CREATE TABLE entry_forms (entry_id INTEGER NOT NULL, form TEXT NOT NULL, normalized_form TEXT NOT NULL, form_type TEXT NOT NULL, is_primary INTEGER NOT NULL, PRIMARY KEY(entry_id, normalized_form));
             CREATE TABLE entry_readings (entry_id INTEGER NOT NULL, reading TEXT NOT NULL, normalized_reading TEXT NOT NULL, is_primary INTEGER NOT NULL, PRIMARY KEY(entry_id, normalized_reading));
             CREATE VIRTUAL TABLE entries_fts USING fts5(headword, definition, content='entries', content_rowid='id', tokenize='trigram');
             INSERT INTO entries VALUES (1, 'けいさつしょ', NULL, '@@@LINK=けいさつしょ【警察署】', '测试词典');
             INSERT INTO entries VALUES (2, 'けいさつしょ【警察署】', NULL, '<p>警察署释义</p>', '测试词典');
             INSERT INTO entries VALUES (3, 'つなぐ【繫ぐ】', NULL, '<p>繫ぐ释义</p>', '测试词典');
             INSERT INTO entries VALUES (4, 'いる', NULL, '<p>☞ <a href="entry://いる【入る】">いる【入る】</a><br>☞ <a href="entry://いる【居る】">いる【居る】</a></p>', '测试词典');
             INSERT INTO entries VALUES (5, 'いる【入る】', NULL, '<p><span class="bss">いる</span> 入る释义</p>', '测试词典');
             INSERT INTO metadata VALUES (3, '测试词典', '2026-07-11T00:00:00Z');
             INSERT INTO entry_forms VALUES (1, 'けいさつしょ', 'けいさつしょ', 'kana', 1);
             INSERT INTO entry_forms VALUES (2, '警察署', '警察署', 'kanji', 1);
             INSERT INTO entry_forms VALUES (3, '繫ぐ', '繫ぐ', 'kanji', 1);
             INSERT INTO entry_forms VALUES (4, 'いる', 'いる', 'kana', 1);
             INSERT INTO entry_forms VALUES (5, '入る', '入る', 'mixed', 1);
             INSERT INTO entry_readings VALUES (1, 'けいさつしょ', 'ケイサツショ', 1);
             INSERT INTO entry_readings VALUES (2, 'けいさつしょ', 'ケイサツショ', 1);
             INSERT INTO entry_readings VALUES (3, 'つなぐ', 'ツナグ', 1);
             INSERT INTO entry_readings VALUES (4, 'いる', 'イル', 1);
             INSERT INTO entry_readings VALUES (5, 'いる', 'イル', 1);
             INSERT INTO entries_fts(entries_fts) VALUES('rebuild');"#
        ).unwrap();
        drop(connection);

        let engine = DictionaryEngine::new(&directory).unwrap();
        let kanji = engine.lookup("警察署", Some("ケイサツショ"));
        assert_eq!(kanji.len(), 1);
        assert_eq!(kanji[0].headword, "警察署");
        assert!(kanji[0].definition_html.contains("警察署释义"));

        let kana = engine.lookup("けいさつしょ", None);
        assert_eq!(kana.len(), 1);
        assert_eq!(kana[0].headword, "警察署");
        assert!(!kana[0].definition_html.contains("@@@LINK"));

        let variant = engine.lookup("繋ぐ", Some("ツナガ"));
        assert_eq!(variant.len(), 1);
        assert_eq!(variant[0].headword, "繫ぐ");
        assert!(engine.contains_exact("繋ぐ"));

        let navigation = engine.lookup("いる", None);
        assert!(navigation.iter().any(|entry| entry.links.len() == 2));
        let target = engine.lookup("いる【入る】", None);
        assert!(target.iter().any(|entry| entry.definition_html.contains("入る释义")));
        assert!(target.iter().any(|entry| entry.definition_html.contains("class=\"bss\"")));

        drop(engine);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
