use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use serde::de::{
    DeserializeSeed, Deserializer, Error as DeError, IgnoredAny, MapAccess, SeqAccess, Visitor,
};
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const SNAPSHOT_SCHEMA_VERSION: &str = "kotoclip.quality.snapshot.v1";

const STAGE_ORDER: &[&str] = &[
    "resource",
    "source",
    "preprocessing",
    "morpheme",
    "morphology",
    "word_formation_candidate",
    "word_formation",
    "lexical_candidate",
    "lexical_unit",
    "bunsetsu_boundary",
    "bunsetsu",
    "grammar_candidate",
    "grammar_occurrence",
    "grammar_projection",
    "grammar_residual",
    "personalization",
    "expression_candidate",
    "expression",
    "ui_projection",
];

type Range = (i64, i64);

pub fn run(manifest_path: &Path, output_root: &Path, metadata_path: &Path) -> Result<()> {
    let manifest = read_json(manifest_path)?;
    if manifest.get("schema_version").and_then(Value::as_str) != Some(SNAPSHOT_SCHEMA_VERSION) {
        bail!(
            "快照 schema 不兼容：{}，要求 {SNAPSHOT_SCHEMA_VERSION}",
            manifest.get("schema_version").unwrap_or(&Value::Null)
        );
    }
    fs::create_dir_all(output_root)?;
    let mut spool = EntitySpool::new(output_root)?;
    let mut seen = HashSet::new();
    let mut covered = HashSet::from(["source".to_owned(), "preprocessing".to_owned()]);
    normalize_manifest_entities(&manifest, &mut spool, &mut seen, &mut covered)?;
    normalize_artifacts(
        manifest_path,
        &manifest,
        &mut spool,
        &mut seen,
        &mut covered,
    )?;
    let counts = spool.finish()?;
    let mut covered: Vec<String> = covered.into_iter().collect();
    covered.sort_by_key(|stage| {
        STAGE_ORDER
            .iter()
            .position(|candidate| candidate == stage)
            .unwrap_or(usize::MAX)
    });
    write_json(
        metadata_path,
        &json!({
            "schema_version": "kotoclip.quality.normalize.v1",
            "counts": counts,
            "covered": covered,
        }),
    )
}

pub fn run_candidates(
    before_manifest_path: &Path,
    after_manifest_path: &Path,
    output_path: &Path,
    metadata_path: &Path,
    count_cache: Option<&Path>,
    before_reading_output: Option<&Path>,
    after_reading_output: Option<&Path>,
) -> Result<()> {
    let total_started = Instant::now();
    let manifest_started = Instant::now();
    let before_manifest = read_json(before_manifest_path)?;
    let after_manifest = read_json(after_manifest_path)?;
    let before_artifacts = before_manifest
        .get("artifacts")
        .and_then(Value::as_object)
        .context("before manifest.artifacts 必须是对象")?;
    let after_artifacts = after_manifest
        .get("artifacts")
        .and_then(Value::as_object)
        .context("after manifest.artifacts 必须是对象")?;
    let before_tokens = artifact_path(before_manifest_path, before_artifacts, "tokens")?
        .context("before 快照缺少 tokens")?;
    let after_tokens = artifact_path(after_manifest_path, after_artifacts, "tokens")?
        .context("after 快照缺少 tokens")?;
    let manifest_ms = elapsed_ms(manifest_started.elapsed());
    let mut writer = BufWriter::with_capacity(1024 * 1024, File::create(output_path)?);
    let mut counts_before: BTreeMap<String, u64> = BTreeMap::new();
    let mut counts_after: BTreeMap<String, u64> = BTreeMap::new();
    let mut authoritative_before = HashSet::new();
    let mut authoritative_after = HashSet::new();
    let mut reading_before = CandidateReadingSpool::new(before_reading_output)?;
    let mut reading_after = CandidateReadingSpool::new(after_reading_output)?;
    let mut event_count = 0_u64;

    let mut manifest_before = EntityBatch::default();
    let mut manifest_after = EntityBatch::default();
    let mut manifest_seen = HashSet::new();
    let mut manifest_covered = HashSet::new();
    normalize_manifest_entities(
        &before_manifest,
        &mut manifest_before,
        &mut manifest_seen,
        &mut manifest_covered,
    )?;
    manifest_seen.clear();
    manifest_covered.clear();
    normalize_manifest_entities(
        &after_manifest,
        &mut manifest_after,
        &mut manifest_seen,
        &mut manifest_covered,
    )?;
    add_entity_counts(&mut counts_before, &manifest_before.values);
    add_entity_counts(&mut counts_after, &manifest_after.values);
    event_count +=
        compare_entity_batches(&manifest_before.values, &manifest_after.values, &mut writer)?;

    let scan_started = Instant::now();
    let (before_stream, before_stream_profile) = stream_raw_json_array(before_tokens);
    let (after_stream, after_stream_profile) = stream_raw_json_array(after_tokens);
    let mut before = next_raw_token(&before_stream)?;
    let mut after = next_raw_token(&after_stream)?;
    while before.is_some() || after.is_some() {
        let ordering = compare_fast_token_position(before.as_ref(), after.as_ref());
        match ordering {
            std::cmp::Ordering::Equal => {
                let before_token = before.take().expect("相等时 before 存在");
                let after_token = after.take().expect("相等时 after 存在");
                if before_token.raw.get() == after_token.raw.get() {
                    count_fast_token(
                        &before_token.fast,
                        &mut counts_before,
                        &mut authoritative_before,
                    );
                    count_fast_token(
                        &after_token.fast,
                        &mut counts_after,
                        &mut authoritative_after,
                    );
                    reading_before.push(before_token)?;
                    reading_after.push(after_token)?;
                    before = next_raw_token(&before_stream)?;
                    after = next_raw_token(&after_stream)?;
                    continue;
                }
                let before_value: Value = serde_json::from_str(before_token.raw.get())?;
                let after_value: Value = serde_json::from_str(after_token.raw.get())?;
                let before_entities = token_entities(&before_value, &mut authoritative_before)?;
                let after_entities = token_entities(&after_value, &mut authoritative_after)?;
                add_entity_counts(&mut counts_before, &before_entities);
                add_entity_counts(&mut counts_after, &after_entities);
                event_count +=
                    compare_entity_batches(&before_entities, &after_entities, &mut writer)?;
                let before_range = fast_token_range(&before_token.fast);
                let after_range = fast_token_range(&after_token.fast);
                reading_before.mark_range(before_range);
                reading_before.mark_range(after_range);
                reading_after.mark_range(before_range);
                reading_after.mark_range(after_range);
                reading_before.push(before_token)?;
                reading_after.push(after_token)?;
                before = next_raw_token(&before_stream)?;
                after = next_raw_token(&after_stream)?;
            }
            std::cmp::Ordering::Less => {
                let before_token = before.take().expect("before 游标存在");
                let value: Value = serde_json::from_str(before_token.raw.get())?;
                let entities = token_entities(&value, &mut authoritative_before)?;
                add_entity_counts(&mut counts_before, &entities);
                event_count += write_unmatched_batch("before", &entities, &mut writer)?;
                let range = fast_token_range(&before_token.fast);
                reading_before.mark_range(range);
                reading_after.mark_range(range);
                reading_before.push(before_token)?;
                before = next_raw_token(&before_stream)?;
            }
            std::cmp::Ordering::Greater => {
                let after_token = after.take().expect("after 游标存在");
                let value: Value = serde_json::from_str(after_token.raw.get())?;
                let entities = token_entities(&value, &mut authoritative_after)?;
                add_entity_counts(&mut counts_after, &entities);
                event_count += write_unmatched_batch("after", &entities, &mut writer)?;
                let range = fast_token_range(&after_token.fast);
                reading_before.mark_range(range);
                reading_after.mark_range(range);
                reading_after.push(after_token)?;
                after = next_raw_token(&after_stream)?;
            }
        }
    }
    let candidate_scan_ms = elapsed_ms(scan_started.elapsed());

    let grammar_started = Instant::now();
    let before_occurrences = collect_external_grammar(
        before_manifest_path,
        before_artifacts,
        &authoritative_before,
    )?;
    let after_occurrences = if artifacts_match(before_artifacts, after_artifacts, "tokens")
        && artifacts_match(before_artifacts, after_artifacts, "grammar_occurrences")
    {
        before_occurrences.clone()
    } else {
        collect_external_grammar(after_manifest_path, after_artifacts, &authoritative_after)?
    };
    add_entity_counts(&mut counts_before, &before_occurrences);
    add_entity_counts(&mut counts_after, &after_occurrences);
    event_count += compare_entity_batches(&before_occurrences, &after_occurrences, &mut writer)?;
    let external_grammar_ms = elapsed_ms(grammar_started.elapsed());
    let changed_artifacts = changed_artifact_names(before_artifacts, after_artifacts);
    let supported = changed_artifacts
        .iter()
        .all(|name| name == "tokens" || name == "grammar_occurrences");
    let unchanged_count_started = Instant::now();
    add_unchanged_artifact_counts(
        before_manifest_path,
        before_artifacts,
        after_artifacts,
        &mut counts_before,
        &mut counts_after,
        count_cache,
    )?;
    let unchanged_count_ms = elapsed_ms(unchanged_count_started.elapsed());
    writer.flush()?;

    let unchanged_artifacts: Vec<String> = before_artifacts
        .iter()
        .filter_map(|(name, descriptor)| {
            let before_hash = descriptor.get("sha256").and_then(Value::as_str);
            let after_hash = after_artifacts
                .get(name)
                .and_then(|value| value.get("sha256"))
                .and_then(Value::as_str);
            (before_hash.is_some() && before_hash == after_hash).then(|| name.clone())
        })
        .collect();
    let (reading_headers_before, reading_positions_before) = reading_before.finish()?;
    let (reading_headers_after, reading_positions_after) = reading_after.finish()?;
    write_json(
        metadata_path,
        &json!({
            "schema_version": "kotoclip.quality.candidates.v1",
            "event_count": event_count,
            "partial_counts_before": counts_before,
            "partial_counts_after": counts_after,
            "unchanged_artifacts": unchanged_artifacts,
            "changed_artifacts": changed_artifacts,
            "supported": supported,
            "reading_headers_before": reading_headers_before,
            "reading_headers_after": reading_headers_after,
            "reading_positions_before": reading_positions_before,
            "reading_positions_after": reading_positions_after,
            "timings_ms": {
                "manifest": manifest_ms,
                "candidate_scan": candidate_scan_ms,
                "external_grammar": external_grammar_ms,
                "unchanged_artifact_counts": unchanged_count_ms,
                "before_stream": stream_profile_json(&before_stream_profile),
                "after_stream": stream_profile_json(&after_stream_profile),
                "total": elapsed_ms(total_started.elapsed()),
            },
        }),
    )
}

pub fn run_reading_sentences(
    manifest_path: &Path,
    ranges_path: &Path,
    output_path: &Path,
    metadata_path: &Path,
) -> Result<()> {
    let manifest = read_json(manifest_path)?;
    let artifacts = manifest
        .get("artifacts")
        .and_then(Value::as_object)
        .context("manifest.artifacts 必须是对象")?;
    let tokens_path =
        artifact_path(manifest_path, artifacts, "tokens")?.context("快照缺少 tokens")?;
    let ranges_value = read_json(ranges_path)?;
    let mut ranges: Vec<Range> = ranges_value
        .as_array()
        .context("阅读范围必须是数组")?
        .iter()
        .filter_map(normalized_range)
        .collect();
    ranges.sort_unstable();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut spool = ReadingSentenceSpool::new(output_path, ranges)?;
    for_each_json_array(&tokens_path, |token| spool.push(&token))?;
    let (headers, positions) = spool.finish()?;
    write_json(
        metadata_path,
        &json!({
            "schema_version": "kotoclip.quality.reading-sentences.v1",
            "headers": headers,
            "positions": positions,
        }),
    )
}

#[derive(Deserialize, Default)]
struct FastToken {
    #[serde(default)]
    bunsetsu: Option<FastBunsetsu>,
    #[serde(default)]
    display_class: Option<String>,
    #[serde(default)]
    expressions: Vec<FastExpression>,
}

#[derive(Deserialize, Default)]
struct FastBunsetsu {
    #[serde(default)]
    char_range: Option<[i64; 2]>,
    #[serde(default)]
    surface: String,
    #[serde(default)]
    morphemes: CountedArray,
    #[serde(default)]
    word_formations: CountedArray,
    #[serde(default)]
    lexical_units: CountedArray,
    #[serde(default)]
    morphology: FastMorphology,
    #[serde(default)]
    grammar_occurrences: Vec<FastGrammarOccurrence>,
    #[serde(default)]
    grammar_tags: CountedArray,
    #[serde(default)]
    functional_residuals: CountedArray,
}

#[derive(Deserialize, Default)]
struct FastMorphology {
    #[serde(default)]
    chains: CountedArray,
    #[serde(default)]
    unclassified: CountedArray,
}

#[derive(Deserialize, Default)]
struct FastExpression {
    #[serde(default)]
    status: Option<String>,
}

#[derive(Deserialize, Default)]
struct FastGrammarOccurrence {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    concept_id: Value,
    #[serde(default)]
    rule_id: Value,
    #[serde(default)]
    matched_ranges: Vec<[i64; 2]>,
    #[serde(default)]
    display_ranges: Vec<[i64; 2]>,
    #[serde(default)]
    source_ranges: Vec<[i64; 2]>,
    #[serde(default)]
    char_range: Option<[i64; 2]>,
}

#[derive(Default)]
struct CountedArray(usize);

impl<'de> Deserialize<'de> for CountedArray {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Counter;

        impl<'de> Visitor<'de> for Counter {
            type Value = CountedArray;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("JSON 数组")
            }

            fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut count = 0;
                while sequence.next_element::<IgnoredAny>()?.is_some() {
                    count += 1;
                }
                Ok(CountedArray(count))
            }
        }

        deserializer.deserialize_seq(Counter)
    }
}

struct RawToken {
    raw: Box<RawValue>,
    fast: FastToken,
}

struct CandidateReadingSpool {
    output: Option<BufWriter<File>>,
    headers: Vec<Value>,
    positions: Vec<Value>,
    selected_ranges: Vec<Range>,
    pending_tokens: Vec<(Range, Box<RawValue>)>,
    current: String,
    start: Option<i64>,
    end: i64,
    terminal_pending: bool,
    written: u64,
}

impl CandidateReadingSpool {
    fn new(output_path: Option<&Path>) -> Result<Self> {
        let output = output_path
            .map(|path| -> Result<BufWriter<File>> {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                Ok(BufWriter::with_capacity(1024 * 1024, File::create(path)?))
            })
            .transpose()?;
        Ok(Self {
            output,
            headers: Vec::new(),
            positions: Vec::new(),
            selected_ranges: Vec::new(),
            pending_tokens: Vec::new(),
            current: String::new(),
            start: None,
            end: 0,
            terminal_pending: false,
            written: 0,
        })
    }

    fn mark_range(&mut self, range: Range) {
        if range != (i64::MAX, i64::MAX) {
            self.selected_ranges.push(range);
        }
    }

    fn push(&mut self, token: RawToken) -> Result<()> {
        let Some(bunsetsu) = token.fast.bunsetsu.as_ref() else {
            return Ok(());
        };
        let Some([range_start, range_end]) = bunsetsu.char_range else {
            return Ok(());
        };
        let surface = bunsetsu.surface.clone();
        if self.output.is_some() {
            self.pending_tokens
                .push(((range_start, range_end), token.raw));
        }
        for (index, character) in surface.chars().enumerate() {
            if self.terminal_pending
                && !matches!(character, '。' | '！' | '？' | '!' | '?')
                && !matches!(
                    character,
                    '」' | '』' | '）' | '】' | '》' | '〉' | '〕' | '］' | '”' | '’'
                )
            {
                self.emit()?;
            }
            let offset = range_start + index as i64;
            self.start.get_or_insert(offset);
            self.current.push(character);
            self.end = offset + 1;
            if character == '\n' || matches!(character, '。' | '！' | '？' | '!' | '?') {
                self.terminal_pending = true;
            }
        }
        Ok(())
    }

    fn emit(&mut self) -> Result<()> {
        let Some(start) = self.start.take() else {
            return Ok(());
        };
        if self.current.is_empty() {
            return Ok(());
        }
        let sentence_range = (start, self.end);
        let index = self.headers.len();
        let digest = format!("{:x}", Sha256::digest(self.current.as_bytes()));
        self.headers.push(json!({
            "index": index,
            "char_range": [start, self.end],
            "text_sha256": digest,
        }));
        let selected = self
            .selected_ranges
            .iter()
            .any(|range| ranges_intersect(*range, sentence_range));
        if selected {
            // 只写阅读协议所需字段，避免 Python 再解析完整原始 token。
            let mut line = format!(
                "{{\"char_range\":[{start},{}],\"index\":{index},\"text\":{},\"text_sha256\":{},\"tokens\":[",
                self.end,
                serde_json::to_string(&self.current)?,
                serde_json::to_string(&digest)?,
            )
            .into_bytes();
            let mut first_token = true;
            for (_, raw) in self
                .pending_tokens
                .iter()
                .filter(|(range, _)| ranges_intersect(*range, sentence_range))
            {
                if !first_token {
                    line.push(b',');
                }
                let token: Value = serde_json::from_str(raw.get())?;
                serde_json::to_writer(&mut line, &reading_token(&token)?)?;
                first_token = false;
            }
            line.extend_from_slice(b"]}");
            line.push(b'\n');
            self.positions.push(json!({
                "index": index,
                "offset": self.written,
                "bytes": line.len(),
            }));
            if let Some(output) = self.output.as_mut() {
                output.write_all(&line)?;
            }
            self.written += line.len() as u64;
        }
        self.pending_tokens.retain(|(range, _)| range.1 > self.end);
        self.selected_ranges.retain(|range| range.1 > self.end);
        self.current.clear();
        self.terminal_pending = false;
        Ok(())
    }

    fn finish(mut self) -> Result<(Vec<Value>, Vec<Value>)> {
        self.emit()?;
        if let Some(output) = self.output.as_mut() {
            output.flush()?;
        }
        Ok((self.headers, self.positions))
    }
}

#[derive(Default)]
struct StreamProfile {
    tokens: u64,
    fast_token_parse: Duration,
    total: Duration,
}

fn reading_token(token: &Value) -> Result<Value> {
    let bunsetsu = token
        .get("bunsetsu")
        .and_then(Value::as_object)
        .context("阅读 token 缺少 bunsetsu")?;
    let char_range = bunsetsu.get("char_range").cloned().unwrap_or(Value::Null);
    let morphemes = array_or_empty(bunsetsu.get("morphemes"));
    let formations = array_or_empty(bunsetsu.get("word_formations"));
    let lexical_units = array_or_empty(bunsetsu.get("lexical_units"));
    let lookup_request = if let Some(lexical) = lexical_units.first().and_then(Value::as_object) {
        object([
            (
                "word",
                lexical
                    .get("base_form")
                    .cloned()
                    .unwrap_or_else(|| json!("")),
            ),
            (
                "observed_form",
                lexical
                    .get("base_form")
                    .cloned()
                    .unwrap_or_else(|| json!("")),
            ),
            (
                "reading",
                lexical.get("reading").cloned().unwrap_or(Value::Null),
            ),
            (
                "pos",
                lexical.get("output_pos").cloned().unwrap_or(Value::Null),
            ),
        ])
    } else if let Some(index) = formations
        .first()
        .and_then(Value::as_object)
        .and_then(|value| value.get("head_morpheme"))
        .and_then(Value::as_i64)
        .filter(|index| *index >= 0)
        .map(|index| index as usize)
        .filter(|index| *index < morphemes.len())
    {
        reading_lookup_request(morphemes[index].as_object())
    } else {
        reading_lookup_request(bunsetsu.get("head_word").and_then(Value::as_object))
    };
    Ok(object([
        (
            "surface",
            bunsetsu
                .get("surface")
                .cloned()
                .unwrap_or_else(|| json!("")),
        ),
        ("char_range", char_range),
        (
            "head_word",
            bunsetsu.get("head_word").cloned().unwrap_or(Value::Null),
        ),
        ("morphemes", Value::Array(morphemes)),
        (
            "morphology",
            bunsetsu
                .get("morphology")
                .cloned()
                .unwrap_or_else(|| json!({"chains": []})),
        ),
        ("word_formations", Value::Array(formations)),
        ("lexical_units", Value::Array(lexical_units)),
        (
            "grammar_occurrences",
            bunsetsu
                .get("grammar_occurrences")
                .cloned()
                .unwrap_or_else(|| json!([])),
        ),
        (
            "grammar_tags",
            bunsetsu
                .get("grammar_tags")
                .cloned()
                .unwrap_or_else(|| json!([])),
        ),
        (
            "functional_residuals",
            bunsetsu
                .get("functional_residuals")
                .cloned()
                .unwrap_or_else(|| json!([])),
        ),
        (
            "function",
            bunsetsu.get("function").cloned().unwrap_or(Value::Null),
        ),
        (
            "expressions",
            token
                .get("expressions")
                .cloned()
                .unwrap_or_else(|| json!([])),
        ),
        (
            "display_class",
            token
                .get("display_class")
                .cloned()
                .unwrap_or_else(|| json!("content")),
        ),
        ("lookup_request", lookup_request),
    ]))
}

fn array_or_empty(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn reading_lookup_request(word: Option<&Map<String, Value>>) -> Value {
    let empty = Map::new();
    let word = word.unwrap_or(&empty);
    let pos = word
        .get("pos")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let major = pos.get("major").and_then(Value::as_str).unwrap_or_default();
    let sub1 = pos.get("sub1").and_then(Value::as_str).unwrap_or_default();
    let surface = word
        .get("surface")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let base = word
        .get("base_form")
        .and_then(Value::as_str)
        .unwrap_or(surface);
    let prefer_surface = matches!(major, "助詞" | "助動詞") || (major == "動詞" && sub1 == "接尾");
    let selected = if prefer_surface || base == "*" {
        surface
    } else {
        base
    };
    object([
        ("word", json!(selected)),
        ("observed_form", json!(selected)),
        (
            "reading",
            word.get("reading").cloned().unwrap_or(Value::Null),
        ),
        ("pos", Value::Object(pos)),
    ])
}

struct ReadingSentenceSpool {
    output: BufWriter<File>,
    headers: Vec<Value>,
    positions: Vec<Value>,
    relevant_ranges: Vec<Range>,
    relevant_cursor: usize,
    pending_tokens: Vec<(Range, Value)>,
    current: String,
    start: Option<i64>,
    end: i64,
    terminal_pending: bool,
    written: u64,
}

impl ReadingSentenceSpool {
    fn new(output_path: &Path, relevant_ranges: Vec<Range>) -> Result<Self> {
        Ok(Self {
            output: BufWriter::with_capacity(1024 * 1024, File::create(output_path)?),
            headers: Vec::new(),
            positions: Vec::new(),
            relevant_ranges,
            relevant_cursor: 0,
            pending_tokens: Vec::new(),
            current: String::new(),
            start: None,
            end: 0,
            terminal_pending: false,
            written: 0,
        })
    }

    fn push(&mut self, token: &Value) -> Result<()> {
        let Some(bunsetsu) = token.get("bunsetsu").and_then(Value::as_object) else {
            return Ok(());
        };
        let Some(token_range) = bunsetsu.get("char_range").and_then(normalized_range) else {
            return Ok(());
        };
        if let Some(record) = reading_token_record(token)? {
            self.pending_tokens.push((token_range, record));
        }
        let surface = bunsetsu.get("surface").map(py_string).unwrap_or_default();
        for (index, character) in surface.chars().enumerate() {
            if self.terminal_pending
                && !matches!(character, '。' | '！' | '？' | '!' | '?')
                && !matches!(
                    character,
                    '」' | '』' | '）' | '】' | '》' | '〉' | '〕' | '］' | '”' | '’'
                )
            {
                self.emit()?;
            }
            let offset = token_range.0 + index as i64;
            self.start.get_or_insert(offset);
            self.current.push(character);
            self.end = offset + 1;
            if character == '\n' || matches!(character, '。' | '！' | '？' | '!' | '?') {
                self.terminal_pending = true;
            }
        }
        Ok(())
    }

    fn emit(&mut self) -> Result<()> {
        let Some(start) = self.start.take() else {
            return Ok(());
        };
        if self.current.is_empty() {
            return Ok(());
        }
        let sentence_range = (start, self.end);
        let index = self.headers.len();
        let digest = format!("{:x}", Sha256::digest(self.current.as_bytes()));
        let header = json!({
            "index": index,
            "char_range": [start, self.end],
            "text_sha256": digest,
        });
        self.headers.push(header.clone());
        while self.relevant_cursor < self.relevant_ranges.len()
            && self.relevant_ranges[self.relevant_cursor].1 < sentence_range.0
        {
            self.relevant_cursor += 1;
        }
        let mut selected = false;
        let mut probe = self.relevant_cursor;
        while probe < self.relevant_ranges.len()
            && self.relevant_ranges[probe].0 <= sentence_range.1
        {
            if ranges_intersect(sentence_range, self.relevant_ranges[probe]) {
                selected = true;
                break;
            }
            probe += 1;
        }
        if selected {
            let tokens: Vec<Value> = self
                .pending_tokens
                .iter()
                .filter(|(range, _)| ranges_intersect(*range, sentence_range))
                .map(|(_, record)| record.clone())
                .collect();
            let text = tokens
                .iter()
                .filter_map(|token| token.get("surface"))
                .map(py_string)
                .collect::<String>();
            let sentence = json!({
                "index": index,
                "char_range": [start, self.end],
                "text_sha256": header["text_sha256"].clone(),
                "tokens": tokens,
                "text": text,
            });
            let mut line = canonical_json(&sentence)?.into_bytes();
            line.push(b'\n');
            self.positions.push(json!({
                "index": index,
                "offset": self.written,
                "bytes": line.len(),
            }));
            self.output.write_all(&line)?;
            self.written += line.len() as u64;
        }
        self.pending_tokens.retain(|(range, _)| range.1 > self.end);
        self.current.clear();
        self.terminal_pending = false;
        Ok(())
    }

    fn finish(mut self) -> Result<(Vec<Value>, Vec<Value>)> {
        self.emit()?;
        self.output.flush()?;
        Ok((self.headers, self.positions))
    }
}

fn ranges_intersect(left: Range, right: Range) -> bool {
    left.0 < right.1 && right.0 < left.1
}

fn reading_token_record(token: &Value) -> Result<Option<Value>> {
    let Some(bunsetsu) = token.get("bunsetsu").and_then(Value::as_object) else {
        return Ok(None);
    };
    let Some(char_range) = bunsetsu.get("char_range").and_then(normalized_range) else {
        return Ok(None);
    };
    let morphemes = bunsetsu
        .get("morphemes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let formations = bunsetsu
        .get("word_formations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let lexical = bunsetsu
        .get("lexical_units")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_object);
    let query = if let Some(lexical) = lexical {
        let word = lexical
            .get("base_form")
            .cloned()
            .unwrap_or(Value::String(String::new()));
        json!({
            "word": word.clone(),
            "observed_form": lexical.get("base_form").cloned().unwrap_or(Value::String(String::new())),
            "reading": lexical.get("reading").cloned().unwrap_or(Value::Null),
            "pos": lexical.get("output_pos").cloned().unwrap_or(Value::Null),
        })
    } else {
        let formation_head = formations
            .first()
            .and_then(Value::as_object)
            .and_then(|formation| formation.get("head_morpheme"))
            .and_then(Value::as_u64)
            .and_then(|index| morphemes.get(index as usize));
        let head = formation_head
            .or_else(|| bunsetsu.get("head_word"))
            .and_then(Value::as_object);
        let pos = head
            .and_then(|value| value.get("pos"))
            .and_then(Value::as_object);
        let major = pos
            .and_then(|value| value.get("major"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let sub1 = pos
            .and_then(|value| value.get("sub1"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let surface = head
            .and_then(|value| value.get("surface"))
            .cloned()
            .unwrap_or(Value::String(String::new()));
        let base = head
            .and_then(|value| value.get("base_form"))
            .filter(|value| !matches!(value.as_str(), None | Some("*")))
            .cloned()
            .unwrap_or_else(|| surface.clone());
        let word = if matches!(major, "助詞" | "助動詞") || (major == "動詞" && sub1 == "接尾")
        {
            surface
        } else {
            base
        };
        json!({
            "word": word.clone(),
            "observed_form": word,
            "reading": head.and_then(|value| value.get("reading")).cloned().unwrap_or(Value::Null),
            "pos": pos.cloned().map(Value::Object).unwrap_or(Value::Null),
        })
    };
    let request_digest = format!("{:x}", Sha256::digest(canonical_json(&query)?.as_bytes()));
    Ok(Some(json!({
        "surface": bunsetsu.get("surface").cloned().unwrap_or(Value::String(String::new())),
        "char_range": [char_range.0, char_range.1],
        "head_word": bunsetsu.get("head_word").cloned().unwrap_or(Value::Null),
        "morphemes": bunsetsu.get("morphemes").cloned().unwrap_or_else(|| json!([])),
        "morphology": bunsetsu.get("morphology").cloned().unwrap_or_else(|| json!({"chains": []})),
        "word_formations": bunsetsu.get("word_formations").cloned().unwrap_or_else(|| json!([])),
        "lexical_units": bunsetsu.get("lexical_units").cloned().unwrap_or_else(|| json!([])),
        "grammar_occurrences": bunsetsu.get("grammar_occurrences").cloned().unwrap_or_else(|| json!([])),
        "grammar_tags": bunsetsu.get("grammar_tags").cloned().unwrap_or_else(|| json!([])),
        "functional_residuals": bunsetsu.get("functional_residuals").cloned().unwrap_or_else(|| json!([])),
        "function": bunsetsu.get("function").cloned().unwrap_or(Value::Null),
        "expressions": token.get("expressions").cloned().unwrap_or_else(|| json!([])),
        "display_class": token.get("display_class").cloned().unwrap_or(Value::String("content".to_owned())),
        "lookup_request_id": &request_digest[..20],
        "lookup_request": query,
    })))
}

fn changed_artifact_names(before: &Map<String, Value>, after: &Map<String, Value>) -> Vec<String> {
    let mut names: Vec<String> = before.keys().chain(after.keys()).cloned().collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .filter(|name| {
            before
                .get(name)
                .and_then(|value| value.get("sha256"))
                .and_then(Value::as_str)
                != after
                    .get(name)
                    .and_then(|value| value.get("sha256"))
                    .and_then(Value::as_str)
        })
        .collect()
}

fn add_unchanged_artifact_counts(
    manifest_path: &Path,
    before: &Map<String, Value>,
    after: &Map<String, Value>,
    counts_before: &mut BTreeMap<String, u64>,
    counts_after: &mut BTreeMap<String, u64>,
    count_cache: Option<&Path>,
) -> Result<()> {
    let same = |name: &str| {
        before
            .get(name)
            .and_then(|value| value.get("sha256"))
            .and_then(Value::as_str)
            == after
                .get(name)
                .and_then(|value| value.get("sha256"))
                .and_then(Value::as_str)
    };
    if same("word_formations") {
        let counts = if let Some(counts) = artifact_stage_counts(before, "word_formations") {
            counts
        } else if let Some(path) = artifact_path(manifest_path, before, "word_formations")? {
            cached_artifact_counts(before, "word_formations", count_cache, || {
                let counts = count_named_object_arrays(&path, &["items", "rejected"])?;
                Ok(BTreeMap::from([(
                    "word_formation_candidate".to_owned(),
                    counts.values().sum::<usize>() as u64,
                )]))
            })?
        } else {
            BTreeMap::new()
        };
        if !counts.is_empty() {
            add_same_count(
                counts_before,
                counts_after,
                "word_formation_candidate",
                counts.get("word_formation_candidate").copied().unwrap_or(0) as usize,
            );
        }
    }
    if same("lexical_candidates") {
        let counts = if let Some(counts) = artifact_stage_counts(before, "lexical_candidates") {
            counts
        } else if let Some(path) = artifact_path(manifest_path, before, "lexical_candidates")? {
            cached_artifact_counts(before, "lexical_candidates", count_cache, || {
                let counts = count_named_object_arrays(&path, &["items"])?;
                Ok(BTreeMap::from([(
                    "lexical_candidate".to_owned(),
                    counts.values().sum::<usize>() as u64,
                )]))
            })?
        } else {
            BTreeMap::new()
        };
        if !counts.is_empty() {
            add_same_count(
                counts_before,
                counts_after,
                "lexical_candidate",
                counts.get("lexical_candidate").copied().unwrap_or(0) as usize,
            );
        }
    }
    if same("bunsetsu") {
        let counts = if let Some(counts) = artifact_stage_counts(before, "bunsetsu") {
            counts
        } else if let Some(path) = artifact_path(manifest_path, before, "bunsetsu")? {
            cached_artifact_counts(before, "bunsetsu", count_cache, || {
                Ok(BTreeMap::from([(
                    "bunsetsu_boundary".to_owned(),
                    count_report_boundaries(&path)? as u64,
                )]))
            })?
        } else {
            BTreeMap::new()
        };
        if !counts.is_empty() {
            add_same_count(
                counts_before,
                counts_after,
                "bunsetsu_boundary",
                counts.get("bunsetsu_boundary").copied().unwrap_or(0) as usize,
            );
        }
    }
    if same("expressions") {
        let counts = if let Some(counts) = artifact_stage_counts(before, "expressions") {
            counts
        } else if let Some(path) = artifact_path(manifest_path, before, "expressions")? {
            cached_artifact_counts(before, "expressions", count_cache, || {
                let mut accepted = 0_u64;
                let mut rejected = 0_u64;
                for_each_json_array(&path, |expression| {
                    if expression
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("accepted")
                        == "accepted"
                    {
                        accepted += 1;
                    } else {
                        rejected += 1;
                    }
                    Ok(())
                })?;
                Ok(BTreeMap::from([
                    ("expression".to_owned(), accepted),
                    ("expression_candidate".to_owned(), rejected),
                ]))
            })?
        } else {
            BTreeMap::new()
        };
        if !counts.is_empty() {
            let accepted = counts.get("expression").copied().unwrap_or(0) as usize;
            let rejected = counts.get("expression_candidate").copied().unwrap_or(0) as usize;
            add_same_count(counts_before, counts_after, "expression", accepted);
            add_same_count(
                counts_before,
                counts_after,
                "expression_candidate",
                rejected,
            );
        }
    }
    if same("catalogs") {
        let counts = if let Some(counts) = artifact_stage_counts(before, "catalogs") {
            counts
        } else if let Some(path) = artifact_path(manifest_path, before, "catalogs")? {
            cached_artifact_counts(before, "catalogs", count_cache, || {
                let mut count = 0_u64;
                for_each_json_array(&path, |_| {
                    count += 1;
                    Ok(())
                })?;
                Ok(BTreeMap::from([("resource".to_owned(), count)]))
            })?
        } else {
            BTreeMap::new()
        };
        if !counts.is_empty() {
            let count = counts.get("resource").copied().unwrap_or(0) as usize;
            add_same_count(counts_before, counts_after, "resource", count);
        }
    }
    if same("ui_projection") {
        let counts = if let Some(counts) = artifact_stage_counts(before, "ui_projection") {
            counts
        } else if let Some(path) = artifact_path(manifest_path, before, "ui_projection")? {
            cached_artifact_counts(before, "ui_projection", count_cache, || {
                let report = read_json(&path)?;
                let mut batch = EntityBatch::default();
                let mut seen = HashSet::new();
                append_ui_projection_entities(&mut batch, &mut seen, &report)?;
                Ok(BTreeMap::from([(
                    "ui_projection".to_owned(),
                    batch.values.len() as u64,
                )]))
            })?
        } else {
            BTreeMap::new()
        };
        if !counts.is_empty() {
            add_same_count(
                counts_before,
                counts_after,
                "ui_projection",
                counts.get("ui_projection").copied().unwrap_or(0) as usize,
            );
        }
    }
    Ok(())
}

fn artifact_stage_counts(
    artifacts: &Map<String, Value>,
    name: &str,
) -> Option<BTreeMap<String, u64>> {
    let stage_counts = artifacts.get(name)?.get("stage_counts")?.as_object()?;
    let counts: BTreeMap<String, u64> = stage_counts
        .iter()
        .filter_map(|(stage, count)| count.as_u64().map(|count| (stage.clone(), count)))
        .collect();
    (!counts.is_empty()).then_some(counts)
}

fn cached_artifact_counts<F>(
    artifacts: &Map<String, Value>,
    name: &str,
    cache_root: Option<&Path>,
    compute: F,
) -> Result<BTreeMap<String, u64>>
where
    F: FnOnce() -> Result<BTreeMap<String, u64>>,
{
    let Some(cache_root) = cache_root else {
        return compute();
    };
    let Some(digest) = artifacts
        .get(name)
        .and_then(|value| value.get("sha256"))
        .and_then(Value::as_str)
    else {
        return compute();
    };
    let cache_path = cache_root.join(format!("{name}-{digest}.json"));
    if cache_path.is_file() {
        return serde_json::from_reader(BufReader::new(File::open(&cache_path)?))
            .with_context(|| format!("无法读取 artifact 计数缓存 {}", cache_path.display()));
    }
    let counts = compute()?;
    fs::create_dir_all(cache_root)?;
    let temporary = cache_path.with_extension(format!("json.{}.tmp", std::process::id()));
    write_json(&temporary, &json!(counts))?;
    if !cache_path.exists() {
        fs::rename(&temporary, &cache_path)?;
    }
    Ok(counts)
}

fn count_named_object_arrays(path: &Path, fields: &[&str]) -> Result<HashMap<String, usize>> {
    let file = File::open(path).with_context(|| format!("无法读取 {}", path.display()))?;
    let reader: Box<dyn Read> = if path.extension().is_some_and(|value| value == "gz") {
        Box::new(GzDecoder::new(BufReader::with_capacity(1024 * 1024, file)))
    } else {
        Box::new(BufReader::with_capacity(1024 * 1024, file))
    };
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let wanted = fields.iter().map(|value| (*value).to_owned()).collect();
    ObjectArrayCountSeed { wanted }
        .deserialize(&mut deserializer)
        .map_err(Into::into)
}

struct ObjectArrayCountSeed {
    wanted: HashSet<String>,
}

impl<'de> DeserializeSeed<'de> for ObjectArrayCountSeed {
    type Value = HashMap<String, usize>;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ObjectArrayCountVisitor {
            wanted: self.wanted,
        })
    }
}

struct ObjectArrayCountVisitor {
    wanted: HashSet<String>,
}

impl<'de> Visitor<'de> for ObjectArrayCountVisitor {
    type Value = HashMap<String, usize>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("包含目标数组的 JSON 对象")
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut counts = HashMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if self.wanted.contains(&key) {
                counts.insert(key, map.next_value_seed(ArrayLengthSeed)?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(counts)
    }
}

struct ArrayLengthSeed;

impl<'de> DeserializeSeed<'de> for ArrayLengthSeed {
    type Value = usize;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(ArrayLengthVisitor)
    }
}

struct ArrayLengthVisitor;

impl<'de> Visitor<'de> for ArrayLengthVisitor {
    type Value = usize;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON 数组")
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut count = 0;
        while sequence.next_element::<IgnoredAny>()?.is_some() {
            count += 1;
        }
        Ok(count)
    }
}

fn count_report_boundaries(path: &Path) -> Result<usize> {
    let file = File::open(path).with_context(|| format!("无法读取 {}", path.display()))?;
    let reader: Box<dyn Read> = if path.extension().is_some_and(|value| value == "gz") {
        Box::new(GzDecoder::new(BufReader::with_capacity(1024 * 1024, file)))
    } else {
        Box::new(BufReader::with_capacity(1024 * 1024, file))
    };
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    ReportBoundarySeed
        .deserialize(&mut deserializer)
        .map_err(Into::into)
}

struct ReportBoundarySeed;

impl<'de> DeserializeSeed<'de> for ReportBoundarySeed {
    type Value = usize;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(ReportBoundaryArrayVisitor)
    }
}

struct ReportBoundaryArrayVisitor;

impl<'de> Visitor<'de> for ReportBoundaryArrayVisitor {
    type Value = usize;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bunsetsu 报告数组")
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut count = 0;
        while let Some(boundaries) = sequence.next_element_seed(ReportBoundaryItemSeed)? {
            count += 1 + boundaries;
        }
        Ok(count)
    }
}

struct ReportBoundaryItemSeed;

impl<'de> DeserializeSeed<'de> for ReportBoundaryItemSeed {
    type Value = usize;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ReportBoundaryItemVisitor)
    }
}

struct ReportBoundaryItemVisitor;

impl<'de> Visitor<'de> for ReportBoundaryItemVisitor {
    type Value = usize;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bunsetsu 报告对象")
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut boundaries = 0;
        while let Some(key) = map.next_key::<String>()? {
            if key == "boundaries" {
                boundaries = map.next_value_seed(ArrayLengthSeed)?;
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(boundaries)
    }
}

fn add_same_count(
    before: &mut BTreeMap<String, u64>,
    after: &mut BTreeMap<String, u64>,
    stage: &str,
    count: usize,
) {
    add_count(before, stage, count);
    add_count(after, stage, count);
}

#[derive(Default)]
struct EntityBatch {
    values: Vec<Value>,
}

impl EntityTarget for EntityBatch {
    fn append(&mut self, seen: &mut HashSet<[u8; 16]>, entity: Value) -> Result<()> {
        let stage = entity
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let key = entity
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let value = entity.get("value").unwrap_or(&Value::Null);
        let mut digest = Sha256::new();
        for part in [stage.to_owned(), key.to_owned(), canonical_json(value)?] {
            digest.update(part.as_bytes());
            digest.update([0]);
        }
        let signature: [u8; 16] = digest.finalize()[..16]
            .try_into()
            .expect("固定 SHA-256 长度");
        if seen.insert(signature) {
            self.values.push(entity);
        }
        Ok(())
    }
}

fn token_entities(
    token: &Value,
    authoritative: &mut HashSet<(String, String)>,
) -> Result<Vec<Value>> {
    let mut sink = EntityBatch::default();
    let mut ignored_seen = HashSet::new();
    let is_content = token
        .get("display_class")
        .and_then(Value::as_str)
        .unwrap_or("content")
        == "content";
    if is_content {
        if let Some(bunsetsu) = token.get("bunsetsu").and_then(Value::as_object) {
            add_bunsetsu_entities(
                &mut sink,
                &mut ignored_seen,
                bunsetsu,
                "tokens",
                Some(authoritative),
            )?;
        }
    }
    if let Some(expressions) = token.get("expressions").and_then(Value::as_array) {
        for expression in expressions.iter().filter_map(Value::as_object) {
            append_expression_entity(&mut sink, &mut ignored_seen, expression, "tokens")?;
        }
    }
    if is_content {
        let bunsetsu = token
            .get("bunsetsu")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let mut personalization = Map::new();
        for key in [
            "novelty_score",
            "is_selected",
            "is_known",
            "inference_reason",
            "display_class",
        ] {
            if let Some(value) = token.get(key) {
                personalization.insert(key.to_owned(), value.clone());
            }
        }
        let char_range = bunsetsu.get("char_range").cloned().unwrap_or(Value::Null);
        personalization.insert("char_range".to_owned(), char_range.clone());
        append_entity(
            &mut sink,
            &mut ignored_seen,
            snapshot_entity(
                "personalization",
                "token_personalization",
                Value::Object(personalization),
                "tokens",
                &[json!(range_key(&char_range))],
                &[],
                Some(normalized_ranges(&Value::Object(bunsetsu))),
            ),
        )?;
    }
    Ok(sink.values)
}

fn collect_external_grammar(
    manifest_path: &Path,
    artifacts: &Map<String, Value>,
    authoritative: &HashSet<(String, String)>,
) -> Result<Vec<Value>> {
    let Some(path) = artifact_path(manifest_path, artifacts, "grammar_occurrences")? else {
        return Ok(Vec::new());
    };
    let mut sink = EntityBatch::default();
    let mut ignored_seen = HashSet::new();
    for_each_json_array(&path, |occurrence| {
        if let Some(occurrence) = occurrence.as_object() {
            append_grammar_occurrence(
                &mut sink,
                &mut ignored_seen,
                occurrence,
                "grammar_occurrences",
                Some(authoritative),
                None,
            )?;
        }
        Ok(())
    })?;
    Ok(sink.values)
}

fn compare_entity_batches(
    before: &[Value],
    after: &[Value],
    writer: &mut impl Write,
) -> Result<u64> {
    let mut before_groups = group_entities(before)?;
    let mut after_groups = group_entities(after)?;
    let mut keys: Vec<String> = before_groups
        .keys()
        .chain(after_groups.keys())
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    let mut count = 0;
    for key in keys {
        let mut left = before_groups.remove(&key).unwrap_or_default();
        let mut right = after_groups.remove(&key).unwrap_or_default();
        left.sort_by_cached_key(|entity| {
            canonical_json(entity.get("value").unwrap_or(&Value::Null)).unwrap_or_default()
        });
        right.sort_by_cached_key(|entity| {
            canonical_json(entity.get("value").unwrap_or(&Value::Null)).unwrap_or_default()
        });
        let paired = left.len().min(right.len());
        for index in 0..paired {
            if left[index].get("value") != right[index].get("value") {
                write_candidate(writer, "pair", &left[index], Some(&right[index]))?;
                count += 1;
            }
        }
        for entity in &left[paired..] {
            write_candidate(writer, "before", entity, None)?;
            count += 1;
        }
        for entity in &right[paired..] {
            write_candidate(writer, "after", entity, None)?;
            count += 1;
        }
    }
    Ok(count)
}

fn group_entities(values: &[Value]) -> Result<HashMap<String, Vec<Value>>> {
    let mut groups: HashMap<String, Vec<Value>> = HashMap::new();
    for value in values {
        let key = value
            .get("key")
            .and_then(Value::as_str)
            .context("实体缺少 key")?;
        let stage = value
            .get("stage")
            .and_then(Value::as_str)
            .context("实体缺少 stage")?;
        groups
            .entry(format!("{stage}\0{key}"))
            .or_default()
            .push(value.clone());
    }
    Ok(groups)
}

fn write_unmatched_batch(side: &str, values: &[Value], writer: &mut impl Write) -> Result<u64> {
    for value in values {
        write_candidate(writer, side, value, None)?;
    }
    Ok(values.len() as u64)
}

fn write_candidate(
    writer: &mut impl Write,
    kind: &str,
    first: &Value,
    second: Option<&Value>,
) -> Result<()> {
    let value = if kind == "pair" {
        json!({"kind": kind, "before": first, "after": second})
    } else {
        json!({"kind": kind, "entity": first})
    };
    serde_json::to_writer(&mut *writer, &value)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn add_entity_counts(counts: &mut BTreeMap<String, u64>, values: &[Value]) {
    for value in values {
        if let Some(stage) = value.get("stage").and_then(Value::as_str) {
            *counts.entry(stage.to_owned()).or_default() += 1;
        }
    }
}

fn count_fast_token(
    token: &FastToken,
    counts: &mut BTreeMap<String, u64>,
    authoritative: &mut HashSet<(String, String)>,
) {
    let is_content = token.display_class.as_deref().unwrap_or("content") == "content";
    if is_content {
        if let Some(bunsetsu) = token.bunsetsu.as_ref() {
            add_count(counts, "morpheme", bunsetsu.morphemes.0);
            add_count(counts, "word_formation", bunsetsu.word_formations.0);
            add_count(counts, "lexical_unit", bunsetsu.lexical_units.0);
            add_count(counts, "bunsetsu", 1);
            add_count(
                counts,
                "morphology",
                bunsetsu.morphology.chains.0 + bunsetsu.morphology.unclassified.0,
            );
            for occurrence in &bunsetsu.grammar_occurrences {
                let (stage, key) = fast_grammar_identity(occurrence);
                add_count(counts, stage, 1);
                authoritative.insert((stage.to_owned(), key));
            }
            add_count(counts, "grammar_projection", bunsetsu.grammar_tags.0);
            add_count(counts, "grammar_residual", bunsetsu.functional_residuals.0);
            add_count(counts, "personalization", 1);
        }
    }
    for expression in &token.expressions {
        let stage = if expression.status.as_deref().unwrap_or("accepted") == "accepted" {
            "expression"
        } else {
            "expression_candidate"
        };
        add_count(counts, stage, 1);
    }
}

fn fast_grammar_identity(occurrence: &FastGrammarOccurrence) -> (&'static str, String) {
    let stage = if occurrence.status.as_deref().unwrap_or("accepted") == "accepted" {
        "grammar_occurrence"
    } else {
        "grammar_candidate"
    };
    let ranges: Vec<Range> = if !occurrence.matched_ranges.is_empty() {
        occurrence
            .matched_ranges
            .iter()
            .map(|range| (range[0], range[1]))
            .collect()
    } else if !occurrence.display_ranges.is_empty() {
        occurrence
            .display_ranges
            .iter()
            .map(|range| (range[0], range[1]))
            .collect()
    } else if !occurrence.source_ranges.is_empty() {
        occurrence
            .source_ranges
            .iter()
            .map(|range| (range[0], range[1]))
            .collect()
    } else {
        occurrence
            .char_range
            .map(|range| vec![(range[0], range[1])])
            .unwrap_or_default()
    };
    let key = entity_key(
        "grammar_occurrence",
        &ranges,
        &[occurrence.concept_id.clone(), occurrence.rule_id.clone()],
    );
    (stage, key)
}

fn add_count(counts: &mut BTreeMap<String, u64>, stage: &str, count: usize) {
    *counts.entry(stage.to_owned()).or_default() += count as u64;
}

fn stream_raw_json_array(
    path: PathBuf,
) -> (
    Receiver<std::result::Result<RawToken, String>>,
    Arc<Mutex<StreamProfile>>,
) {
    let (sender, receiver) = sync_channel(32);
    let profile = Arc::new(Mutex::new(StreamProfile::default()));
    let thread_profile = Arc::clone(&profile);
    thread::spawn(move || {
        let started = Instant::now();
        let mut tokens = 0_u64;
        let mut fast_token_parse = Duration::ZERO;
        let result = for_each_raw_json_array(&path, |value| {
            let fast_started = Instant::now();
            let fast = serde_json::from_str(value.get()).context("无法解析轻量 token")?;
            fast_token_parse += fast_started.elapsed();
            tokens += 1;
            sender
                .send(Ok(RawToken { raw: value, fast }))
                .map_err(|_| anyhow::anyhow!("候选比较接收端已关闭"))
        });
        if let Ok(mut profile) = thread_profile.lock() {
            profile.tokens = tokens;
            profile.fast_token_parse = fast_token_parse;
            profile.total = started.elapsed();
        }
        if let Err(error) = result {
            let _ = sender.send(Err(format!("{error:#}")));
        }
    });
    (receiver, profile)
}

fn artifacts_match(before: &Map<String, Value>, after: &Map<String, Value>, name: &str) -> bool {
    let before_hash = before
        .get(name)
        .and_then(|value| value.get("sha256"))
        .and_then(Value::as_str);
    let after_hash = after
        .get(name)
        .and_then(|value| value.get("sha256"))
        .and_then(Value::as_str);
    before_hash.is_some() && before_hash == after_hash
}

fn elapsed_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn stream_profile_json(profile: &Arc<Mutex<StreamProfile>>) -> Value {
    let profile = profile.lock().expect("stream profile 锁未中毒");
    json!({
        "tokens": profile.tokens,
        "fast_token_parse": elapsed_ms(profile.fast_token_parse),
        "total": elapsed_ms(profile.total),
        "other": elapsed_ms(profile.total.saturating_sub(profile.fast_token_parse)),
    })
}

fn next_raw_token(
    receiver: &Receiver<std::result::Result<RawToken, String>>,
) -> Result<Option<RawToken>> {
    match receiver.recv() {
        Ok(Ok(token)) => Ok(Some(token)),
        Ok(Err(error)) => bail!(error),
        Err(_) => Ok(None),
    }
}

fn compare_fast_token_position(
    before: Option<&RawToken>,
    after: Option<&RawToken>,
) -> std::cmp::Ordering {
    match (before, after) {
        (None, None) => std::cmp::Ordering::Equal,
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(before), Some(after)) => {
            fast_token_range(&before.fast).cmp(&fast_token_range(&after.fast))
        }
    }
}

fn fast_token_range(token: &FastToken) -> Range {
    token
        .bunsetsu
        .as_ref()
        .and_then(|bunsetsu| bunsetsu.char_range)
        .map(|range| (range[0], range[1]))
        .unwrap_or((i64::MAX, i64::MAX))
}

fn normalize_manifest_entities(
    manifest: &Value,
    spool: &mut impl EntityTarget,
    seen: &mut HashSet<[u8; 16]>,
    covered: &mut HashSet<String>,
) -> Result<()> {
    let corpus = manifest.get("corpus").and_then(Value::as_object);
    let corpus_value = |key: &str| {
        corpus
            .and_then(|value| value.get(key))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let corpus_id = corpus_value("id");
    let selected_characters = as_i64_or_zero(&corpus_value("selected_characters"));
    append_entity(
        spool,
        seen,
        snapshot_entity(
            "source",
            "selected_source",
            object([
                ("corpus_id", corpus_value("id")),
                ("selected_sha256", corpus_value("selected_sha256")),
                ("selected_bytes", corpus_value("selected_bytes")),
                ("selected_characters", corpus_value("selected_characters")),
            ]),
            "manifest",
            &[default_if_empty(&corpus_id, json!("corpus"))],
            &[],
            Some(vec![(0, selected_characters)]),
        ),
    )?;
    let analysis_characters = as_i64_or_zero(&corpus_value("analysis_characters"));
    append_entity(
        spool,
        seen,
        snapshot_entity(
            "preprocessing",
            "prepared_text",
            object([
                ("analysis_text_sha256", corpus_value("analysis_text_sha256")),
                ("analysis_characters", corpus_value("analysis_characters")),
            ]),
            "manifest",
            &[default_if_empty(&corpus_id, json!("corpus"))],
            &[],
            Some(vec![(0, analysis_characters)]),
        ),
    )?;

    let resources = manifest.get("resources").and_then(Value::as_object);
    for kind in ["cli", "system_dictionary", "profile"] {
        if let Some(raw) = resources
            .and_then(|value| value.get(kind))
            .and_then(Value::as_object)
        {
            append_resource(spool, seen, kind, raw)?;
            covered.insert("resource".to_owned());
        }
    }
    for (plural, singular) in [
        ("dictionary_sources", "dictionary_source"),
        ("dictionary_caches", "dictionary_cache"),
        ("catalogs", "catalog"),
    ] {
        for raw in resources
            .and_then(|value| value.get(plural))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
        {
            append_resource(spool, seen, singular, raw)?;
            covered.insert("resource".to_owned());
        }
    }
    Ok(())
}

fn append_resource(
    spool: &mut impl EntityTarget,
    seen: &mut HashSet<[u8; 16]>,
    kind: &str,
    raw: &Map<String, Value>,
) -> Result<()> {
    let name = raw
        .get("logical_name")
        .or_else(|| raw.get("path"))
        .map(py_string)
        .unwrap_or_else(|| "unknown".to_owned())
        .replace('\\', "/");
    let value = object([
        ("resource_kind", json!(kind)),
        ("name", json!(name)),
        ("bytes", raw.get("bytes").cloned().unwrap_or(Value::Null)),
        ("sha256", raw.get("sha256").cloned().unwrap_or(Value::Null)),
        ("affects_stages", json!(resource_affects(kind, &name))),
    ]);
    append_entity(
        spool,
        seen,
        snapshot_entity(
            "resource",
            "pipeline_resource",
            value,
            "manifest",
            &[json!(kind), json!(name)],
            &[],
            Some(Vec::new()),
        ),
    )
}

fn normalize_artifacts(
    manifest_path: &Path,
    manifest: &Value,
    spool: &mut EntitySpool,
    seen: &mut HashSet<[u8; 16]>,
    covered: &mut HashSet<String>,
) -> Result<()> {
    let artifacts = manifest
        .get("artifacts")
        .and_then(Value::as_object)
        .context("manifest.artifacts 必须是对象")?;
    for name in artifacts.keys() {
        for stage in artifact_stage_coverage(name) {
            covered.insert((*stage).to_owned());
        }
    }
    let has_tokens = artifacts.get("tokens").is_some_and(Value::is_object);
    let mut authoritative_grammar_keys = HashSet::new();
    if let Some(path) = artifact_path(manifest_path, artifacts, "tokens")? {
        for_each_json_array(&path, |token| {
            let is_content = token
                .get("display_class")
                .and_then(Value::as_str)
                .unwrap_or("content")
                == "content";
            if is_content {
                if let Some(bunsetsu) = token.get("bunsetsu").and_then(Value::as_object) {
                    add_bunsetsu_entities(
                        spool,
                        seen,
                        bunsetsu,
                        "tokens",
                        Some(&mut authoritative_grammar_keys),
                    )?;
                }
            }
            if let Some(expressions) = token.get("expressions").and_then(Value::as_array) {
                for expression in expressions.iter().filter_map(Value::as_object) {
                    append_expression_entity(spool, seen, expression, "tokens")?;
                }
            }
            if is_content {
                let bunsetsu = token
                    .get("bunsetsu")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let mut personalization = Map::new();
                for key in [
                    "novelty_score",
                    "is_selected",
                    "is_known",
                    "inference_reason",
                    "display_class",
                ] {
                    if let Some(value) = token.get(key) {
                        personalization.insert(key.to_owned(), value.clone());
                    }
                }
                let char_range = bunsetsu.get("char_range").cloned().unwrap_or(Value::Null);
                personalization.insert("char_range".to_owned(), char_range.clone());
                append_entity(
                    spool,
                    seen,
                    snapshot_entity(
                        "personalization",
                        "token_personalization",
                        Value::Object(personalization),
                        "tokens",
                        &[json!(range_key(&char_range))],
                        &[],
                        Some(normalized_ranges(&Value::Object(bunsetsu))),
                    ),
                )?;
            }
            Ok(())
        })?;
    }

    if let Some(path) = artifact_path(manifest_path, artifacts, "bunsetsu")? {
        let mut report_index = 0_i64;
        for_each_json_array(&path, |report| {
            let bunsetsus = report
                .get("bunsetsus")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let flat_morphemes: Vec<&Value> = bunsetsus
                .iter()
                .filter_map(Value::as_object)
                .flat_map(|bunsetsu| {
                    bunsetsu
                        .get("morphemes")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                })
                .collect();
            let report_ranges: Vec<Range> = bunsetsus
                .iter()
                .filter_map(|bunsetsu| {
                    normalized_range(bunsetsu.get("char_range").unwrap_or(&Value::Null))
                })
                .collect();
            let report_span = span_of_ranges(&report_ranges);
            if !has_tokens {
                for bunsetsu in bunsetsus.iter().filter_map(Value::as_object) {
                    add_bunsetsu_entities(spool, seen, bunsetsu, "bunsetsu", None)?;
                }
            }
            for boundary in report
                .get("boundaries")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_object)
            {
                let morpheme_index = boundary
                    .get("morpheme_index")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let point =
                    if morpheme_index > 0 && (morpheme_index as usize) <= flat_morphemes.len() {
                        normalized_range(
                            flat_morphemes[morpheme_index as usize - 1]
                                .get("char_range")
                                .unwrap_or(&Value::Null),
                        )
                        .map(|value| value.1)
                    } else {
                        None
                    };
                let mut value = boundary.clone();
                if let Some(point) = point {
                    value.insert("_quality_range".to_owned(), json!([point, point]));
                }
                append_entity(
                    spool,
                    seen,
                    snapshot_entity(
                        "bunsetsu_boundary",
                        "bunsetsu_boundary",
                        Value::Object(value),
                        "bunsetsu",
                        &[
                            json!(report_index),
                            point.map_or(Value::Null, Value::from),
                            json!(morpheme_index),
                        ],
                        &point.map(|value| vec![json!(value)]).unwrap_or_default(),
                        Some(point.map(|value| vec![(value, value)]).unwrap_or_default()),
                    ),
                )?;
            }
            let integrity = object([
                ("report_index", json!(report_index)),
                (
                    "char_range",
                    report_span
                        .map(|value| json!([value.0, value.1]))
                        .unwrap_or(Value::Null),
                ),
                (
                    "unresolved_boundaries",
                    report
                        .get("unresolved_boundaries")
                        .cloned()
                        .unwrap_or(Value::Null),
                ),
                (
                    "reconstruction_ok",
                    report
                        .get("reconstruction_ok")
                        .cloned()
                        .unwrap_or(Value::Null),
                ),
                (
                    "range_integrity_ok",
                    report
                        .get("range_integrity_ok")
                        .cloned()
                        .unwrap_or(Value::Null),
                ),
            ]);
            append_entity(
                spool,
                seen,
                snapshot_entity(
                    "bunsetsu_boundary",
                    "segment_integrity",
                    integrity,
                    "bunsetsu",
                    &[json!(report_index)],
                    &[],
                    Some(report_span.into_iter().collect()),
                ),
            )?;
            report_index += 1;
            Ok(())
        })?;
    }

    if let Some(report) = load_artifact(manifest_path, artifacts, "word_formations")? {
        for item in report
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
        {
            let mut formation = item
                .get("formation")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            formation.insert("status".to_owned(), json!("accepted"));
            formation.insert(
                "morpheme_signature".to_owned(),
                item.get("morpheme_signature").cloned().unwrap_or(json!([])),
            );
            formation.insert(
                "output_pos".to_owned(),
                item.get("output_pos").cloned().unwrap_or(Value::Null),
            );
            let rule_id = formation.get("rule_id").cloned().unwrap_or(Value::Null);
            let surface = formation.get("surface").cloned().unwrap_or(Value::Null);
            append_entity(
                spool,
                seen,
                snapshot_entity(
                    "word_formation_candidate",
                    "word_formation_candidate",
                    Value::Object(formation),
                    "word_formations",
                    std::slice::from_ref(&rule_id),
                    &[rule_id.clone(), surface],
                    None,
                ),
            )?;
        }
        for item in report
            .get("rejected")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
        {
            let mut value = item.clone();
            value.insert("status".to_owned(), json!("rejected"));
            let rule_id = value.get("rule_id").cloned().unwrap_or(Value::Null);
            append_entity(
                spool,
                seen,
                snapshot_entity(
                    "word_formation_candidate",
                    "word_formation_candidate",
                    Value::Object(value),
                    "word_formations",
                    std::slice::from_ref(&rule_id),
                    std::slice::from_ref(&rule_id),
                    None,
                ),
            )?;
        }
    }

    if let Some(report) = load_artifact(manifest_path, artifacts, "lexical_candidates")? {
        for item in report
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
        {
            let mut candidate = item
                .get("candidate")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            candidate.insert(
                "morpheme_signature".to_owned(),
                item.get("morpheme_signature").cloned().unwrap_or(json!([])),
            );
            let query = candidate.get("query").cloned().unwrap_or(Value::Null);
            let shape = candidate
                .get("lexical_shape")
                .cloned()
                .unwrap_or(Value::Null);
            let surface = candidate.get("surface").cloned().unwrap_or(Value::Null);
            append_entity(
                spool,
                seen,
                snapshot_entity(
                    "lexical_candidate",
                    "lexical_candidate",
                    Value::Object(candidate),
                    "lexical_candidates",
                    &[query.clone(), shape],
                    &[surface, query],
                    None,
                ),
            )?;
        }
    }

    if let Some(path) = artifact_path(manifest_path, artifacts, "grammar_occurrences")? {
        for_each_json_array(&path, |occurrence| {
            if let Some(occurrence) = occurrence.as_object() {
                append_grammar_occurrence(
                    spool,
                    seen,
                    occurrence,
                    "grammar_occurrences",
                    Some(&authoritative_grammar_keys),
                    None,
                )?;
            }
            Ok(())
        })?;
    }
    if let Some(report) = load_artifact(manifest_path, artifacts, "grammar_residuals")? {
        for residual in report
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
        {
            append_functional_residual(spool, seen, residual, "grammar_residuals")?;
        }
    }
    if let Some(path) = artifact_path(manifest_path, artifacts, "expressions")? {
        for_each_json_array(&path, |expression| {
            if let Some(expression) = expression.as_object() {
                append_expression_entity(spool, seen, expression, "expressions")?;
            }
            Ok(())
        })?;
    }
    if let Some(path) = artifact_path(manifest_path, artifacts, "catalogs")? {
        let mut ordinal = 0_i64;
        for_each_json_array(&path, |catalog| {
            if let Some(catalog) = catalog.as_object() {
                let name = catalog
                    .get("layer")
                    .or_else(|| catalog.get("name"))
                    .map(py_string)
                    .unwrap_or_else(|| ordinal.to_string());
                let value = object([
                    ("resource_kind", json!("catalog_audit")),
                    ("name", json!(name)),
                    ("catalog", Value::Object(catalog.clone())),
                    (
                        "affects_stages",
                        json!([
                            "morphology",
                            "grammar_candidate",
                            "grammar_occurrence",
                            "grammar_projection",
                            "grammar_residual"
                        ]),
                    ),
                ]);
                append_entity(
                    spool,
                    seen,
                    snapshot_entity(
                        "resource",
                        "catalog_audit",
                        value,
                        "catalogs",
                        &[json!(name)],
                        &[],
                        Some(Vec::new()),
                    ),
                )?;
            }
            ordinal += 1;
            Ok(())
        })?;
    }
    if let Some(report) = load_artifact(manifest_path, artifacts, "ui_projection")? {
        append_ui_projection_entities(spool, seen, &report)?;
    }
    Ok(())
}

fn add_bunsetsu_entities(
    spool: &mut impl EntityTarget,
    seen: &mut HashSet<[u8; 16]>,
    bunsetsu: &Map<String, Value>,
    artifact: &str,
    mut authoritative: Option<&mut HashSet<(String, String)>>,
) -> Result<()> {
    for morpheme in array_objects(bunsetsu.get("morphemes")) {
        let char_range = morpheme.get("char_range").cloned().unwrap_or(Value::Null);
        append_entity(
            spool,
            seen,
            snapshot_entity(
                "morpheme",
                "morpheme",
                Value::Object(morpheme.clone()),
                artifact,
                &[json!(range_key(&char_range))],
                &[json!(range_key(&char_range))],
                Some(normalized_ranges(&Value::Object(morpheme.clone()))),
            ),
        )?;
    }
    for formation in array_objects(bunsetsu.get("word_formations")) {
        append_simple_bunsetsu_entity(
            spool,
            seen,
            "word_formation",
            "accepted_word_formation",
            formation,
            artifact,
            &["rule_id"],
            &["rule_id", "surface"],
        )?;
    }
    for lexical in array_objects(bunsetsu.get("lexical_units")) {
        append_simple_bunsetsu_entity(
            spool,
            seen,
            "lexical_unit",
            "accepted_lexical_unit",
            lexical,
            artifact,
            &["surface", "base_form"],
            &["surface"],
        )?;
    }
    let excluded = HashSet::from([
        "morphemes",
        "word_formations",
        "lexical_units",
        "morphology",
        "grammar_occurrences",
        "grammar_tags",
        "functional_residuals",
    ]);
    let stripped: Map<String, Value> = bunsetsu
        .iter()
        .filter(|(key, _)| !excluded.contains(key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let char_range = bunsetsu.get("char_range").cloned().unwrap_or(Value::Null);
    append_entity(
        spool,
        seen,
        snapshot_entity(
            "bunsetsu",
            "bunsetsu",
            Value::Object(stripped),
            artifact,
            &[json!(range_key(&char_range))],
            &[json!(range_key(&char_range))],
            None,
        ),
    )?;
    if let Some(morphology) = bunsetsu.get("morphology").and_then(Value::as_object) {
        for chain in array_objects(morphology.get("chains")) {
            append_simple_bunsetsu_entity(
                spool,
                seen,
                "morphology",
                "morphology_chain",
                chain,
                artifact,
                &["role", "base_lexeme"],
                &["role", "base_lexeme"],
            )?;
        }
        for unclassified in morphology
            .get("unclassified")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let ranges = normalized_ranges(unclassified);
            let value = if let Some(first) = ranges.first() {
                json!({"char_range": [first.0, first.1]})
            } else {
                json!({"char_range": unclassified})
            };
            append_entity(
                spool,
                seen,
                snapshot_entity(
                    "morphology",
                    "unclassified_morphology",
                    value,
                    artifact,
                    &[json!(range_key(unclassified))],
                    &[],
                    Some(ranges),
                ),
            )?;
        }
    }
    for occurrence in array_objects(bunsetsu.get("grammar_occurrences")) {
        append_grammar_occurrence(
            spool,
            seen,
            occurrence,
            artifact,
            None,
            authoritative.as_deref_mut(),
        )?;
    }
    for tag in array_objects(bunsetsu.get("grammar_tags")) {
        append_simple_bunsetsu_entity(
            spool,
            seen,
            "grammar_projection",
            "grammar_tag",
            tag,
            artifact,
            &["concept_id", "pattern_id"],
            &["concept_id"],
        )?;
    }
    for residual in array_objects(bunsetsu.get("functional_residuals")) {
        append_functional_residual(spool, seen, residual, artifact)?;
    }
    Ok(())
}

fn append_simple_bunsetsu_entity(
    spool: &mut impl EntityTarget,
    seen: &mut HashSet<[u8; 16]>,
    stage: &str,
    kind: &str,
    value: &Map<String, Value>,
    artifact: &str,
    key_fields: &[&str],
    anchor_fields: &[&str],
) -> Result<()> {
    let parts = |fields: &[&str]| {
        fields
            .iter()
            .map(|field| value.get(*field).cloned().unwrap_or(Value::Null))
            .collect::<Vec<_>>()
    };
    append_entity(
        spool,
        seen,
        snapshot_entity(
            stage,
            kind,
            Value::Object(value.clone()),
            artifact,
            &parts(key_fields),
            &parts(anchor_fields),
            None,
        ),
    )
}

fn append_grammar_occurrence(
    spool: &mut impl EntityTarget,
    seen: &mut HashSet<[u8; 16]>,
    occurrence: &Map<String, Value>,
    artifact: &str,
    authoritative: Option<&HashSet<(String, String)>>,
    record_authoritative: Option<&mut HashSet<(String, String)>>,
) -> Result<()> {
    let status = occurrence
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("accepted");
    let stage = if status == "accepted" {
        "grammar_occurrence"
    } else {
        "grammar_candidate"
    };
    let entity = snapshot_entity(
        stage,
        "grammar_occurrence",
        Value::Object(occurrence.clone()),
        artifact,
        &[
            occurrence.get("concept_id").cloned().unwrap_or(Value::Null),
            occurrence.get("rule_id").cloned().unwrap_or(Value::Null),
        ],
        &[occurrence.get("concept_id").cloned().unwrap_or(Value::Null)],
        None,
    );
    let key = entity
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if authoritative.is_some_and(|keys| keys.contains(&(stage.to_owned(), key.clone()))) {
        return Ok(());
    }
    append_entity(spool, seen, entity)?;
    if let Some(keys) = record_authoritative {
        keys.insert((stage.to_owned(), key));
    }
    Ok(())
}

fn append_functional_residual(
    spool: &mut impl EntityTarget,
    seen: &mut HashSet<[u8; 16]>,
    residual: &Map<String, Value>,
    artifact: &str,
) -> Result<()> {
    append_simple_bunsetsu_entity(
        spool,
        seen,
        "grammar_residual",
        "functional_residual",
        residual,
        artifact,
        &["surface", "base_form"],
        &["surface"],
    )
}

fn append_expression_entity(
    spool: &mut impl EntityTarget,
    seen: &mut HashSet<[u8; 16]>,
    expression: &Map<String, Value>,
    artifact: &str,
) -> Result<()> {
    let mut value = expression.clone();
    value
        .entry("status".to_owned())
        .or_insert_with(|| json!("accepted"));
    let stage = if value.get("status").and_then(Value::as_str) == Some("accepted") {
        "expression"
    } else {
        "expression_candidate"
    };
    let label = value
        .get("label")
        .filter(|value| !is_empty(value))
        .or_else(|| value.get("rule_id"))
        .cloned()
        .unwrap_or(Value::Null);
    append_entity(
        spool,
        seen,
        snapshot_entity(
            stage,
            "expression",
            Value::Object(value.clone()),
            artifact,
            &[
                value.get("origin").cloned().unwrap_or(Value::Null),
                value.get("rule_id").cloned().unwrap_or(Value::Null),
                label.clone(),
            ],
            &[label, value.get("surface").cloned().unwrap_or(Value::Null)],
            None,
        ),
    )
}

fn append_ui_projection_entities(
    spool: &mut impl EntityTarget,
    seen: &mut HashSet<[u8; 16]>,
    report: &Value,
) -> Result<()> {
    let items = if let Some(items) = report.as_array() {
        items
    } else {
        report
            .get("items")
            .and_then(Value::as_array)
            .context("ui_projection 产物必须是数组或包含 items 数组")?
    };
    let id_fields = [
        "projection_id",
        "projectionId",
        "target_id",
        "targetId",
        "occurrence_id",
        "occurrenceId",
        "match_id",
        "matchId",
        "token_id",
        "tokenId",
        "id",
    ];
    for (ordinal, item) in items.iter().enumerate() {
        let item = item
            .as_object()
            .with_context(|| format!("ui_projection.items[{ordinal}] 必须是对象"))?;
        let projection_id = id_fields
            .iter()
            .find_map(|field| item.get(*field).filter(|value| !is_empty(value)))
            .cloned()
            .unwrap_or_else(|| json!(ordinal));
        let kind = item
            .get("kind")
            .or_else(|| item.get("type"))
            .map(py_string)
            .unwrap_or_else(|| "projection".to_owned());
        append_entity(
            spool,
            seen,
            snapshot_entity(
                "ui_projection",
                &kind,
                Value::Object(item.clone()),
                "ui_projection",
                std::slice::from_ref(&projection_id),
                &[projection_id.clone(), json!(kind)],
                None,
            ),
        )?;
    }
    Ok(())
}

fn snapshot_entity(
    stage: &str,
    kind: &str,
    value: Value,
    artifact: &str,
    key_parts: &[Value],
    anchor_parts: &[Value],
    ranges: Option<Vec<Range>>,
) -> Value {
    let ranges = ranges.unwrap_or_else(|| normalized_ranges(&value));
    let anchor_parts = if anchor_parts.is_empty() {
        key_parts
    } else {
        anchor_parts
    };
    object([
        ("stage", json!(stage)),
        ("kind", json!(kind)),
        ("key", json!(entity_key(kind, &ranges, key_parts))),
        ("anchor", json!(entity_key(kind, &ranges, anchor_parts))),
        (
            "ranges",
            json!(ranges
                .iter()
                .map(|value| [value.0, value.1])
                .collect::<Vec<_>>()),
        ),
        ("context", json!(context_for(&value))),
        ("artifact", json!(artifact)),
        ("value", value),
    ])
}

fn append_entity(
    spool: &mut impl EntityTarget,
    seen: &mut HashSet<[u8; 16]>,
    entity: Value,
) -> Result<()> {
    spool.append(seen, entity)
}

trait EntityTarget {
    fn append(&mut self, seen: &mut HashSet<[u8; 16]>, entity: Value) -> Result<()>;
}

impl EntityTarget for EntitySpool {
    fn append(&mut self, seen: &mut HashSet<[u8; 16]>, entity: Value) -> Result<()> {
        let stage = entity
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let key = entity
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let value = entity.get("value").unwrap_or(&Value::Null);
        let mut digest = Sha256::new();
        for part in [stage.to_owned(), key.to_owned(), canonical_json(value)?] {
            digest.update(part.as_bytes());
            digest.update([0]);
        }
        let signature: [u8; 16] = digest.finalize()[..16]
            .try_into()
            .expect("固定 SHA-256 长度");
        if seen.insert(signature) {
            self.write(stage, &entity)?;
        }
        Ok(())
    }
}

struct EntitySpool {
    root: PathBuf,
    streams: HashMap<String, BufWriter<File>>,
    counts: BTreeMap<String, u64>,
}

impl EntitySpool {
    fn new(root: &Path) -> Result<Self> {
        Ok(Self {
            root: root.to_owned(),
            streams: HashMap::new(),
            counts: BTreeMap::new(),
        })
    }

    fn write(&mut self, stage: &str, entity: &Value) -> Result<()> {
        if !self.streams.contains_key(stage) {
            let file = File::create(self.root.join(format!("{stage}.jsonl")))?;
            self.streams.insert(
                stage.to_owned(),
                BufWriter::with_capacity(1024 * 1024, file),
            );
        }
        let stream = self.streams.get_mut(stage).expect("刚插入的阶段流");
        serde_json::to_writer(&mut *stream, entity)?;
        stream.write_all(b"\n")?;
        *self.counts.entry(stage.to_owned()).or_default() += 1;
        Ok(())
    }

    fn finish(mut self) -> Result<BTreeMap<String, u64>> {
        for (_, mut stream) in self.streams.drain() {
            stream.flush()?;
        }
        Ok(self.counts)
    }
}

fn normalized_range(value: &Value) -> Option<Range> {
    let values = value.as_array()?;
    if values.len() != 2 {
        return None;
    }
    Some((values[0].as_i64()?, values[1].as_i64()?))
}

fn normalized_ranges(value: &Value) -> Vec<Range> {
    if let Some(object) = value.as_object() {
        for field in [
            "matched_ranges",
            "display_ranges",
            "source_ranges",
            "matchedRanges",
            "displayRanges",
            "sourceRanges",
        ] {
            let ranges: Vec<Range> = object
                .get(field)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(normalized_range)
                .collect();
            if !ranges.is_empty() {
                return ranges;
            }
        }
        for field in [
            "char_range",
            "anchor_range",
            "_quality_range",
            "charRange",
            "anchorRange",
            "range",
        ] {
            if let Some(range) = object.get(field).and_then(normalized_range) {
                return vec![range];
            }
        }
    }
    normalized_range(value).into_iter().collect()
}

fn span_of_ranges(ranges: &[Range]) -> Option<Range> {
    Some((
        ranges.iter().map(|value| value.0).min()?,
        ranges.iter().map(|value| value.1).max()?,
    ))
}

fn entity_key(kind: &str, ranges: &[Range], parts: &[Value]) -> String {
    let mut values = vec![
        kind.to_owned(),
        canonical_json(&json!(ranges)).expect("范围可序列化"),
    ];
    values.extend(parts.iter().filter(|value| !is_empty(value)).map(py_string));
    values.join("|")
}

fn context_for(value: &Value) -> String {
    value
        .as_object()
        .and_then(|object| {
            ["context", "surface", "label", "base_form", "query"]
                .iter()
                .find_map(|field| object.get(*field).filter(|value| !is_empty(value)))
        })
        .map(py_string)
        .unwrap_or_default()
}

fn range_key(value: &Value) -> String {
    canonical_json(value).expect("坐标可序列化")
}

fn resource_affects(kind: &str, name: &str) -> Vec<&'static str> {
    let lowered = name.to_lowercase();
    if kind == "cli" {
        return STAGE_ORDER
            .iter()
            .copied()
            .filter(|stage| *stage != "resource" && *stage != "source")
            .collect();
    }
    if kind == "profile" {
        return vec!["personalization", "expression_candidate", "expression"];
    }
    if kind == "system_dictionary" {
        return vec!["morpheme"];
    }
    if kind.starts_with("dictionary_") {
        return vec!["lexical_candidate", "lexical_unit", "expression_candidate"];
    }
    if lowered.contains("word") && lowered.contains("formation") {
        return vec!["word_formation_candidate", "word_formation"];
    }
    if lowered.contains("lexical") {
        return vec!["lexical_candidate", "lexical_unit"];
    }
    if lowered.contains("bunsetsu") {
        return vec!["bunsetsu_boundary", "bunsetsu"];
    }
    if lowered.contains("grammar") || lowered.contains("morph") {
        return vec![
            "morphology",
            "grammar_candidate",
            "grammar_occurrence",
            "grammar_projection",
            "grammar_residual",
        ];
    }
    if lowered.contains("expression") {
        return vec!["expression_candidate", "expression"];
    }
    vec![
        "word_formation",
        "word_formation_candidate",
        "lexical_candidate",
        "bunsetsu_boundary",
        "grammar_candidate",
        "grammar_occurrence",
        "expression_candidate",
    ]
}

fn artifact_stage_coverage(name: &str) -> &'static [&'static str] {
    match name {
        "tokens" => &[
            "morpheme",
            "morphology",
            "word_formation",
            "lexical_unit",
            "bunsetsu",
            "grammar_candidate",
            "grammar_occurrence",
            "grammar_projection",
            "grammar_residual",
            "personalization",
            "expression_candidate",
            "expression",
        ],
        "word_formations" => &["word_formation_candidate"],
        "lexical_candidates" => &["lexical_candidate"],
        "bunsetsu" => &[
            "morpheme",
            "word_formation",
            "lexical_unit",
            "bunsetsu_boundary",
            "bunsetsu",
        ],
        "grammar_occurrences" => &["grammar_candidate", "grammar_occurrence"],
        "grammar_residuals" => &["grammar_residual"],
        "expressions" => &["expression_candidate", "expression"],
        "catalogs" => &["resource"],
        "ui_projection" => &["ui_projection"],
        _ => &[],
    }
}

fn artifact_path(
    manifest_path: &Path,
    artifacts: &Map<String, Value>,
    name: &str,
) -> Result<Option<PathBuf>> {
    let Some(descriptor) = artifacts.get(name).and_then(Value::as_object) else {
        return Ok(None);
    };
    let relative = descriptor
        .get("path")
        .and_then(Value::as_str)
        .with_context(|| format!("产物 {name} 缺少 path"))?;
    let path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(relative);
    if !path.is_file() {
        bail!("快照产物不存在：{name} -> {}", path.display());
    }
    // tokens 在 snapshot 写入内容寻址存储时已校验；compare 再完整哈希会额外扫描多 GiB。
    if name != "tokens" {
        if let Some(expected) = descriptor.get("sha256").and_then(Value::as_str) {
            let actual = file_hash(&path)?;
            if expected != actual {
                bail!("快照产物哈希不一致：{name} -> {}", path.display());
            }
        }
    }
    Ok(Some(path))
}

fn load_artifact(
    manifest_path: &Path,
    artifacts: &Map<String, Value>,
    name: &str,
) -> Result<Option<Value>> {
    artifact_path(manifest_path, artifacts, name)?
        .map(|path| read_json(&path))
        .transpose()
}

fn read_json(path: &Path) -> Result<Value> {
    let file = File::open(path).with_context(|| format!("无法读取 {}", path.display()))?;
    let reader: Box<dyn Read> = if path.extension().is_some_and(|value| value == "gz") {
        Box::new(GzDecoder::new(BufReader::with_capacity(1024 * 1024, file)))
    } else {
        Box::new(BufReader::with_capacity(1024 * 1024, file))
    };
    serde_json::from_reader(reader).with_context(|| format!("无法解析 {}", path.display()))
}

fn for_each_json_array<F>(path: &Path, callback: F) -> Result<()>
where
    F: FnMut(Value) -> Result<()>,
{
    let file = File::open(path).with_context(|| format!("无法读取 {}", path.display()))?;
    let reader: Box<dyn Read> = if path.extension().is_some_and(|value| value == "gz") {
        Box::new(GzDecoder::new(BufReader::with_capacity(1024 * 1024, file)))
    } else {
        Box::new(BufReader::with_capacity(1024 * 1024, file))
    };
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    ArraySeed { callback }.deserialize(&mut deserializer)?;
    Ok(())
}

fn for_each_raw_json_array<F>(path: &Path, callback: F) -> Result<()>
where
    F: FnMut(Box<RawValue>) -> Result<()>,
{
    let file = File::open(path).with_context(|| format!("无法读取 {}", path.display()))?;
    let reader: Box<dyn Read> = if path.extension().is_some_and(|value| value == "gz") {
        Box::new(GzDecoder::new(BufReader::with_capacity(1024 * 1024, file)))
    } else {
        Box::new(BufReader::with_capacity(1024 * 1024, file))
    };
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    RawArraySeed { callback }.deserialize(&mut deserializer)?;
    Ok(())
}

struct ArraySeed<F> {
    callback: F,
}

impl<'de, F> DeserializeSeed<'de> for ArraySeed<F>
where
    F: FnMut(Value) -> Result<()>,
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(ArrayVisitor {
            callback: self.callback,
        })
    }
}

struct ArrayVisitor<F> {
    callback: F,
}

impl<'de, F> Visitor<'de> for ArrayVisitor<F>
where
    F: FnMut(Value) -> Result<()>,
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("顶层 JSON 数组")
    }

    fn visit_seq<A>(mut self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while let Some(value) = sequence.next_element::<Value>()? {
            (self.callback)(value).map_err(A::Error::custom)?;
        }
        Ok(())
    }
}

struct RawArraySeed<F> {
    callback: F,
}

impl<'de, F> DeserializeSeed<'de> for RawArraySeed<F>
where
    F: FnMut(Box<RawValue>) -> Result<()>,
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(RawArrayVisitor {
            callback: self.callback,
        })
    }
}

struct RawArrayVisitor<F> {
    callback: F,
}

impl<'de, F> Visitor<'de> for RawArrayVisitor<F>
where
    F: FnMut(Box<RawValue>) -> Result<()>,
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("顶层 JSON 数组")
    }

    fn visit_seq<A>(mut self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while let Some(value) = sequence.next_element::<Box<RawValue>>()? {
            (self.callback)(value).map_err(A::Error::custom)?;
        }
        Ok(())
    }
}

fn file_hash(path: &Path) -> Result<String> {
    let mut source = BufReader::with_capacity(1024 * 1024, File::open(path)?);
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut writer = BufWriter::new(File::create(path)?);
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn canonical_json(value: &Value) -> Result<String> {
    serde_json::to_string(value).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_token_keeps_lexical_lookup_contract() {
        let token = json!({
            "display_class": "content",
            "expressions": [{"rule_id": "expr"}],
            "bunsetsu": {
                "surface": "言った",
                "char_range": [4, 7],
                "head_word": {"surface": "言っ"},
                "morphemes": [],
                "word_formations": [],
                "lexical_units": [{
                    "base_form": "言う",
                    "reading": "イウ",
                    "output_pos": {"major": "動詞"}
                }],
                "grammar_occurrences": [],
                "grammar_tags": [],
                "functional_residuals": []
            }
        });
        let record = reading_token(&token).expect("阅读字段投影成功");
        assert_eq!(record["surface"], "言った");
        assert_eq!(record["char_range"], json!([4, 7]));
        assert_eq!(
            record["lookup_request"],
            json!({
                "word": "言う",
                "observed_form": "言う",
                "reading": "イウ",
                "pos": {"major": "動詞"}
            })
        );
        assert_eq!(record["expressions"], json!([{"rule_id": "expr"}]));
    }
}

fn object<const N: usize>(items: [(&str, Value); N]) -> Value {
    Value::Object(
        items
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn array_objects(value: Option<&Value>) -> impl Iterator<Item = &Map<String, Value>> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
}

fn is_empty(value: &Value) -> bool {
    value.is_null() || value.as_str() == Some("")
}

fn default_if_empty(value: &Value, default: Value) -> Value {
    if is_empty(value) {
        default
    } else {
        value.clone()
    }
}

fn as_i64_or_zero(value: &Value) -> i64 {
    value.as_i64().unwrap_or(0)
}

fn py_string(value: &Value) -> String {
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        _ => canonical_json(value).unwrap_or_default(),
    }
}
