use crate::dictionary::{aggregate, bundle, lookup_state, presentation};
use crate::models::{
    DictEntry, DictionaryEntryRef, DictionaryLookup, DictionaryLookupTiming,
    DictionaryMatchEvidence, PosTag,
};
use flate2::read::ZlibDecoder;
use rusqlite::{Connection, OpenFlags, Row};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use unicode_normalization::UnicodeNormalization;

const FORM_KEY: i64 = 0;
const READING_KEY: i64 = 1;
const DEFINITION_CACHE_BLOCKS: usize = 64;
const PRESENTATION_CACHE_ENTRIES: usize = 128;

#[derive(Debug, Clone, Serialize)]
pub struct DictionaryStats {
    pub file_name: String,
    pub source_name: String,
    pub entry_count: usize,
    pub alias_count: usize,
    pub form_count: usize,
    pub reading_count: usize,
    pub definition_block_count: usize,
    pub schema_version: u32,
}

struct DictionaryDatabase {
    name: String,
    file_name: String,
    connection: Connection,
}

struct RawEntry {
    entry_id: i64,
    headword: String,
    raw_headword: String,
    definition_block_id: i64,
    definition_offset: usize,
    definition_length: usize,
    reading: Option<String>,
}

#[derive(Default)]
struct DefinitionBlockCache {
    blocks: HashMap<(usize, i64), Arc<Vec<u8>>>,
    order: VecDeque<(usize, i64)>,
}

#[derive(Default)]
struct PresentationCache {
    entries: HashMap<(usize, i64), Vec<presentation::DictionaryPresentation>>,
    order: VecDeque<(usize, i64)>,
}

impl PresentationCache {
    fn get(&mut self, key: (usize, i64)) -> Option<Vec<presentation::DictionaryPresentation>> {
        let value = self.entries.get(&key)?.clone();
        self.order.retain(|candidate| *candidate != key);
        self.order.push_back(key);
        Some(value)
    }

    fn insert(&mut self, key: (usize, i64), value: Vec<presentation::DictionaryPresentation>) {
        self.entries.insert(key, value);
        self.order.retain(|candidate| *candidate != key);
        self.order.push_back(key);
        while self.order.len() > PRESENTATION_CACHE_ENTRIES {
            if let Some(expired) = self.order.pop_front() {
                self.entries.remove(&expired);
            }
        }
    }
}

impl DefinitionBlockCache {
    fn get(&mut self, key: (usize, i64)) -> Option<Arc<Vec<u8>>> {
        let value = self.blocks.get(&key)?.clone();
        self.order.retain(|candidate| *candidate != key);
        self.order.push_back(key);
        Some(value)
    }

    fn insert(&mut self, key: (usize, i64), value: Arc<Vec<u8>>) {
        self.blocks.insert(key, value);
        self.order.retain(|candidate| *candidate != key);
        self.order.push_back(key);
        while self.order.len() > DEFINITION_CACHE_BLOCKS {
            if let Some(expired) = self.order.pop_front() {
                self.blocks.remove(&expired);
            }
        }
    }
}

pub struct DictionaryEngine {
    databases: Vec<DictionaryDatabase>,
    exists_cache: Mutex<HashMap<String, bool>>,
    definition_cache: Mutex<DefinitionBlockCache>,
    presentation_cache: Mutex<PresentationCache>,
}

impl DictionaryEngine {
    pub fn prepare<P1: AsRef<Path>, P2: AsRef<Path>>(
        source_dir: P1,
        database_dir: P2,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        bundle::prepare_dictionary_sources(source_dir, &database_dir)?;
        Self::new(database_dir)
    }

    pub fn new<P: AsRef<Path>>(dicts_dir: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path = dicts_dir.as_ref();
        std::fs::create_dir_all(path)?;
        let mut files = std::fs::read_dir(path)?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                let extension = path.extension().and_then(|value| value.to_str());
                path.is_file() && matches!(extension, Some("db" | "sqlite"))
            })
            .collect::<Vec<_>>();
        files.sort();

        let mut databases = Vec::new();
        for file_path in files {
            let file_name = file_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown.db")
                .to_string();
            let connection =
                Connection::open_with_flags(&file_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let Ok(schema_version) = connection.query_row(
                "SELECT schema_version FROM metadata WHERE id = 1",
                [],
                |row| row.get::<_, u32>(0),
            ) else {
                log::warn!(
                    "忽略旧词典 {}；当前运行时只读取 schema v4",
                    file_path.display()
                );
                continue;
            };
            if schema_version != bundle::SCHEMA_VERSION {
                log::warn!(
                    "忽略词典 {}：schema={}，当前仅支持 schema v{}",
                    file_path.display(),
                    schema_version,
                    bundle::SCHEMA_VERSION
                );
                continue;
            }
            let source_name = connection.query_row(
                "SELECT source_name FROM metadata WHERE id = 1",
                [],
                |row| row.get(0),
            )?;
            databases.push(DictionaryDatabase {
                name: source_name,
                file_name,
                connection,
            });
        }
        Ok(Self {
            databases,
            exists_cache: Mutex::new(HashMap::new()),
            definition_cache: Mutex::new(DefinitionBlockCache::default()),
            presentation_cache: Mutex::new(PresentationCache::default()),
        })
    }

    pub fn stats(&self) -> Vec<DictionaryStats> {
        self.databases
            .iter()
            .map(|database| {
                let count = |sql: &str| -> usize {
                    database
                        .connection
                        .query_row(sql, [], |row| row.get(0))
                        .unwrap_or(0)
                };
                DictionaryStats {
                    file_name: database.file_name.clone(),
                    source_name: database.name.clone(),
                    entry_count: count("SELECT count(*) FROM entries"),
                    alias_count: count("SELECT count(*) FROM aliases"),
                    form_count: count(
                        "SELECT (SELECT count(*) FROM entry_keys WHERE kind = 0) + (SELECT count(*) FROM alias_keys WHERE kind = 0)",
                    ),
                    reading_count: count(
                        "SELECT (SELECT count(*) FROM entry_keys WHERE kind = 1) + (SELECT count(*) FROM alias_keys WHERE kind = 1)",
                    ),
                    definition_block_count: count("SELECT count(*) FROM definition_blocks"),
                    schema_version: bundle::SCHEMA_VERSION,
                }
            })
            .collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.databases
            .iter()
            .map(|database| database.name.clone())
            .collect()
    }

    /// 构造稳定的“表记 × 全部词典”矩阵，并只加载当前表记的正文。
    pub fn lookup_matrix_profiled(
        &self,
        query: &str,
        observed_form: Option<&str>,
        reading: Option<&str>,
        pos: Option<&PosTag>,
        selected_form: Option<&str>,
        priority_list: &[String],
    ) -> DictionaryLookup {
        let started = Instant::now();
        let (discovery_entries, mut timing) = self.lookup_profiled_with_pos(query, reading, pos);
        let dictionary_names = ordered_dictionary_names(self.names(), priority_list);
        let mut seeds =
            lookup_state::collect_form_seeds(query, observed_form, reading, &discovery_entries);
        self.complete_form_availability(&mut seeds, &discovery_entries, &mut timing);
        let forms = lookup_state::build_form_groups(seeds, &dictionary_names);
        let selected_form_id = lookup_state::selected_form_id(&forms, selected_form);
        let mut entries = Vec::new();
        if let Some(active_form) = lookup_state::selected_form(&forms, selected_form_id.as_deref())
        {
            let mut seen = HashSet::new();
            let mut exact_entries = Vec::new();
            for variant in &active_form.variants {
                let (variant_entries, exact_timing) =
                    self.lookup_exact_form_profiled_with_pos(&variant.surface_form, reading, pos);
                merge_timing(&mut timing, exact_timing);
                exact_entries.extend(variant_entries);
            }
            for entry in exact_entries
                .into_iter()
                .chain(discovery_entries.iter().cloned())
                .filter(|entry| {
                    is_substantive(entry)
                        && lookup_state::entry_matches_form(entry, &active_form.display_form)
                })
            {
                if seen.insert(entry.occurrence_id.clone()) {
                    entries.push(entry);
                }
            }
        }
        entries = aggregate::sort_definitions(entries, priority_list);
        timing.entries = entries.len();
        timing.service_ms = started.elapsed().as_millis() as u64;
        lookup_state::build_lookup(
            query,
            observed_form,
            reading,
            pos,
            selected_form_id,
            "contextual",
            forms,
            dictionary_names,
            entries,
            Some(timing),
        )
    }

    pub fn match_kind(&self, headword: &str, reading: Option<&str>) -> Option<String> {
        if headword.is_empty() {
            return None;
        }
        let normalized = normalize_form(headword);
        if self.any_exists(
            "SELECT EXISTS(SELECT 1 FROM entry_keys WHERE kind = 0 AND normalized_value = ?1)",
            &normalized,
        ) || self.any_exists(
            "SELECT EXISTS(SELECT 1 FROM entries WHERE headword = ?1)",
            headword,
        ) || self.alias_exists(headword, &normalized)
            || self.any_exists(
                "SELECT EXISTS(SELECT 1 FROM alias_keys WHERE kind = 0 AND normalized_value = ?1)",
                &normalized,
            )
        {
            return Some("headword".to_string());
        }
        if is_kana_query(headword)
            && (self.any_exists(
                "SELECT EXISTS(SELECT 1 FROM entry_keys WHERE kind = 1 AND normalized_value = ?1)",
                &normalize_reading(headword),
            ) || self.any_exists(
                "SELECT EXISTS(SELECT 1 FROM alias_keys WHERE kind = 1 AND normalized_value = ?1)",
                &normalize_reading(headword),
            ))
        {
            return Some("reading".to_string());
        }
        if let Some(reading) = reading.filter(|value| !value.is_empty() && *value != "*") {
            for normalized_reading in reading_candidates(headword, reading) {
                if self.any_exists(
                    "SELECT EXISTS(SELECT 1 FROM entry_keys WHERE kind = 1 AND normalized_value = ?1)",
                    &normalized_reading,
                ) || self.any_exists(
                    "SELECT EXISTS(SELECT 1 FROM alias_keys WHERE kind = 1 AND normalized_value = ?1)",
                    &normalized_reading,
                ) {
                    return Some("reading".to_string());
                }
            }
        }
        None
    }

    fn any_exists(&self, sql: &str, value: &str) -> bool {
        self.databases.iter().any(|database| {
            database
                .connection
                .query_row(sql, [value], |row| row.get::<_, bool>(0))
                .unwrap_or(false)
        })
    }

    fn alias_exists(&self, value: &str, normalized: &str) -> bool {
        self.databases.iter().any(|database| {
            database
                .connection
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM aliases
                         WHERE alias = ?1 OR normalized_alias = ?2
                     )",
                    [value, normalized],
                    |row| row.get::<_, bool>(0),
                )
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
        let value = self.databases.iter().any(|database| {
            database
                .connection
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM entry_keys WHERE kind = 0 AND normalized_value = ?1
                         UNION ALL
                         SELECT 1 FROM entries WHERE headword = ?2
                         UNION ALL
                         SELECT 1 FROM aliases WHERE alias = ?2 OR normalized_alias = ?1
                         UNION ALL
                         SELECT 1 FROM alias_keys WHERE kind = 0 AND normalized_value = ?1
                     )",
                    [&normalized, word],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap_or(false)
        });
        if let Ok(mut cache) = self.exists_cache.lock() {
            cache.insert(normalized, value);
        }
        value
    }

    pub fn contains_exact_batch(&self, words: &HashSet<String>) -> HashSet<String> {
        const BATCH_SIZE: usize = 2_000;
        let mut matched = HashSet::new();
        let candidates = words
            .iter()
            .map(|word| (word.as_str(), normalize_form(word)))
            .collect::<Vec<_>>();
        for batch in candidates.chunks(BATCH_SIZE) {
            let Ok(payload) = serde_json::to_string(batch) else {
                continue;
            };
            for database in &self.databases {
                let sql = "WITH candidates(word, normalized) AS (\
                               SELECT json_extract(value, '$[0]'), json_extract(value, '$[1]')\
                               FROM json_each(?1)\
                           ) \
                           SELECT DISTINCT candidates.word \
                           FROM candidates JOIN entry_keys k \
                             ON k.kind = 0 AND k.normalized_value = candidates.normalized \
                           UNION \
                           SELECT DISTINCT candidates.word \
                           FROM candidates JOIN entries e ON e.headword = candidates.word \
                           UNION \
                           SELECT DISTINCT candidates.word \
                           FROM candidates JOIN aliases a \
                             ON a.alias = candidates.word OR a.normalized_alias = candidates.normalized \
                           UNION \
                           SELECT DISTINCT candidates.word \
                           FROM candidates JOIN alias_keys k \
                             ON k.kind = 0 AND k.normalized_value = candidates.normalized";
                let Ok(mut statement) = database.connection.prepare(sql) else {
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

    pub fn resolve_exact_forms_batch(
        &self,
        words: &HashSet<String>,
    ) -> HashMap<String, Vec<DictionaryEntryRef>> {
        const BATCH_SIZE: usize = 2_000;
        let mut result: HashMap<String, Vec<DictionaryEntryRef>> = HashMap::new();
        let candidates = words
            .iter()
            .map(|word| (word.as_str(), normalize_form(word)))
            .collect::<Vec<_>>();
        for batch in candidates.chunks(BATCH_SIZE) {
            let Ok(payload) = serde_json::to_string(batch) else {
                continue;
            };
            for database in &self.databases {
                let sql = "WITH candidates(word, normalized) AS (\
                               SELECT json_extract(value, '$[0]'), json_extract(value, '$[1]')\
                               FROM json_each(?1)\
                           ) \
                           SELECT candidates.word, e.id, e.headword, \
                                  COALESCE(k.display_value, k.normalized_value), \
                                  (SELECT COALESCE(r.display_value, r.normalized_value) \
                                   FROM entry_keys r \
                                   WHERE r.entry_id = e.id AND r.kind = 1 \
                                   ORDER BY r.rank LIMIT 1), \
                                  'exact_form' \
                           FROM candidates JOIN entry_keys k \
                             ON k.kind = 0 AND k.normalized_value = candidates.normalized \
                           JOIN entries e ON e.id = k.entry_id \
                           UNION ALL \
                           SELECT candidates.word, e.id, e.headword, e.headword, \
                                  (SELECT COALESCE(r.display_value, r.normalized_value) \
                                   FROM entry_keys r \
                                   WHERE r.entry_id = e.id AND r.kind = 1 \
                                   ORDER BY r.rank LIMIT 1), \
                                  'headword' \
                           FROM candidates JOIN entries e ON e.headword = candidates.word";
                let Ok(mut statement) = database.connection.prepare(sql) else {
                    continue;
                };
                let Ok(rows) = statement.query_map([&payload], |row| {
                    let query: String = row.get(0)?;
                    let entry_id: i64 = row.get(1)?;
                    let reading: Option<String> = row.get(4).ok();
                    Ok((
                        query,
                        DictionaryEntryRef {
                            entry_key: format!("{}\u{1f}{entry_id}", database.name),
                            dict_name: database.name.clone(),
                            headword: row.get(2)?,
                            matched_form: row.get(3)?,
                            match_type: row.get(5)?,
                            readings: reading.into_iter().collect(),
                        },
                    ))
                }) else {
                    continue;
                };
                for (query, reference) in rows.flatten() {
                    let references = result.entry(query).or_default();
                    if !references
                        .iter()
                        .any(|item| item.entry_key == reference.entry_key)
                    {
                        references.push(reference);
                    }
                }
            }
        }
        result
    }

    pub fn lookup(&self, headword: &str, reading: Option<&str>) -> Vec<DictEntry> {
        self.lookup_profiled(headword, reading).0
    }

    /// 保留查询、定义块解压与富内容解析的真实耗时，供悬浮查词诊断使用。
    pub fn lookup_profiled(
        &self,
        headword: &str,
        reading: Option<&str>,
    ) -> (Vec<DictEntry>, DictionaryLookupTiming) {
        self.lookup_profiled_with_pos(headword, reading, None)
    }

    pub fn lookup_profiled_with_pos(
        &self,
        headword: &str,
        reading: Option<&str>,
        pos: Option<&PosTag>,
    ) -> (Vec<DictEntry>, DictionaryLookupTiming) {
        let started = Instant::now();
        let mut timing = DictionaryLookupTiming::default();
        if headword.is_empty() {
            return (Vec::new(), timing);
        }
        let effective_reading = reading
            .filter(|value| !value.is_empty() && *value != "*")
            .or_else(|| is_kana_query(headword).then_some(headword));
        let normalized_reading = effective_reading.map(normalize_reading);
        let mut direct = Vec::new();
        let mut seen = HashSet::new();
        for database_index in 0..self.databases.len() {
            let direct_in_database =
                self.lookup_exact_in_database(database_index, headword, "exact_form", &mut timing);
            let has_content = direct_in_database.iter().any(|entry| {
                !matches!(entry.entry_kind.as_str(), "navigation" | "redirect")
                    && (!entry.senses.is_empty() || !entry.sections.is_empty())
            });
            for entry in direct_in_database.iter().cloned() {
                if seen.insert(entry.occurrence_id.clone()) {
                    direct.push(entry);
                }
            }
            let mut targets =
                self.redirect_targets_in_database(database_index, headword, &mut timing);
            for target in direct_in_database
                .iter()
                .flat_map(|entry| &entry.links)
                .filter(|link| matches!(link.relation.as_str(), "candidate" | "redirect"))
                .map(|link| link.target.clone())
            {
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
            let mut alias_entries = Vec::new();
            for target in targets {
                alias_entries.extend(self.lookup_exact_in_database(
                    database_index,
                    &target,
                    "explicit_alias",
                    &mut timing,
                ));
            }
            let has_compatible_alias = alias_entries.iter().any(|entry| {
                is_substantive(entry)
                    && normalized_reading.as_deref().is_none_or(|requested| {
                        entry.reading.as_deref().map(normalize_reading).as_deref()
                            == Some(requested)
                    })
            });
            for entry in alias_entries {
                if seen.insert(entry.occurrence_id.clone()) {
                    direct.push(entry);
                }
            }
            if has_content || has_compatible_alias {
                continue;
            }
            if let Some(reading) = effective_reading {
                for candidate in reading_candidates(headword, reading) {
                    let reading_entries = self.query_key_in_database(
                        database_index,
                        READING_KEY,
                        &candidate,
                        "reading_fallback",
                        &mut timing,
                    );
                    let found = !reading_entries.is_empty();
                    for entry in reading_entries {
                        if seen.insert(entry.occurrence_id.clone()) {
                            direct.push(entry);
                        }
                    }
                    if found {
                        break;
                    }
                }
            }
        }
        if !direct.is_empty() {
            rank_entries(&mut direct, headword, normalized_reading.as_deref(), pos);
            timing.entries = direct.len();
            timing.service_ms = started.elapsed().as_millis() as u64;
            return (direct, timing);
        }

        if let Some(target) = compatibility_redirect_target(headword) {
            direct = self.lookup_exact(target, &mut timing);
            if !direct.is_empty() {
                for entry in &mut direct {
                    entry.match_type = "compatibility_alias".to_string();
                }
            }
        }
        if direct.is_empty() {
            if let Some(reading) = effective_reading {
                for candidate in reading_candidates(headword, reading) {
                    direct =
                        self.query_key(READING_KEY, &candidate, "reading_fallback", &mut timing);
                    if !direct.is_empty() {
                        break;
                    }
                }
            }
        }
        rank_entries(&mut direct, headword, normalized_reading.as_deref(), pos);
        timing.entries = direct.len();
        timing.service_ms = started.elapsed().as_millis() as u64;
        (direct, timing)
    }

    pub fn lookup_exact_form_profiled_with_pos(
        &self,
        form: &str,
        reading: Option<&str>,
        pos: Option<&PosTag>,
    ) -> (Vec<DictEntry>, DictionaryLookupTiming) {
        let started = Instant::now();
        let mut timing = DictionaryLookupTiming::default();
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        for database_index in 0..self.databases.len() {
            for entry in
                self.lookup_exact_in_database(database_index, form, "exact_form", &mut timing)
            {
                if is_substantive(&entry) && seen.insert(entry.occurrence_id.clone()) {
                    entries.push(entry);
                }
            }
        }
        let normalized_reading = reading
            .filter(|value| !value.is_empty() && *value != "*")
            .map(normalize_reading);
        rank_entries(&mut entries, form, normalized_reading.as_deref(), pos);
        timing.entries = entries.len();
        timing.service_ms = started.elapsed().as_millis() as u64;
        (entries, timing)
    }

    fn complete_form_availability(
        &self,
        seeds: &mut [lookup_state::DictionaryFormSeed],
        discovery_entries: &[DictEntry],
        timing: &mut DictionaryLookupTiming,
    ) {
        let started = Instant::now();
        for seed in seeds {
            for database in &self.databases {
                let mut available = false;
                for variant in &seed.variants {
                    let legacy_key = normalize_form(&variant.surface_form);
                    let indexed = database
                        .connection
                        .query_row(
                            "SELECT EXISTS(
                                 SELECT 1 FROM entry_keys
                                 WHERE kind = 0 AND normalized_value = ?1
                                 UNION ALL
                                 SELECT 1 FROM entries WHERE headword = ?2
                             )",
                            [&legacy_key, &variant.surface_form],
                            |row| row.get::<_, bool>(0),
                        )
                        .unwrap_or(false);
                    if !indexed {
                        continue;
                    }
                    let discovered = discovery_entries
                        .iter()
                        .filter(|entry| {
                            entry.dict_name == database.name
                                && lookup_state::entry_matches_surface_form(
                                    entry,
                                    &variant.surface_form,
                                )
                        })
                        .collect::<Vec<_>>();
                    if !discovered.is_empty()
                        && !discovered.iter().any(|entry| is_substantive(entry))
                    {
                        continue;
                    }
                    available = true;
                    break;
                }
                if available && !seed.available_dictionary_names.contains(&database.name) {
                    seed.available_dictionary_names.push(database.name.clone());
                }
            }
        }
        timing.sqlite_ms += started.elapsed().as_millis() as u64;
    }

    fn lookup_exact(&self, headword: &str, timing: &mut DictionaryLookupTiming) -> Vec<DictEntry> {
        let mut results = self.query_key(FORM_KEY, &normalize_form(headword), "exact_form", timing);
        if results.is_empty() {
            results = self.query_exact_headword(headword, timing);
        }
        results
    }

    fn redirect_targets_in_database(
        &self,
        database_index: usize,
        headword: &str,
        timing: &mut DictionaryLookupTiming,
    ) -> Vec<String> {
        let started = Instant::now();
        let normalized = normalize_form(headword);
        let mut targets = Vec::new();
        let database = &self.databases[database_index];
        if let Ok(mut statement) = database.connection.prepare(
            "SELECT target FROM aliases \
             WHERE alias = ?1 OR normalized_alias = ?2 \
             ORDER BY target",
        ) {
            targets = statement
                .query_map([headword, &normalized], |row| row.get::<_, String>(0))
                .map(|rows| rows.flatten().collect::<Vec<_>>())
                .unwrap_or_default();
        }
        targets.sort();
        targets.dedup();
        timing.redirect_ms += started.elapsed().as_millis() as u64;
        targets
    }

    fn query_key(
        &self,
        kind: i64,
        value: &str,
        match_type: &str,
        timing: &mut DictionaryLookupTiming,
    ) -> Vec<DictEntry> {
        let mut results = Vec::new();
        for database_index in 0..self.databases.len() {
            results.extend(self.query_key_in_database(
                database_index,
                kind,
                value,
                match_type,
                timing,
            ));
        }
        results
    }

    fn query_key_in_database(
        &self,
        database_index: usize,
        kind: i64,
        value: &str,
        match_type: &str,
        timing: &mut DictionaryLookupTiming,
    ) -> Vec<DictEntry> {
        let sql = "SELECT e.id, COALESCE(k.display_value, k.normalized_value), e.headword, \
                          e.definition_block_id, e.definition_offset, e.definition_length, \
                          (SELECT COALESCE(r.display_value, r.normalized_value) \
                           FROM entry_keys r \
                           WHERE r.entry_id = e.id AND r.kind = 1 \
                           ORDER BY r.rank LIMIT 1) \
                   FROM entry_keys k JOIN entries e ON e.id = k.entry_id \
                   WHERE k.kind = ?1 AND k.normalized_value = ?2 \
                   ORDER BY k.rank, e.id LIMIT 10";
        let database = &self.databases[database_index];
        let Ok(mut statement) = database.connection.prepare(sql) else {
            return Vec::new();
        };
        let query_started = Instant::now();
        let Ok(rows) = statement.query_map((kind, value), raw_entry) else {
            return Vec::new();
        };
        let rows = rows.flatten().collect::<Vec<_>>();
        timing.sqlite_ms += query_started.elapsed().as_millis() as u64;
        self.materialize(database_index, rows.into_iter(), match_type, timing)
    }

    fn query_exact_headword(
        &self,
        value: &str,
        timing: &mut DictionaryLookupTiming,
    ) -> Vec<DictEntry> {
        let mut results = Vec::new();
        for database_index in 0..self.databases.len() {
            results.extend(self.query_exact_headword_in_database(
                database_index,
                value,
                "exact_headword",
                timing,
            ));
        }
        results
    }

    fn query_exact_headword_in_database(
        &self,
        database_index: usize,
        value: &str,
        match_type: &str,
        timing: &mut DictionaryLookupTiming,
    ) -> Vec<DictEntry> {
        let sql = "SELECT e.id, COALESCE((\
                          SELECT COALESCE(k.display_value, k.normalized_value) \
                          FROM entry_keys k \
                          WHERE k.entry_id = e.id AND k.kind = 0 \
                          ORDER BY k.rank LIMIT 1\
                      ), e.headword), e.headword, \
                      e.definition_block_id, e.definition_offset, e.definition_length, \
                      (SELECT COALESCE(r.display_value, r.normalized_value) \
                       FROM entry_keys r \
                       WHERE r.entry_id = e.id AND r.kind = 1 \
                       ORDER BY r.rank LIMIT 1) \
                   FROM entries e WHERE e.headword = ?1 ORDER BY e.id LIMIT 10";
        let database = &self.databases[database_index];
        let Ok(mut statement) = database.connection.prepare(sql) else {
            return Vec::new();
        };
        let query_started = Instant::now();
        let Ok(rows) = statement.query_map([value], raw_entry) else {
            return Vec::new();
        };
        let rows = rows.flatten().collect::<Vec<_>>();
        timing.sqlite_ms += query_started.elapsed().as_millis() as u64;
        self.materialize(database_index, rows.into_iter(), match_type, timing)
    }

    fn lookup_exact_in_database(
        &self,
        database_index: usize,
        headword: &str,
        match_type: &str,
        timing: &mut DictionaryLookupTiming,
    ) -> Vec<DictEntry> {
        let mut results = self.query_key_in_database(
            database_index,
            FORM_KEY,
            &normalize_form(headword),
            match_type,
            timing,
        );
        if results.is_empty() {
            results =
                self.query_exact_headword_in_database(database_index, headword, match_type, timing);
        }
        results
    }

    fn materialize(
        &self,
        database_index: usize,
        rows: impl Iterator<Item = RawEntry>,
        match_type: &str,
        timing: &mut DictionaryLookupTiming,
    ) -> Vec<DictEntry> {
        let database = &self.databases[database_index];
        rows.flat_map(|row| {
            let Some(definition) = self.load_definition(database_index, &row, timing) else {
                return Vec::new();
            };
            self.entries(
                database_index,
                database.name.clone(),
                row.entry_id,
                row.headword,
                row.raw_headword,
                definition,
                row.reading,
                match_type,
                timing,
            )
        })
        .collect()
    }

    fn load_definition(
        &self,
        database_index: usize,
        row: &RawEntry,
        timing: &mut DictionaryLookupTiming,
    ) -> Option<String> {
        let started = Instant::now();
        let key = (database_index, row.definition_block_id);
        let cached = self
            .definition_cache
            .lock()
            .ok()
            .and_then(|mut cache| cache.get(key));
        let block = if let Some(block) = cached {
            timing.definition_cache_hits += 1;
            block
        } else {
            timing.definition_cache_misses += 1;
            let database = &self.databases[database_index];
            let (expected_size, compressed): (usize, Vec<u8>) = database
                .connection
                .query_row(
                    "SELECT uncompressed_size, data FROM definition_blocks WHERE id = ?1",
                    [row.definition_block_id],
                    |result| Ok((result.get(0)?, result.get(1)?)),
                )
                .ok()?;
            let mut decoder = ZlibDecoder::new(compressed.as_slice());
            let mut decoded = Vec::with_capacity(expected_size);
            decoder.read_to_end(&mut decoded).ok()?;
            if decoded.len() != expected_size {
                log::warn!("词典 definition block 解压长度不一致");
                return None;
            }
            let decoded = Arc::new(decoded);
            if let Ok(mut cache) = self.definition_cache.lock() {
                cache.insert(key, decoded.clone());
            }
            decoded
        };
        let end = row.definition_offset.checked_add(row.definition_length)?;
        let bytes = block.get(row.definition_offset..end)?;
        let definition = String::from_utf8(bytes.to_vec()).ok();
        timing.definition_ms += started.elapsed().as_millis() as u64;
        definition
    }

    fn entries(
        &self,
        database_index: usize,
        dict_name: String,
        entry_id: i64,
        headword: String,
        raw_headword: String,
        definition: String,
        structured_reading: Option<String>,
        match_type: &str,
        timing: &mut DictionaryLookupTiming,
    ) -> Vec<DictEntry> {
        let started = Instant::now();
        let key = (database_index, entry_id);
        let presentations = self
            .presentation_cache
            .lock()
            .ok()
            .and_then(|mut cache| cache.get(key))
            .unwrap_or_else(|| {
                let value = presentation::present(
                    &dict_name,
                    &headword,
                    &raw_headword,
                    structured_reading.as_deref(),
                    &definition,
                );
                if let Ok(mut cache) = self.presentation_cache.lock() {
                    cache.insert(key, value.clone());
                }
                value
            });
        let source_entry_key = format!("{dict_name}\u{1f}{entry_id}");
        let entries = presentations
            .into_iter()
            .map(|presentation| {
                let occurrence_id = if presentation.occurrence_suffix.is_empty() {
                    source_entry_key.clone()
                } else {
                    format!("{source_entry_key}\u{1f}{}", presentation.occurrence_suffix)
                };
                let display_form = if presentation.header.display_form.is_empty() {
                    headword.clone()
                } else {
                    presentation.header.display_form.clone()
                };
                let reading = presentation
                    .header
                    .reading
                    .clone()
                    .or_else(|| structured_reading.clone());
                DictEntry {
                    entry_key: source_entry_key.clone(),
                    dict_name: dict_name.clone(),
                    headword: display_form,
                    reading,
                    is_preferred: false,
                    definition_html: presentation.definition_html,
                    style_profile: presentation.style_profile,
                    content_blocks: presentation.content_blocks,
                    match_type: match_type.to_string(),
                    links: presentation.links,
                    occurrence_id,
                    source_record_index: presentation.source_record_index,
                    entry_kind: presentation.entry_kind,
                    header: presentation.header,
                    senses: presentation.senses,
                    sections: presentation.sections,
                    adapter_diagnostics: presentation.diagnostics,
                    match_evidence: Some(DictionaryMatchEvidence {
                        kind: match_type.to_string(),
                        query_form: String::new(),
                        matched_form: Some(headword.clone()),
                        requested_reading: None,
                        reading_match: "absent".to_string(),
                        pos_match: "unknown".to_string(),
                        dictionary_local: matches!(match_type, "explicit_alias"),
                        penalties: Vec::new(),
                        score: 0,
                    }),
                    raw_definition: Some(definition.clone()),
                }
            })
            .collect();
        timing.presentation_ms += started.elapsed().as_millis() as u64;
        entries
    }
}

fn raw_entry(row: &Row<'_>) -> rusqlite::Result<RawEntry> {
    Ok(RawEntry {
        entry_id: row.get(0)?,
        headword: row.get(1)?,
        raw_headword: row.get(2)?,
        definition_block_id: row.get(3)?,
        definition_offset: row.get(4)?,
        definition_length: row.get(5)?,
        reading: row.get(6).ok(),
    })
}

fn is_substantive(entry: &DictEntry) -> bool {
    !matches!(entry.entry_kind.as_str(), "navigation" | "redirect")
        && (!entry.senses.is_empty()
            || !entry.sections.is_empty()
            || !entry.content_blocks.is_empty())
}

fn ordered_dictionary_names(available: Vec<String>, priority_list: &[String]) -> Vec<String> {
    let mut ordered = priority_list
        .iter()
        .filter(|name| available.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    for name in available {
        if !ordered.contains(&name) {
            ordered.push(name);
        }
    }
    ordered
}

fn merge_timing(target: &mut DictionaryLookupTiming, source: DictionaryLookupTiming) {
    target.redirect_ms += source.redirect_ms;
    target.sqlite_ms += source.sqlite_ms;
    target.definition_ms += source.definition_ms;
    target.presentation_ms += source.presentation_ms;
    target.definition_cache_hits += source.definition_cache_hits;
    target.definition_cache_misses += source.definition_cache_misses;
}

fn compatibility_redirect_target(headword: &str) -> Option<&'static str> {
    match normalize_form(headword).as_str() {
        "だっせえ" => Some("ダサい"),
        _ => None,
    }
}

fn rank_entries(
    entries: &mut [DictEntry],
    query: &str,
    requested: Option<&str>,
    requested_pos: Option<&PosTag>,
) {
    let normalized_query = normalize_form(query);
    for entry in entries.iter_mut() {
        let mut score = match entry.match_type.as_str() {
            "exact_headword" => 180,
            "exact_form" => 160,
            "explicit_alias" => 120,
            "compatibility_alias" => 105,
            "reading_fallback" => 55,
            "fuzzy" => 10,
            _ => 80,
        };
        let mut penalties = Vec::new();
        if normalize_form(&entry.header.display_form) == normalized_query
            || normalize_form(&entry.headword) == normalized_query
        {
            score += 45;
        }
        let reading_match = match (requested, entry.reading.as_deref()) {
            (Some(requested), Some(reading)) if normalize_reading(reading) == requested => {
                score += 30;
                "exact"
            }
            (Some(_), Some(_)) => {
                score -= 45;
                penalties.push("reading_conflict".to_string());
                "conflict"
            }
            (Some(_), None) => "absent",
            (None, _) => "absent",
        };
        let pos_match = pos_compatibility(requested_pos, entry);
        match pos_match {
            // 词性只作为软证据。上游分词结果与词典底层分类并不总能一一对应，
            // 不能仅凭 POS 冲突把真实 occurrence 压到候选列表之外。
            "exact" => score += 24,
            "compatible" => score += 12,
            "conflict" => {
                score -= 20;
                penalties.push("pos_conflict".to_string());
            }
            _ => {}
        }
        match entry.entry_kind.as_str() {
            "kanji" => {
                score -= 80;
                penalties.push("kanji_entry".to_string());
            }
            "surname" => {
                score -= 65;
                penalties.push("surname".to_string());
            }
            "prefix" | "suffix" | "bound_morpheme" => {
                if !query.starts_with('-') && !query.ends_with('-') {
                    score -= 75;
                    penalties.push("bound_morpheme".to_string());
                }
            }
            "navigation" | "redirect" => {
                score -= 120;
                penalties.push("navigation_only".to_string());
            }
            _ => {}
        }
        if entry.reading.as_deref().is_some_and(|reading| {
            (reading.starts_with('-') || reading.ends_with('-'))
                && !query.starts_with('-')
                && !query.ends_with('-')
        }) {
            score -= 60;
            if !penalties.iter().any(|penalty| penalty == "bound_morpheme") {
                penalties.push("bound_morpheme".to_string());
            }
        }
        entry.is_preferred = false;
        if let Some(evidence) = entry.match_evidence.as_mut() {
            evidence.query_form = query.to_string();
            evidence.requested_reading = requested.map(str::to_string);
            evidence.reading_match = reading_match.to_string();
            evidence.pos_match = pos_match.to_string();
            evidence.penalties = penalties;
            evidence.score = score;
        }
    }
    entries.sort_by(|left, right| {
        let left_score = left.match_evidence.as_ref().map_or(0, |value| value.score);
        let right_score = right.match_evidence.as_ref().map_or(0, |value| value.score);
        right_score
            .cmp(&left_score)
            .then_with(|| left.dict_name.cmp(&right.dict_name))
            .then_with(|| left.source_record_index.cmp(&right.source_record_index))
    });
    let mut dictionary_indices = HashMap::<String, Vec<usize>>::new();
    for (index, entry) in entries.iter().enumerate() {
        dictionary_indices
            .entry(entry.dict_name.clone())
            .or_default()
            .push(index);
    }
    for indices in dictionary_indices.values() {
        let substantive = indices
            .iter()
            .copied()
            .filter(|index| {
                !matches!(
                    entries[*index].entry_kind.as_str(),
                    "navigation" | "redirect"
                )
            })
            .collect::<Vec<_>>();
        let candidates = if substantive.is_empty() {
            indices.as_slice()
        } else {
            substantive.as_slice()
        };
        let Some(best_index) = candidates.first().copied() else {
            continue;
        };
        let best_score = entries[best_index]
            .match_evidence
            .as_ref()
            .map_or(i32::MIN, |evidence| evidence.score);
        let second_score = candidates.get(1).and_then(|index| {
            entries[*index]
                .match_evidence
                .as_ref()
                .map(|evidence| evidence.score)
        });
        let has_clear_margin = second_score.is_none_or(|score| best_score - score >= 16);
        if best_score >= 80 && has_clear_margin {
            entries[best_index].is_preferred = true;
        }
    }
}

fn pos_compatibility(requested: Option<&PosTag>, entry: &DictEntry) -> &'static str {
    let Some(requested) = requested else {
        return "unknown";
    };
    let labels = entry
        .header
        .pos_tags
        .iter()
        .chain(
            entry
                .senses
                .iter()
                .flat_map(dictionary_sense_tags)
                .filter(|tag| tag.kind == "pos"),
        )
        .map(|tag| tag.label.as_str())
        .collect::<Vec<_>>();
    if labels.is_empty() {
        return match entry.entry_kind.as_str() {
            "surname" if requested.major == "名詞" => "compatible",
            "prefix" | "suffix" | "bound_morpheme"
                if requested.sub1.contains("接尾") || requested.sub1.contains("接頭") =>
            {
                "compatible"
            }
            "surname" | "kanji" | "prefix" | "suffix" | "bound_morpheme" => "conflict",
            _ => "unknown",
        };
    }
    if labels.iter().all(|label| label.contains("連語")) {
        return "unknown";
    }
    let markers = match requested.major.as_str() {
        "動詞" => &["動"][..],
        "形容詞" => &["形"][..],
        "副詞" => &["副"][..],
        "連体詞" => &["連体"][..],
        "接続詞" => &["接続", "接"][..],
        "名詞" => &["名", "姓"][..],
        "助詞" => &["助詞"][..],
        "助動詞" => &["助動"][..],
        "感動詞" => &["感"][..],
        _ => &[][..],
    };
    if markers.is_empty() {
        return "unknown";
    }
    if labels
        .iter()
        .any(|label| markers.iter().any(|marker| label.contains(marker)))
    {
        "exact"
    } else {
        "conflict"
    }
}

fn dictionary_sense_tags(
    sense: &crate::models::DictionarySense,
) -> Box<dyn Iterator<Item = &crate::models::DictionaryTag> + '_> {
    Box::new(
        sense
            .tags
            .iter()
            .chain(sense.children.iter().flat_map(dictionary_sense_tags)),
    )
}

fn normalize_reading(value: &str) -> String {
    normalize_form(value)
        .chars()
        .flat_map(|character| {
            if ('\u{3041}'..='\u{3096}').contains(&character) {
                char::from_u32(character as u32 + 0x60)
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                vec![character]
            }
        })
        .collect()
}

fn normalize_form(value: &str) -> String {
    value
        .nfkc()
        .map(|character| match character {
            '繋' => '繫',
            _ => character,
        })
        .filter(|character| {
            !character.is_whitespace()
                && !matches!(
                    character,
                    '・' | '･' | '-' | '‐' | '‑' | '‒' | '–' | '—' | '―'
                )
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
    let mut characters = normalized.chars().collect::<Vec<_>>();
    if let Some(last) = characters.last_mut() {
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
                characters.into_iter().collect()
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
    use crate::dictionary::bundle::{BASE_SCHEMA, SEARCH_SCHEMA};
    use flate2::{write::ZlibEncoder, Compression};
    use rusqlite::{params, Connection};
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn structured_forms_readings_aliases_and_variants_resolve() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("kotoclip-dictionary-{nonce}"));
        std::fs::create_dir_all(&directory).unwrap();
        let database = directory.join("test.sqlite");
        let connection = Connection::open(&database).unwrap();
        connection.execute_batch(BASE_SCHEMA).unwrap();
        connection
            .execute(
                "INSERT INTO metadata VALUES(1, 4, 1, '三省堂Super大辞林3.1', 'test', 7, 2, 7)",
                [],
            )
            .unwrap();
        let entries = [
            (1, "けいさつしょ【警察署】", "<p>警察署释义</p>"),
            (2, "つなぐ【繫ぐ】", "<p>繫ぐ释义</p>"),
            (3, "いる", "<p>☞ <a href=\"entry://いる【入る】\">いる【入る】</a><br>☞ <a href=\"entry://いる【居る】\">いる【居る】</a></p>"),
            (4, "いる【入る】", "<p><span class=\"bss\">いる</span> 入る释义</p>"),
            (5, "こ【子】", "<p><span class=\"bss\">こ</span>【<hy>子</hy>】<br><div><div class=\"no\">①</div><div class=\"lefta\">子供。⇔<a href=\"entry://親\">親</a>・<a href=\"entry://祖\">祖</a>。</div></div></p>"),
            (6, "ダサい", "<p>野暮ったい。</p>"),
            (7, "ださい【駄才】", "<p>才能がないこと。</p>"),
        ];
        for (id, headword, definition) in entries {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
            encoder.write_all(definition.as_bytes()).unwrap();
            let compressed = encoder.finish().unwrap();
            connection
                .execute(
                    "INSERT INTO definition_blocks VALUES(?1, ?2, ?3)",
                    params![id, definition.len(), compressed],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO entries VALUES(?1, ?2, ?1, 0, ?3)",
                    params![id, headword, definition.len()],
                )
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO aliases VALUES('けいさつしょ', NULL, 'けいさつしょ【警察署】')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO aliases VALUES('ダサい', NULL, 'ださい【駄才】')",
                [],
            )
            .unwrap();
        let keys = [
            (1, 0, "警察署", None, 0),
            (1, 1, "ケイサツショ", Some("けいさつしょ"), 0),
            (2, 0, "繫ぐ", None, 0),
            (2, 1, "ツナグ", Some("つなぐ"), 0),
            (3, 0, "いる", None, 0),
            (3, 1, "イル", Some("いる"), 0),
            (4, 0, "入る", None, 0),
            (4, 1, "イル", Some("いる"), 0),
            (5, 0, "子", None, 0),
            (5, 1, "コ", Some("こ"), 0),
            (6, 0, "ダサい", None, 0),
            (6, 1, "ダサイ", Some("ださい"), 0),
            (7, 0, "駄才", None, 0),
            (7, 1, "ダサイ", Some("ださい"), 0),
        ];
        for key in keys {
            connection
                .execute(
                    "INSERT INTO entry_keys VALUES(?1, ?2, ?3, ?4, ?5)",
                    params![key.0, key.1, key.2, key.3, key.4],
                )
                .unwrap();
        }
        connection.execute_batch(SEARCH_SCHEMA).unwrap();
        drop(connection);

        let engine = DictionaryEngine::new(&directory).unwrap();
        let kanji = engine.lookup("警察署", Some("ケイサツショ"));
        assert_eq!(kanji.len(), 1);
        assert_eq!(kanji[0].headword, "警察署");
        assert!(kanji[0].definition_html.contains("警察署释义"));

        let kana = engine.lookup("けいさつしょ", None);
        assert_eq!(kana.len(), 1);
        assert_eq!(kana[0].headword, "警察署");

        let variant = engine.lookup("繋ぐ", Some("ツナガ"));
        assert_eq!(variant.len(), 1);
        assert_eq!(variant[0].headword, "繫ぐ");
        assert!(engine.contains_exact("繋ぐ"));

        let navigation = engine.lookup("いる", None);
        assert!(navigation.iter().any(|entry| entry.links.len() == 2));
        assert!(navigation
            .iter()
            .flat_map(|entry| &entry.links)
            .all(|link| link.relation == "candidate"));

        let kana_definition = engine.lookup("こ", None);
        assert!(kana_definition
            .iter()
            .any(|entry| entry.definition_html.contains("子供")));
        assert!(
            kana_definition
                .iter()
                .flat_map(|entry| &entry.links)
                .all(|link| link.relation == "related"),
            "entries={:?}",
            kana_definition
                .iter()
                .map(|entry| (&entry.headword, &entry.reading, &entry.links))
                .collect::<Vec<_>>()
        );
        assert!(kana_definition
            .iter()
            .flat_map(|entry| &entry.senses)
            .flat_map(|sense| &sense.relations)
            .all(|link| link.relation == "antonym"));

        let colloquial = engine.lookup("だっせえ", None);
        assert_eq!(colloquial.len(), 1);
        assert_eq!(colloquial[0].headword, "ダサい");

        let exact_with_alias = engine.lookup("ダサい", Some("ダサイ"));
        assert!(exact_with_alias
            .iter()
            .any(|entry| entry.headword == "ダサい"));
        assert!(exact_with_alias
            .iter()
            .any(|entry| entry.headword == "駄才"));

        drop(engine);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
