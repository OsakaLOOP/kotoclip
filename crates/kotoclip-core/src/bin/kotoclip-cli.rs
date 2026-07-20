use kotoclip_core::cache::{AnalysisCache, CacheLoadPhase};
use kotoclip_core::dictionary::lookup::DictionaryEngine;
use kotoclip_core::document::{AnalysisStage, DocumentSession, StageInvalidation};
use kotoclip_core::models::PosTag;
use kotoclip_core::pipeline::{ruby, Pipeline};
use kotoclip_core::transport::CompactAnalysis;
use kotoclip_core::Engine;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Default)]
struct CliArgs {
    options: HashMap<String, String>,
    flags: HashSet<String>,
}

impl CliArgs {
    fn parse(values: impl Iterator<Item = String>) -> Result<Self, String> {
        let values: Vec<String> = values.collect();
        let mut parsed = Self::default();
        let mut index = 0;
        while index < values.len() {
            let value = &values[index];
            if !value.starts_with("--") {
                return Err(format!("无法识别的位置参数：{value}"));
            }
            let key = value.trim_start_matches("--").to_string();
            if index + 1 < values.len() && !values[index + 1].starts_with("--") {
                parsed.options.insert(key, values[index + 1].clone());
                index += 2;
            } else {
                parsed.flags.insert(key);
                index += 1;
            }
        }
        Ok(parsed)
    }

    fn required(&self, key: &str) -> Result<&str, String> {
        self.options
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| format!("缺少 --{key}"))
    }

    fn usize(&self, key: &str, default: usize) -> Result<usize, String> {
        self.options.get(key).map_or(Ok(default), |value| {
            value.parse().map_err(|_| format!("--{key} 必须是非负整数"))
        })
    }

    fn f64(&self, key: &str, default: f64) -> Result<f64, String> {
        self.options.get(key).map_or(Ok(default), |value| {
            value.parse().map_err(|_| format!("--{key} 必须是数字"))
        })
    }

    fn u64(&self, key: &str, default: u64) -> Result<u64, String> {
        self.options.get(key).map_or(Ok(default), |value| {
            value.parse().map_err(|_| format!("--{key} 必须是非负整数"))
        })
    }
}

#[derive(Debug, Serialize)]
struct MissedLexeme {
    base_form: String,
    reading: String,
    surfaces: Vec<String>,
    occurrences: usize,
}

#[derive(Debug, Serialize)]
struct CoverageReport {
    source: String,
    chapter: Option<String>,
    selected_line_start: usize,
    selected_line_end: usize,
    analyzed_nonempty_lines: usize,
    analyzed_characters: usize,
    lexical_occurrences: usize,
    headword_matches: usize,
    reading_matches: usize,
    unmatched: usize,
    coverage_rate: f64,
    reconstruction_pass_rate: f64,
    range_integrity_pass_rate: f64,
    grammar_tags: usize,
    missed_lexemes: Vec<MissedLexeme>,
}

#[derive(Debug, Serialize)]
struct PhaseTiming {
    phase: String,
    started_ms: u128,
    completed_ms: u128,
    duration_ms: u128,
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    characters: usize,
    tokens: usize,
    total_ms: u128,
    phases: Vec<PhaseTiming>,
}

#[derive(Debug, Serialize)]
struct ReaderLoadBenchmarkReport {
    source: String,
    chapter: Option<String>,
    source_read_ms: u128,
    chapter_extract_ms: u128,
    engine_initialization_ms: u128,
    analysis_total_ms: u128,
    ipc_payload_serialize_ms: u128,
    end_to_end_ms: u128,
    raw_characters: usize,
    analyzed_characters: usize,
    tokens: usize,
    ipc_payload_bytes: usize,
    /// 以下项均包含在 engine_initialization_ms 内，仅用于展示冷启动内部耗时。
    engine_initialization_details: Vec<kotoclip_core::performance::TimingEntry>,
    /// 以下项均包含在 analysis_total_ms 内，仅用于展示阶段内部实际调用耗时。
    analysis_details: Vec<kotoclip_core::performance::TimingEntry>,
}

#[derive(Debug, Serialize)]
struct SessionBenchmarkReport {
    source: String,
    chapter: Option<String>,
    analyzed_characters: usize,
    engine_initialization_ms: u128,
    first_batch_ms: u128,
    first_patch_bytes: usize,
    progressive_complete_ms: u128,
    progressive_patch_bytes: usize,
    deferred_expression_ms: u128,
    deferred_expression_patch_bytes: usize,
    expression_mutation_ms: u128,
    expression_changed_tokens: usize,
    expression_patch_bytes: usize,
    cache_store_ms: u128,
    warm_open_ms: u128,
    warm_cache_read_ms: u128,
    warm_cache_decode_ms: u128,
    warm_cache_validate_ms: u128,
    warm_session_prepare_ms: u128,
    warm_first_batch_select_ms: u128,
    warm_first_state_restore_ms: u128,
    warm_first_patch_ms: u128,
    warm_patch_bytes: usize,
    tokens: usize,
    progressive_reconstruction_ok: bool,
    warm_equals_progressive: bool,
}

#[derive(Debug, Serialize)]
struct IncrementalConsistencyReport {
    source: String,
    chapter: Option<String>,
    seed: u64,
    load_cases: usize,
    rule_cases: usize,
    analyzed_characters: usize,
    baseline_tokens: usize,
    randomized_loads_passed: usize,
    rule_additions_passed: usize,
    rule_deletions_passed: usize,
}

struct DeterministicRng(u64);

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn usize(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            (self.next_u64() as usize) % upper
        }
    }
}

#[derive(Default)]
struct MissAggregate {
    surfaces: HashSet<String>,
    count: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("错误：{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut values = std::env::args().skip(1);
    let command = values.next().unwrap_or_else(|| "help".to_string());
    let args = CliArgs::parse(values).map_err(io::Error::other)?;
    match command.as_str() {
        "dict-info" => dict_info(&args),
        "lookup" => lookup(&args),
        "dict-bubble-html" => dict_bubble_html(&args),
        "analyze" => analyze(&args),
        "grammar-inspect" => grammar_inspect(&args),
        "grammar-scan" => grammar_scan(&args),
        "grammar-residual" => grammar_residual(&args),
        "grammar-catalog" => grammar_catalog(&args),
        "grammar-explain" => grammar_explain(&args),
        "grammar-library-audit" => grammar_library_audit(&args),
        "grammar-audit" => grammar_audit(&args),
        "grammar-compare" => grammar_compare(&args),
        "grammar-review" => grammar_review(&args),
        "audit" => audit(&args),
        "benchmark" => benchmark(&args),
        "reader-benchmark" => reader_benchmark(&args),
        "session-benchmark" => session_benchmark(&args),
        "incremental-consistency" => incremental_consistency(&args),
        "nbest" => nbest(&args),
        "nbest-rank" => nbest_rank(&args),
        "nbest-choose" => nbest_choose(&args),
        "nbest-choices" => nbest_choices(&args),
        "nbest-repl" => nbest_repl(&args),
        "expression-list" => expression_list(&args),
        "expression-preview" => expression_preview(&args),
        "expression-scan" => expression_scan(&args),
        "word-formation-scan" => word_formation_scan(&args),
        "lexical-unit-scan" => lexical_unit_scan(&args),
        "bunsetsu-scan" => bunsetsu_scan(&args),
        "expression-verify" => expression_verify(&args),
        "expression-add" => expression_add(&args),
        "expression-repl" => expression_repl(&args),
        "schema-audit" => schema_audit(&args),
        "repl" => repl(&args),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        _ => Err(format!("未知命令：{command}。运行 help 查看用法。").into()),
    }
}

fn schema_audit(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let audits = vec![
        kotoclip_core::pipeline::word_formation::catalog_audit()?,
        kotoclip_core::pipeline::lexical::catalog_audit()?,
        kotoclip_core::pipeline::bunsetsu::catalog_audit()?,
        kotoclip_core::pipeline::expressions::catalog_audit(),
    ];
    if let Some(path) = args.options.get("json") {
        std::fs::write(path, serde_json::to_string_pretty(&audits)?)?;
    }
    let rules: usize = audits.iter().map(|audit| audit.rule_count).sum();
    let capabilities: HashSet<_> = audits
        .iter()
        .flat_map(|audit| audit.capabilities.iter())
        .collect();
    println!(
        "规则审计：层 {}，规则 {}，能力 {}，严格校验通过。",
        audits.len(),
        rules,
        capabilities.len()
    );
    Ok(())
}

fn dictionary(args: &CliArgs) -> Result<DictionaryEngine, Box<dyn Error>> {
    Ok(DictionaryEngine::prepare(
        args.options
            .get("dict-source-dir")
            .map_or("data/dict-sources", String::as_str),
        args.options
            .get("dict-dir")
            .map_or("data/dicts", String::as_str),
    )?)
}

fn pipeline(args: &CliArgs) -> Result<Pipeline, Box<dyn Error>> {
    Ok(Pipeline::new(
        args.options
            .get("system-dict")
            .map_or("ipadic/system.dic", String::as_str),
    )?)
}

fn engine(args: &CliArgs) -> Result<Engine, Box<dyn Error>> {
    Ok(Engine::new_from_dictionary_sources(
        args.options
            .get("system-dict")
            .map_or("ipadic/system.dic", String::as_str),
        args.options
            .get("dict-source-dir")
            .map_or("data/dict-sources", String::as_str),
        args.options
            .get("dict-dir")
            .map_or("data/dicts", String::as_str),
        args.required("profile").map_err(io::Error::other)?,
    )?)
}

fn dict_info(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let stats = dictionary(args)?.stats();
    println!("{}", serde_json::to_string_pretty(&stats)?);
    Ok(())
}

fn lookup(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let word = args.required("word").map_err(io::Error::other)?;
    let reading = args.options.get("reading").map(String::as_str);
    let (results, timing) = dictionary(args)?.lookup_profiled(word, reading);
    if results.is_empty() {
        println!("未命中：{word}");
        return Ok(());
    }
    for (index, entry) in results.iter().enumerate() {
        let definition = if args.flags.contains("full") {
            entry.definition_html.clone()
        } else {
            entry.definition_html.chars().take(500).collect()
        };
        println!(
            "[{}] {} / {} / {}\n{}\n",
            index + 1,
            entry.dict_name,
            entry.match_type,
            entry.headword,
            definition
        );
    }
    if args.flags.contains("timing") {
        println!("诊断耗时：{}", serde_json::to_string_pretty(&timing)?);
    }
    Ok(())
}

fn dict_bubble_html(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let word = args.required("word").map_err(io::Error::other)?;
    let reading = args.options.get("reading").map(String::as_str);
    let pos = cli_pos(args);

    let (entries, timing) = dictionary(args)?.lookup_profiled_with_pos(word, reading, pos.as_ref());
    let lookup = kotoclip_core::dictionary::lookup_state::build_lookup(
        word,
        reading,
        None,
        "contextual-cli",
        &entries,
        entries.clone(),
        Some(timing.clone()),
    );

    let html_content =
        kotoclip_core::dictionary::bubble_html::generate_bubble_preview_html(&lookup);

    // JSON 与 HTML 使用同一份完整 Lookup，便于检查候选、活动 occurrence 和词典可用性。
    if let Some(json_path) = args.options.get("json") {
        std::fs::write(json_path, serde_json::to_string_pretty(&lookup)?)?;
        println!("Lookup JSON 已保存至：{}", json_path);
    }

    if args.flags.contains("raw") {
        // --raw 模式下直接输出到 stdout
        print!("{}", html_content);
    } else {
        // 确定输出文件路径
        let output_path = if let Some(path) = args.options.get("output") {
            PathBuf::from(path)
        } else {
            // 没有指定时生成到临时文件目录
            let mut temp_dir = std::env::temp_dir();
            temp_dir.push(format!("kotoclip_dict_preview_{}.html", word));
            temp_dir
        };

        std::fs::write(&output_path, &html_content)?;
        println!("HTML 已渲染并保存至：{}", output_path.display());

        if !args.flags.contains("no-open") {
            let status = std::process::Command::new("cmd")
                .args(["/C", "start", "", &output_path.to_string_lossy()])
                .status();

            if let Err(e) = status {
                eprintln!(
                    "自动打开浏览器失败：{}，您可以手动在浏览器中打开该文件。",
                    e
                );
            }
        }
    }

    if args.flags.contains("timing") {
        println!("诊断耗时：{}", serde_json::to_string_pretty(&timing)?);
    }

    Ok(())
}

fn cli_pos(args: &CliArgs) -> Option<PosTag> {
    let major = args.options.get("pos-major")?.clone();
    Some(PosTag {
        major,
        sub1: args.options.get("pos-sub1").cloned().unwrap_or_default(),
        sub2: args.options.get("pos-sub2").cloned().unwrap_or_default(),
        sub3: args.options.get("pos-sub3").cloned().unwrap_or_default(),
    })
}

fn analyze(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_argument(args)?;
    let dictionary = dictionary(args)?;
    let pipeline = pipeline(args)?;
    let tokens = pipeline.process_with_dictionary(&text, &[], &dictionary);
    println!("{}", serde_json::to_string_pretty(&tokens)?);
    Ok(())
}

fn grammar_inspect(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_argument(args)?;
    let tokens = pipeline(args)?.process(&text, &[]);
    println!("{}", serde_json::to_string_pretty(&tokens)?);
    Ok(())
}

fn grammar_scan(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let tokens = pipeline(args)?.process(&text, &[]);
    let include_pending = args.flags.contains("include-pending");
    let include_rejected = args.flags.contains("include-rejected");
    let occurrences = tokens
        .iter()
        .flat_map(|token| &token.bunsetsu.grammar_occurrences)
        .filter(|occurrence| {
            matches!(
                occurrence.status,
                kotoclip_core::models::GrammarOccurrenceStatus::Accepted
            ) || (include_pending
                && matches!(
                    occurrence.status,
                    kotoclip_core::models::GrammarOccurrenceStatus::Pending
                ))
                || (include_rejected
                    && matches!(
                        occurrence.status,
                        kotoclip_core::models::GrammarOccurrenceStatus::Rejected
                    ))
        })
        .cloned()
        .collect::<Vec<_>>();
    output_json(args, &occurrences)
}

#[derive(Debug, Serialize)]
struct GrammarResidualReport {
    characters: usize,
    non_punctuation_morphemes: usize,
    residuals: usize,
    functional_residual_rate: f64,
    items: Vec<kotoclip_core::models::FunctionalResidual>,
}

fn grammar_residual(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let tokens = pipeline(args)?.process(&text, &[]);
    let non_punctuation_morphemes = tokens
        .iter()
        .flat_map(|token| &token.bunsetsu.morphemes)
        .filter(|morpheme| morpheme.pos.major != "記号" && !morpheme.surface.trim().is_empty())
        .count();
    let items = tokens
        .iter()
        .flat_map(|token| token.bunsetsu.functional_residuals.clone())
        .collect::<Vec<_>>();
    let report = GrammarResidualReport {
        characters: text.chars().count(),
        non_punctuation_morphemes,
        residuals: items.len(),
        functional_residual_rate: if non_punctuation_morphemes == 0 {
            0.0
        } else {
            items.len() as f64 / non_punctuation_morphemes as f64
        },
        items,
    };
    output_json(args, &report)
}

fn grammar_catalog(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let catalog = kotoclip_core::pipeline::grammar::catalog::GrammarCatalog::load_embedded()?;
    let jlpt = args
        .options
        .get("jlpt")
        .and_then(|value| value.parse::<u8>().ok());
    let concepts = catalog.search(
        args.options.get("query").map(String::as_str),
        args.options.get("family").map(String::as_str),
        jlpt,
        args.options.get("status").map(String::as_str),
        args.options.get("source-ref").map(String::as_str),
    );
    output_json(args, &concepts)
}

fn grammar_explain(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    if args.options.contains_key("text") || args.options.contains_key("source") {
        let text = read_text_selection(args)?;
        let tokens = pipeline(args)?.process(&text, &[]);
        let requested = args.options.get("occurrence");
        let explanations = tokens
            .iter()
            .flat_map(|token| &token.bunsetsu.grammar_tags)
            .filter(|tag| requested.is_none_or(|value| &tag.occurrence_id == value))
            .map(|tag| (&tag.occurrence_id, &tag.concept_id, &tag.explanation))
            .collect::<Vec<_>>();
        return output_json(args, &explanations);
    }
    let concept_id = args.required("concept").map_err(io::Error::other)?;
    let catalog = kotoclip_core::pipeline::grammar::catalog::GrammarCatalog::load_embedded()?;
    let concept = catalog
        .concept(concept_id)
        .ok_or_else(|| io::Error::other(format!("未知 concept：{concept_id}")))?;
    let explanation = catalog
        .explanation(&concept.default_explanation_id)
        .ok_or_else(|| io::Error::other("concept 缺少讲解"))?;
    output_json(
        args,
        &(concept, catalog.senses_for(concept_id), explanation),
    )
}

fn grammar_library_audit(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let catalog = kotoclip_core::pipeline::grammar::catalog::GrammarCatalog::load_embedded()?;
    output_json(args, &catalog.audit())
}

#[derive(Debug, Deserialize)]
struct GrammarRepresentativeCase {
    id: String,
    text: String,
    expected_concepts: Vec<String>,
    #[serde(default)]
    forbidden_concepts: Vec<String>,
}

#[derive(Debug, Serialize)]
struct GrammarCaseResult {
    id: String,
    text: String,
    passed: bool,
    found_concepts: Vec<String>,
    missing_concepts: Vec<String>,
    forbidden_hits: Vec<String>,
    reconstruction_ok: bool,
    explanation_resolution_ok: bool,
}

fn grammar_audit(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let path = args.options.get("cases").map_or(
        "crates/kotoclip-core/tests/fixtures/grammar_representative_cases.json",
        String::as_str,
    );
    let cases: Vec<GrammarRepresentativeCase> =
        serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let pipeline = pipeline(args)?;
    let mut results = Vec::new();
    for case in cases {
        let tokens = pipeline.process(&case.text, &[]);
        let reconstructed = tokens
            .iter()
            .map(|token| token.bunsetsu.surface.as_str())
            .collect::<String>();
        let found = tokens
            .iter()
            .flat_map(|token| &token.bunsetsu.grammar_occurrences)
            .filter(|occurrence| {
                matches!(
                    occurrence.status,
                    kotoclip_core::models::GrammarOccurrenceStatus::Accepted
                )
            })
            .map(|occurrence| occurrence.concept_id.clone())
            .collect::<HashSet<_>>();
        let missing_concepts = case
            .expected_concepts
            .iter()
            .filter(|item| !found.contains(*item))
            .cloned()
            .collect::<Vec<_>>();
        let forbidden_hits = case
            .forbidden_concepts
            .iter()
            .filter(|item| found.contains(*item))
            .cloned()
            .collect::<Vec<_>>();
        let explanation_resolution_ok = tokens
            .iter()
            .flat_map(|token| &token.bunsetsu.grammar_tags)
            .all(|tag| tag.explanation.is_some());
        let reconstruction_ok = reconstructed == case.text;
        results.push(GrammarCaseResult {
            id: case.id,
            text: case.text,
            passed: missing_concepts.is_empty()
                && forbidden_hits.is_empty()
                && reconstruction_ok
                && explanation_resolution_ok,
            found_concepts: found.into_iter().collect(),
            missing_concepts,
            forbidden_hits,
            reconstruction_ok,
            explanation_resolution_ok,
        });
    }
    let all_passed = results.iter().all(|result| result.passed);
    output_json(args, &results)?;
    if !all_passed {
        return Err("语法代表性用例未全部通过".into());
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct GrammarCompareReport {
    added: Vec<String>,
    removed: Vec<String>,
    unchanged: usize,
}

fn grammar_compare(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let before_path = args.required("before").map_err(io::Error::other)?;
    let after_path = args.required("after").map_err(io::Error::other)?;
    let before: Vec<kotoclip_core::models::GrammarOccurrence> =
        serde_json::from_str(&std::fs::read_to_string(before_path)?)?;
    let after: Vec<kotoclip_core::models::GrammarOccurrence> =
        serde_json::from_str(&std::fs::read_to_string(after_path)?)?;
    let before_ids = before
        .into_iter()
        .map(|item| item.occurrence_id)
        .collect::<HashSet<_>>();
    let after_ids = after
        .into_iter()
        .map(|item| item.occurrence_id)
        .collect::<HashSet<_>>();
    let report = GrammarCompareReport {
        added: after_ids.difference(&before_ids).cloned().collect(),
        removed: before_ids.difference(&after_ids).cloned().collect(),
        unchanged: before_ids.intersection(&after_ids).count(),
    };
    output_json(args, &report)
}

#[derive(Debug, Serialize)]
struct GrammarReviewOccurrence {
    occurrence_id: String,
    concept_id: String,
    rule_id: String,
    status: String,
    char_range: (usize, usize),
    actual_form: String,
    explanation_ready: bool,
}

#[derive(Debug, Serialize)]
struct GrammarReviewMorpheme {
    surface: String,
    base_form: String,
    pos_major: String,
    pos_sub1: String,
    conjugation_type: String,
    conjugation_form: String,
    char_range: (usize, usize),
    bunsetsu_surface: String,
}

#[derive(Debug, Serialize)]
struct GrammarReviewItem {
    review_id: String,
    line_number: usize,
    text: String,
    issue_score: usize,
    accepted: Vec<GrammarReviewOccurrence>,
    pending: Vec<GrammarReviewOccurrence>,
    rejected: Vec<GrammarReviewOccurrence>,
    residuals: Vec<kotoclip_core::models::FunctionalResidual>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    morphemes: Vec<GrammarReviewMorpheme>,
}

#[derive(Debug, Serialize)]
struct GrammarReviewBatch {
    batch: usize,
    batch_size: usize,
    total_items: usize,
    total_batches: usize,
    family: Option<String>,
    items: Vec<GrammarReviewItem>,
}

#[derive(Debug, Serialize)]
struct GrammarResidualReviewSample {
    line_number: usize,
    text: String,
    bunsetsu_surface: String,
    residual: kotoclip_core::models::FunctionalResidual,
    morphemes: Vec<GrammarReviewMorpheme>,
}

#[derive(Debug, Serialize)]
struct GrammarResidualReviewCandidate {
    review_id: String,
    base_form: String,
    pos_major: String,
    pos_sub1: String,
    conjugation_type: String,
    conjugation_form: String,
    reason: String,
    occurrences: usize,
    surfaces: Vec<String>,
    samples: Vec<GrammarResidualReviewSample>,
}

#[derive(Debug, Serialize)]
struct GrammarResidualReviewBatch {
    batch: usize,
    batch_size: usize,
    total_items: usize,
    total_batches: usize,
    items: Vec<GrammarResidualReviewCandidate>,
}

#[derive(Default)]
struct GrammarResidualReviewAggregate {
    occurrences: usize,
    surfaces: HashSet<String>,
    samples: Vec<GrammarResidualReviewSample>,
}

fn grammar_review(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let tokens = pipeline(args)?.process(&text, &[]);
    if args.flags.contains("group-residuals") {
        return grammar_review_residual_candidates(args, &text, &tokens);
    }
    let catalog = kotoclip_core::pipeline::grammar::catalog::GrammarCatalog::load_embedded()?;
    let family = args.options.get("family").cloned();
    let mut line_ranges = Vec::new();
    let mut offset = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let prepared = ruby::prepare_text(line).text;
        let length = prepared.chars().count();
        line_ranges.push((
            index + 1,
            offset,
            offset + length,
            line.trim_end_matches(['\r', '\n']).to_string(),
        ));
        offset += length;
    }
    if line_ranges.is_empty() && !text.is_empty() {
        line_ranges.push((1, 0, text.chars().count(), text.clone()));
    }

    let mut grouped: BTreeMap<usize, GrammarReviewItem> = BTreeMap::new();
    for token in &tokens {
        for occurrence in &token.bunsetsu.grammar_occurrences {
            if matches!(
                occurrence.status,
                kotoclip_core::models::GrammarOccurrenceStatus::Accepted
            ) && !occurrence.show_badge
                && !args.flags.contains("include-atoms")
            {
                continue;
            }
            let family_matches = family.as_ref().is_none_or(|requested| {
                catalog
                    .concept(&occurrence.concept_id)
                    .is_some_and(|concept| {
                        concept.kind == *requested
                            || concept
                                .semantic_domains
                                .iter()
                                .any(|item| item == requested)
                            || concept.function_tags.iter().any(|item| item == requested)
                    })
            });
            if !family_matches {
                continue;
            }
            let Some((line_number, _, _, line_text)) =
                line_ranges.iter().find(|(_, start, end, _)| {
                    occurrence.anchor_range.0 >= *start && occurrence.anchor_range.0 < *end
                })
            else {
                continue;
            };
            let item = grouped
                .entry(*line_number)
                .or_insert_with(|| GrammarReviewItem {
                    review_id: format!("line-{line_number}"),
                    line_number: *line_number,
                    text: line_text.clone(),
                    issue_score: 0,
                    accepted: Vec::new(),
                    pending: Vec::new(),
                    rejected: Vec::new(),
                    residuals: Vec::new(),
                    morphemes: Vec::new(),
                });
            let view = GrammarReviewOccurrence {
                occurrence_id: occurrence.occurrence_id.clone(),
                concept_id: occurrence.concept_id.clone(),
                rule_id: occurrence.rule_id.clone(),
                status: format!("{:?}", occurrence.status).to_ascii_lowercase(),
                char_range: occurrence.anchor_range,
                actual_form: occurrence
                    .captures
                    .iter()
                    .map(|capture| capture.surface.as_str())
                    .collect(),
                explanation_ready: catalog
                    .concept(&occurrence.concept_id)
                    .and_then(|concept| catalog.explanation(&concept.default_explanation_id))
                    .is_some(),
            };
            match occurrence.status {
                kotoclip_core::models::GrammarOccurrenceStatus::Accepted => {
                    item.accepted.push(view)
                }
                kotoclip_core::models::GrammarOccurrenceStatus::Pending => {
                    item.issue_score += 3;
                    item.pending.push(view);
                }
                kotoclip_core::models::GrammarOccurrenceStatus::Rejected => {
                    item.issue_score += 1;
                    item.rejected.push(view);
                }
                kotoclip_core::models::GrammarOccurrenceStatus::Unknown => {
                    item.issue_score += 4;
                    item.pending.push(view);
                }
            }
        }
        for residual in &token.bunsetsu.functional_residuals {
            let Some((line_number, _, _, line_text)) =
                line_ranges.iter().find(|(_, start, end, _)| {
                    residual.char_range.0 >= *start && residual.char_range.0 < *end
                })
            else {
                continue;
            };
            let item = grouped
                .entry(*line_number)
                .or_insert_with(|| GrammarReviewItem {
                    review_id: format!("line-{line_number}"),
                    line_number: *line_number,
                    text: line_text.clone(),
                    issue_score: 0,
                    accepted: Vec::new(),
                    pending: Vec::new(),
                    rejected: Vec::new(),
                    residuals: Vec::new(),
                    morphemes: Vec::new(),
                });
            item.issue_score += 5;
            item.residuals.push(residual.clone());
        }
    }
    if args.flags.contains("include-morphemes") {
        for token in &tokens {
            for morpheme in &token.bunsetsu.morphemes {
                let Some((line_number, _, _, _)) = line_ranges.iter().find(|(_, start, end, _)| {
                    morpheme.char_range.0 >= *start && morpheme.char_range.0 < *end
                }) else {
                    continue;
                };
                let Some(item) = grouped.get_mut(line_number) else {
                    continue;
                };
                item.morphemes.push(GrammarReviewMorpheme {
                    surface: morpheme.surface.clone(),
                    base_form: morpheme.base_form.clone(),
                    pos_major: morpheme.pos.major.clone(),
                    pos_sub1: morpheme.pos.sub1.clone(),
                    conjugation_type: morpheme.conjugation_type.clone(),
                    conjugation_form: morpheme.conjugation_form.clone(),
                    char_range: morpheme.char_range,
                    bunsetsu_surface: token.bunsetsu.surface.clone(),
                });
            }
        }
    }
    let mut items = grouped.into_values().collect::<Vec<_>>();
    if let Some(value) = args.options.get("lines") {
        let selected = value
            .split(',')
            .map(|item| {
                item.trim()
                    .parse::<usize>()
                    .map_err(|_| format!("--lines 含非法行号：{item}"))
            })
            .collect::<Result<HashSet<_>, _>>()
            .map_err(io::Error::other)?;
        items.retain(|item| selected.contains(&item.line_number));
    }
    if args.flags.contains("issues-first") {
        items.sort_by_key(|item| (std::cmp::Reverse(item.issue_score), item.line_number));
    }
    let batch_size = args.usize("batch-size", 30).map_err(io::Error::other)?;
    if !(20..=50).contains(&batch_size) {
        return Err("--batch-size 必须在 20 到 50 之间".into());
    }
    let batch = args.usize("batch", 1).map_err(io::Error::other)?.max(1);
    let total_items = items.len();
    let total_batches = total_items.div_ceil(batch_size).max(1);
    let start = (batch - 1).saturating_mul(batch_size).min(total_items);
    let end = (start + batch_size).min(total_items);
    let report = GrammarReviewBatch {
        batch,
        batch_size,
        total_items,
        total_batches,
        family,
        items: items.drain(start..end).collect(),
    };
    output_json(args, &report)
}

fn grammar_review_residual_candidates(
    args: &CliArgs,
    text: &str,
    tokens: &[kotoclip_core::models::AnnotatedToken],
) -> Result<(), Box<dyn Error>> {
    let mut line_ranges = Vec::new();
    let mut offset = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let prepared = ruby::prepare_text(line).text;
        let length = prepared.chars().count();
        line_ranges.push((
            index + 1,
            offset,
            offset + length,
            line.trim_end_matches(['\r', '\n']).to_string(),
        ));
        offset += length;
    }
    if line_ranges.is_empty() && !text.is_empty() {
        line_ranges.push((1, 0, text.chars().count(), text.to_string()));
    }

    let sample_count = args
        .usize("sample-count", 4)
        .map_err(io::Error::other)?
        .clamp(1, 12);
    let mut grouped: BTreeMap<
        (String, String, String, String, String, String),
        GrammarResidualReviewAggregate,
    > = BTreeMap::new();
    for token in tokens {
        for residual in &token.bunsetsu.functional_residuals {
            let key = (
                residual.base_form.clone(),
                residual.pos.major.clone(),
                residual.pos.sub1.clone(),
                residual.conjugation_type.clone(),
                residual.conjugation_form.clone(),
                residual.reason.clone(),
            );
            let aggregate = grouped.entry(key).or_default();
            aggregate.occurrences += 1;
            aggregate.surfaces.insert(residual.surface.clone());
            if aggregate.samples.len() >= sample_count {
                continue;
            }
            let Some((line_number, _, _, line_text)) =
                line_ranges.iter().find(|(_, start, end, _)| {
                    residual.char_range.0 >= *start && residual.char_range.0 < *end
                })
            else {
                continue;
            };
            let morphemes = token
                .bunsetsu
                .morphemes
                .iter()
                .map(|morpheme| GrammarReviewMorpheme {
                    surface: morpheme.surface.clone(),
                    base_form: morpheme.base_form.clone(),
                    pos_major: morpheme.pos.major.clone(),
                    pos_sub1: morpheme.pos.sub1.clone(),
                    conjugation_type: morpheme.conjugation_type.clone(),
                    conjugation_form: morpheme.conjugation_form.clone(),
                    char_range: morpheme.char_range,
                    bunsetsu_surface: token.bunsetsu.surface.clone(),
                })
                .collect();
            aggregate.samples.push(GrammarResidualReviewSample {
                line_number: *line_number,
                text: line_text.clone(),
                bunsetsu_surface: token.bunsetsu.surface.clone(),
                residual: residual.clone(),
                morphemes,
            });
        }
    }

    let mut items = grouped
        .into_iter()
        .map(
            |(
                (base_form, pos_major, pos_sub1, conjugation_type, conjugation_form, reason),
                aggregate,
            )| {
                let mut surfaces = aggregate.surfaces.into_iter().collect::<Vec<_>>();
                surfaces.sort();
                GrammarResidualReviewCandidate {
                    review_id: format!(
                        "residual:{base_form}:{pos_major}:{pos_sub1}:{conjugation_type}:{conjugation_form}"
                    ),
                    base_form,
                    pos_major,
                    pos_sub1,
                    conjugation_type,
                    conjugation_form,
                    reason,
                    occurrences: aggregate.occurrences,
                    surfaces,
                    samples: aggregate.samples,
                }
            },
        )
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .occurrences
            .cmp(&left.occurrences)
            .then_with(|| left.base_form.cmp(&right.base_form))
            .then_with(|| left.pos_major.cmp(&right.pos_major))
            .then_with(|| left.pos_sub1.cmp(&right.pos_sub1))
    });

    let batch_size = args.usize("batch-size", 30).map_err(io::Error::other)?;
    if !(20..=50).contains(&batch_size) {
        return Err("--batch-size 必须在 20 到 50 之间".into());
    }
    let batch = args.usize("batch", 1).map_err(io::Error::other)?.max(1);
    let total_items = items.len();
    let total_batches = total_items.div_ceil(batch_size).max(1);
    let start = (batch - 1).saturating_mul(batch_size).min(total_items);
    let end = (start + batch_size).min(total_items);
    let report = GrammarResidualReviewBatch {
        batch,
        batch_size,
        total_items,
        total_batches,
        items: items.drain(start..end).collect(),
    };
    output_json(args, &report)
}

fn output_json<T: Serialize>(args: &CliArgs, value: &T) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string_pretty(value)?;
    if let Some(path) = args.options.get("json") {
        std::fs::write(path, &json)?;
    }
    if !args.flags.contains("quiet") {
        println!("{json}");
    }
    Ok(())
}

fn audit(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let source_path = PathBuf::from(args.required("source").map_err(io::Error::other)?);
    let source = std::fs::read_to_string(&source_path)?;
    let chapter = args.options.get("chapter").cloned();
    let chapter_text = extract_chapter(&source, chapter.as_deref())?;
    let all_lines: Vec<&str> = chapter_text.lines().collect();
    let page_lines = args.usize("page-lines", 0).map_err(io::Error::other)?;
    let page = args.usize("page", 1).map_err(io::Error::other)?.max(1);
    let explicit_start = args
        .usize("start-line", 1)
        .map_err(io::Error::other)?
        .max(1);
    let start = if page_lines > 0 {
        (page - 1) * page_lines + 1
    } else {
        explicit_start
    };
    let default_count = if page_lines > 0 {
        page_lines
    } else {
        all_lines.len()
    };
    let line_count = args
        .usize("line-count", default_count)
        .map_err(io::Error::other)?;
    let end_exclusive = start
        .saturating_sub(1)
        .saturating_add(line_count)
        .min(all_lines.len());
    let sample_every = args
        .usize("sample-every", 1)
        .map_err(io::Error::other)?
        .max(1);

    let dictionary = dictionary(args)?;
    let pipeline = pipeline(args)?;
    let mut lexical_occurrences: usize = 0;
    let mut headword_matches: usize = 0;
    let mut reading_matches: usize = 0;
    let mut analyzed_lines = 0;
    let mut analyzed_characters = 0;
    let mut reconstruction_passes = 0;
    let mut integrity_passes = 0;
    let mut grammar_tags = 0;
    let mut misses: BTreeMap<(String, String), MissAggregate> = BTreeMap::new();

    for (offset, line) in all_lines[start.saturating_sub(1)..end_exclusive]
        .iter()
        .enumerate()
    {
        if offset % sample_every != 0 || line.trim().is_empty() {
            continue;
        }
        analyzed_lines += 1;
        let prepared = ruby::prepare_text(line);
        analyzed_characters += prepared.text.chars().count();
        let tokens = pipeline.process_with_dictionary(line, &[], &dictionary);
        let reconstructed: String = tokens
            .iter()
            .map(|token| token.bunsetsu.surface.as_str())
            .collect();
        if reconstructed == prepared.text {
            reconstruction_passes += 1;
        }
        if ranges_are_valid(&tokens, prepared.text.chars().count()) {
            integrity_passes += 1;
        }

        for token in tokens {
            grammar_tags += token.bunsetsu.grammar_tags.len();
            let head = &token.bunsetsu.head_word;
            if !is_lexical(&head.pos.major)
                || head.base_form.trim().is_empty()
                || !head.surface.chars().any(char::is_alphanumeric)
            {
                continue;
            }
            lexical_occurrences += 1;
            let mut matched = dictionary.match_kind(&head.base_form, Some(&head.reading));
            if matched.is_none() && head.surface != head.base_form {
                matched = dictionary.match_kind(&head.surface, Some(&head.reading));
            }
            match matched.as_deref() {
                Some("headword") => headword_matches += 1,
                Some("reading") => reading_matches += 1,
                _ => {
                    let miss = misses
                        .entry((head.base_form.clone(), head.reading.clone()))
                        .or_default();
                    miss.count += 1;
                    miss.surfaces.insert(head.surface.clone());
                }
            }
        }
    }

    let unmatched = lexical_occurrences.saturating_sub(headword_matches + reading_matches);
    let ratio = |passed: usize, total: usize| {
        if total == 0 {
            1.0
        } else {
            passed as f64 / total as f64
        }
    };
    let mut missed_lexemes: Vec<MissedLexeme> = misses
        .into_iter()
        .map(|((base_form, reading), aggregate)| {
            let mut surfaces: Vec<String> = aggregate.surfaces.into_iter().collect();
            surfaces.sort();
            MissedLexeme {
                base_form,
                reading,
                surfaces,
                occurrences: aggregate.count,
            }
        })
        .collect();
    missed_lexemes.sort_by(|left, right| {
        right
            .occurrences
            .cmp(&left.occurrences)
            .then_with(|| left.base_form.cmp(&right.base_form))
    });
    missed_lexemes.truncate(args.usize("max-misses", 100).map_err(io::Error::other)?);

    let report = CoverageReport {
        source: source_path.display().to_string(),
        chapter,
        selected_line_start: start,
        selected_line_end: end_exclusive,
        analyzed_nonempty_lines: analyzed_lines,
        analyzed_characters,
        lexical_occurrences,
        headword_matches,
        reading_matches,
        unmatched,
        coverage_rate: ratio(headword_matches + reading_matches, lexical_occurrences),
        reconstruction_pass_rate: ratio(reconstruction_passes, analyzed_lines),
        range_integrity_pass_rate: ratio(integrity_passes, analyzed_lines),
        grammar_tags,
        missed_lexemes,
    };
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(output) = args.options.get("json") {
        std::fs::write(output, &json)?;
        println!("报告已写入：{output}");
    }
    println!("{json}");
    let minimum = args.f64("min-coverage", 0.10).map_err(io::Error::other)?;
    if report.coverage_rate < minimum {
        return Err(format!(
            "覆盖率 {:.2}% 低于门槛 {:.2}%",
            report.coverage_rate * 100.0,
            minimum * 100.0
        )
        .into());
    }
    Ok(())
}

fn benchmark(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let source_path = PathBuf::from(args.required("source").map_err(io::Error::other)?);
    let source = std::fs::read_to_string(&source_path)?;
    let text = limit_benchmark_text(
        extract_chapter(&source, args.options.get("chapter").map(String::as_str))?,
        args.usize("max-chars", 0).map_err(io::Error::other)?,
    );
    let profile = args.required("profile").map_err(io::Error::other)?;
    let engine = Engine::new(
        args.options
            .get("system-dict")
            .map_or("ipadic/system.dic", String::as_str),
        args.options
            .get("dict-dir")
            .map_or("data/dicts", String::as_str),
        profile,
    )?;
    let started = Instant::now();
    let mut current_phase: Option<(String, u128)> = None;
    let mut phases = Vec::new();
    let tokens = engine.analyze_text_with_progress(
        &text,
        !args.flags.contains("no-record-exposure"),
        |event| {
            let elapsed = started.elapsed().as_millis();
            let phase = format!("{:?}", event.phase);
            if current_phase
                .as_ref()
                .map_or(true, |(current, _)| current != &phase)
            {
                if let Some((previous, phase_started)) = current_phase.replace((phase, elapsed)) {
                    phases.push(PhaseTiming {
                        phase: previous,
                        started_ms: phase_started,
                        completed_ms: elapsed,
                        duration_ms: elapsed.saturating_sub(phase_started),
                    });
                }
            }
        },
    )?;
    let total_ms = started.elapsed().as_millis();
    if let Some((phase, phase_started)) = current_phase {
        phases.push(PhaseTiming {
            phase,
            started_ms: phase_started,
            completed_ms: total_ms,
            duration_ms: total_ms.saturating_sub(phase_started),
        });
    }
    let report = BenchmarkReport {
        characters: text.chars().count(),
        tokens: tokens.len(),
        total_ms,
        phases,
    };
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(output) = args.options.get("json") {
        std::fs::write(output, &json)?;
        println!("基准报告已写入：{output}");
    }
    println!("{json}");
    Ok(())
}

/// 从读取源文件到构造 Tauri IPC 返回负载的端到端阅读器后端基准。
/// 调用方可传入临时 profile 并使用 --no-record-exposure，避免污染用户画像。
fn reader_benchmark(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let total_started = Instant::now();
    let source_path = PathBuf::from(args.required("source").map_err(io::Error::other)?);

    let read_started = Instant::now();
    let source = std::fs::read_to_string(&source_path)?;
    let source_read_ms = read_started.elapsed().as_millis();

    let chapter = args.options.get("chapter").cloned();
    let chapter_started = Instant::now();
    let text = limit_benchmark_text(
        extract_chapter(&source, chapter.as_deref())?,
        args.usize("max-chars", 0).map_err(io::Error::other)?,
    );
    let chapter_extract_ms = chapter_started.elapsed().as_millis();

    let engine_started = Instant::now();
    let (engine, initialization_timings) = Engine::new_profiled(
        args.options
            .get("system-dict")
            .map_or("ipadic/system.dic", String::as_str),
        args.options
            .get("dict-dir")
            .map_or("data/dicts", String::as_str),
        args.required("profile").map_err(io::Error::other)?,
    )?;
    let engine_initialization_ms = engine_started.elapsed().as_millis();

    let analysis_started = Instant::now();
    let (tokens, timings) =
        engine.analyze_text_profiled(&text, !args.flags.contains("no-record-exposure"))?;
    let analysis_total_ms = analysis_started.elapsed().as_millis();

    // 桌面端热路径使用字符串表紧凑模型；这里保持与 Tauri 返回体一致，
    // 以便同时测量实际序列化时间与传输字节数。
    let serialization_started = Instant::now();
    let ipc_payload = serde_json::to_vec(&CompactAnalysis::from(tokens.as_slice()))?;
    let ipc_payload_serialize_ms = serialization_started.elapsed().as_millis();

    let report = ReaderLoadBenchmarkReport {
        source: source_path.display().to_string(),
        chapter,
        source_read_ms,
        chapter_extract_ms,
        engine_initialization_ms,
        analysis_total_ms,
        ipc_payload_serialize_ms,
        end_to_end_ms: total_started.elapsed().as_millis(),
        raw_characters: text.chars().count(),
        analyzed_characters: ruby::prepare_text(&text).text.chars().count(),
        tokens: tokens.len(),
        ipc_payload_bytes: ipc_payload.len(),
        engine_initialization_details: initialization_timings.entries(),
        analysis_details: timings.entries(),
    };
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(output) = args.options.get("json") {
        std::fs::write(output, &json)?;
        println!("报告已写入：{output}");
    }
    println!("{json}");
    Ok(())
}

/// 文档会话冷首批、渐进补全、局部 mutation 与暖启动的统一基准。
fn session_benchmark(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let source_path = PathBuf::from(args.required("source").map_err(io::Error::other)?);
    let source = std::fs::read_to_string(&source_path)?;
    let chapter = args.options.get("chapter").cloned();
    let text = limit_benchmark_text(
        extract_chapter(&source, chapter.as_deref())?,
        args.usize("max-chars", 0).map_err(io::Error::other)?,
    );
    let system_dictionary = PathBuf::from(
        args.options
            .get("system-dict")
            .map_or("ipadic/system.dic", String::as_str),
    );
    let dictionary_directory = PathBuf::from(
        args.options
            .get("dict-dir")
            .map_or("data/dicts", String::as_str),
    );
    let engine_started = Instant::now();
    let engine = Engine::new(
        &system_dictionary,
        &dictionary_directory,
        args.required("profile").map_err(io::Error::other)?,
    )?;
    let engine_initialization_ms = engine_started.elapsed().as_millis();

    let progressive_started = Instant::now();
    let mut session =
        DocumentSession::new_progressive("benchmark".to_string(), text.to_string(), false);
    let mut first_batch_ms = 0;
    let mut first_patch_bytes = 0;
    let mut progressive_patch_bytes = 0;
    let mut first = true;
    let mut continuation = 0;
    while let Some(batch) = session.next_batch(if first {
        2_000
    } else if continuation == 0 {
        4_000
    } else {
        8_000
    }) {
        let (stable_tokens, tokens) = engine
            .analyze_document_batch_with_stable(&batch.source, session.document_readings())?;
        session.record_stable_batch(&batch, stable_tokens);
        let patch = session.append_analyzed_batch(session.revision, &batch, tokens)?;
        let payload = serde_json::to_vec(&patch)?;
        progressive_patch_bytes += payload.len();
        if first {
            first_batch_ms = progressive_started.elapsed().as_millis();
            first_patch_bytes = payload.len();
            first = false;
        } else {
            continuation += 1;
        }
    }
    let progressive_complete_ms = progressive_started.elapsed().as_millis();
    let reconstructed: String = session
        .tokens
        .iter()
        .map(|token| token.bunsetsu.surface.as_str())
        .collect();
    let expected = ruby::prepare_text(&text).text;

    let deferred_expression_started = Instant::now();
    let changed = engine.refresh_expression_annotations_changed(&mut session.tokens)?;
    let document_range = session.char_range();
    let deferred_expression_patch = session.apply_token_mutation(
        session.revision,
        "deferred_expression_completion",
        vec![StageInvalidation {
            stage: AnalysisStage::Expression,
            char_ranges: vec![document_range],
        }],
        |_| changed,
    )?;
    let deferred_expression_patch_bytes = serde_json::to_vec(&deferred_expression_patch)?.len();
    let deferred_expression_ms = deferred_expression_started.elapsed().as_millis();

    let mutation_started = Instant::now();
    let changed = engine.refresh_expression_annotations_changed(&mut session.tokens)?;
    let expression_changed_tokens = changed.len();
    let expression_patch = session.apply_token_mutation(
        session.revision,
        "benchmark_expression_refresh",
        vec![StageInvalidation {
            stage: AnalysisStage::Expression,
            char_ranges: vec![document_range],
        }],
        |_| changed,
    )?;
    let expression_patch_bytes = serde_json::to_vec(&expression_patch)?.len();
    let expression_mutation_ms = mutation_started.elapsed().as_millis();

    let cache_directory = args
        .options
        .get("cache-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "kotoclip-session-benchmark-cache-{}",
                std::process::id()
            ))
        });
    let temporary_cache = !args.options.contains_key("cache-dir");
    let cache = AnalysisCache::new(&cache_directory, &system_dictionary, &dictionary_directory)?;
    let cache_started = Instant::now();
    cache.store(&text, session.stable_tokens_for_cache())?;
    let cache_store_ms = cache_started.elapsed().as_millis();

    let warm_started = Instant::now();
    let cache_load_started = Instant::now();
    let mut cache_decode_started_ms = None;
    let mut cache_validate_started_ms = None;
    let cached = cache
        .load_with_progress(&text, |progress| match progress.phase {
            CacheLoadPhase::Decoding if progress.completed == 0 => {
                cache_decode_started_ms = Some(cache_load_started.elapsed().as_millis());
            }
            CacheLoadPhase::Validating if progress.completed == 0 => {
                cache_validate_started_ms = Some(cache_load_started.elapsed().as_millis());
            }
            _ => {}
        })
        .ok_or("刚写入的缓存无法读取")?;
    let cache_load_ms = cache_load_started.elapsed().as_millis();
    let warm_cache_read_ms = cache_decode_started_ms.unwrap_or(cache_load_ms);
    let warm_cache_decode_ms = cache_validate_started_ms
        .unwrap_or(cache_load_ms)
        .saturating_sub(warm_cache_read_ms);
    let warm_cache_validate_ms =
        cache_load_ms.saturating_sub(cache_validate_started_ms.unwrap_or(cache_load_ms));
    let warm_session_started = Instant::now();
    let mut warm_session =
        DocumentSession::new_progressive("warm".to_string(), text.to_string(), false);
    warm_session.set_cached_stable_tokens(cached);
    let warm_session_prepare_ms = warm_session_started.elapsed().as_millis();
    let mut warm_open_ms = 0;
    let mut warm_patch_bytes = 0;
    let mut warm_first_batch_select_ms = 0;
    let mut warm_first_state_restore_ms = 0;
    let mut warm_first_patch_ms = 0;
    let mut warm_first = true;
    let mut warm_continuation = 0;
    while let Some(batch) = warm_session.next_batch(if warm_first {
        2_000
    } else if warm_continuation == 0 {
        4_000
    } else {
        8_000
    }) {
        let batch_select_started = Instant::now();
        let stable = warm_session
            .take_cached_stable_tokens(&batch)
            .ok_or("缓存缺少对应批次 Token")?;
        let batch_select_ms = batch_select_started.elapsed().as_millis();
        let state_restore_started = Instant::now();
        let hydrated = engine.hydrate_stable_tokens_for_document_batch(stable)?;
        let state_restore_ms = state_restore_started.elapsed().as_millis();
        let patch_started = Instant::now();
        let patch = warm_session.append_analyzed_batch(warm_session.revision, &batch, hydrated)?;
        let patch_ms = patch_started.elapsed().as_millis();
        if warm_first {
            warm_patch_bytes = serde_json::to_vec(&patch)?.len();
            warm_open_ms = warm_started.elapsed().as_millis();
            warm_first_batch_select_ms = batch_select_ms;
            warm_first_state_restore_ms = state_restore_ms;
            warm_first_patch_ms = patch_ms;
            warm_first = false;
        } else {
            warm_continuation += 1;
        }
    }
    engine.refresh_expression_annotations_in_place(&mut warm_session.tokens)?;
    let warm_equals_progressive =
        serde_json::to_value(&warm_session.tokens)? == serde_json::to_value(&session.tokens)?;
    if temporary_cache {
        let _ = std::fs::remove_dir_all(cache_directory);
    }

    let report = SessionBenchmarkReport {
        source: source_path.display().to_string(),
        chapter,
        analyzed_characters: expected.chars().count(),
        engine_initialization_ms,
        first_batch_ms,
        first_patch_bytes,
        progressive_complete_ms,
        progressive_patch_bytes,
        deferred_expression_ms,
        deferred_expression_patch_bytes,
        expression_mutation_ms,
        expression_changed_tokens,
        expression_patch_bytes,
        cache_store_ms,
        warm_open_ms,
        warm_cache_read_ms,
        warm_cache_decode_ms,
        warm_cache_validate_ms,
        warm_session_prepare_ms,
        warm_first_batch_select_ms,
        warm_first_state_restore_ms,
        warm_first_patch_ms,
        warm_patch_bytes,
        tokens: session.tokens.len(),
        progressive_reconstruction_ok: reconstructed == expected,
        warm_equals_progressive,
    };
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(output) = args.options.get("json") {
        std::fs::write(output, &json)?;
    }
    println!("{json}");
    Ok(())
}

/// 使用当前管线动态生成全量基准，并以可复现随机顺序验证加载与规则增量结果。
fn incremental_consistency(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let source_path = PathBuf::from(args.required("source").map_err(io::Error::other)?);
    let source = std::fs::read_to_string(&source_path)?;
    let chapter = args.options.get("chapter").cloned();
    let text = extract_chapter(&source, chapter.as_deref())?;
    let seed = args.u64(
        "seed",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
    )?;
    let load_cases = args.usize("load-cases", 5)?;
    let rule_cases = args.usize("rule-cases", 5)?;
    let system_dictionary = PathBuf::from(
        args.options
            .get("system-dict")
            .map_or("ipadic/system.dic", String::as_str),
    );
    let dictionary_directory = PathBuf::from(
        args.options
            .get("dict-dir")
            .map_or("data/dicts", String::as_str),
    );
    let seed_profile = PathBuf::from(args.required("profile").map_err(io::Error::other)?);
    let temporary_directory = std::env::temp_dir().join(format!(
        "kotoclip-incremental-consistency-{}-{seed}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temporary_directory);
    std::fs::create_dir_all(&temporary_directory)?;
    let temporary_profile = temporary_directory.join("profile.sqlite");
    if seed_profile.is_file() {
        std::fs::copy(&seed_profile, &temporary_profile)?;
    }

    let result = (|| -> Result<IncrementalConsistencyReport, Box<dyn Error>> {
        let engine = Engine::new(
            &system_dictionary,
            &dictionary_directory,
            &temporary_profile,
        )?;
        let baseline = engine.analyze_text_with_exposure(text, false)?;
        let character_count = ruby::prepare_text(text).text.chars().count();
        let mut rng = DeterministicRng::new(seed);
        let targets = [512, 1_024, 2_000, 4_000, 8_000, 16_000, 100_000];

        for case_index in 0..load_cases {
            let mut session = DocumentSession::new_progressive(
                format!("random-load-{case_index}"),
                text.to_string(),
                false,
            );
            while !session.is_complete() {
                let position = rng.usize(character_count.max(1));
                let target = targets[rng.usize(targets.len())];
                let batch = session
                    .batch_for_range((position, (position + 1).min(character_count)), target)
                    .or_else(|| session.next_batch(target))
                    .ok_or("随机加载仍未完成但无法生成批次")?;
                let tokens =
                    engine.analyze_document_batch(&batch.source, session.document_readings())?;
                session.append_analyzed_batch(session.revision, &batch, tokens)?;
            }
            engine.refresh_expression_annotations_in_place(&mut session.tokens)?;
            require_token_equality(
                &baseline,
                &session.tokens,
                &format!("随机加载 case={case_index} seed={seed}"),
            )?;
        }

        let selectable_starts = baseline
            .windows(3)
            .enumerate()
            .filter(|(_, window)| {
                window.iter().all(|token| {
                    token.display_class == "content" && !token.bunsetsu.morphemes.is_empty()
                })
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if rule_cases > 0 && selectable_starts.is_empty() {
            return Err("完整文本中没有可用于随机规则的连续内容 Token".into());
        }

        let mut incremental = baseline.clone();
        for case_index in 0..rule_cases {
            let start = selectable_starts[rng.usize(selectable_starts.len())];
            let width = 2 + rng.usize(2);
            let selected = &incremental[start..start + width];
            let states = vec!["fixed".to_string(); width];
            let masks = selected
                .iter()
                .map(|token| vec![true; token.bunsetsu.morphemes.len()])
                .collect::<Vec<_>>();
            let rule = engine.add_configured_expression_rule(
                selected,
                Some(&format!("一致性探针-{seed}-{case_index}")),
                Some("随机增量差分验证临时规则"),
                &states,
                &masks,
                None,
                "grammar_construction",
                10_000 + case_index as i32,
                "annotate_only",
            )?;
            let before_add = incremental.clone();
            let changed_after_add =
                engine.refresh_expression_annotations_changed(&mut incremental)?;
            let full_after_add = engine.analyze_text_with_exposure(text, false)?;
            require_changed_indices(
                &before_add,
                &full_after_add,
                &changed_after_add,
                &format!(
                    "规则新增 Patch case={case_index} seed={seed} rule={}",
                    rule.id
                ),
            )?;
            require_token_equality(
                &full_after_add,
                &incremental,
                &format!("规则新增 case={case_index} seed={seed} rule={}", rule.id),
            )?;

            if !engine.delete_expression_rule(rule.id)? {
                return Err(format!("临时规则删除失败：{}", rule.id).into());
            }
            let before_delete = incremental.clone();
            let changed_after_delete =
                engine.refresh_expression_annotations_changed(&mut incremental)?;
            let full_after_delete = engine.analyze_text_with_exposure(text, false)?;
            require_changed_indices(
                &before_delete,
                &full_after_delete,
                &changed_after_delete,
                &format!(
                    "规则删除 Patch case={case_index} seed={seed} rule={}",
                    rule.id
                ),
            )?;
            require_token_equality(
                &full_after_delete,
                &incremental,
                &format!("规则删除 case={case_index} seed={seed} rule={}", rule.id),
            )?;
        }

        Ok(IncrementalConsistencyReport {
            source: source_path.display().to_string(),
            chapter,
            seed,
            load_cases,
            rule_cases,
            analyzed_characters: character_count,
            baseline_tokens: baseline.len(),
            randomized_loads_passed: load_cases,
            rule_additions_passed: rule_cases,
            rule_deletions_passed: rule_cases,
        })
    })();
    let _ = std::fs::remove_dir_all(&temporary_directory);
    let report = result?;
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(output) = args.options.get("json") {
        std::fs::write(output, &json)?;
    }
    println!("{json}");
    Ok(())
}

fn require_token_equality(
    expected: &[kotoclip_core::models::AnnotatedToken],
    actual: &[kotoclip_core::models::AnnotatedToken],
    context: &str,
) -> Result<(), Box<dyn Error>> {
    if expected.len() != actual.len() {
        return Err(format!(
            "{context} Token 数不一致：全量 {}，增量 {}",
            expected.len(),
            actual.len()
        )
        .into());
    }
    if let Some((index, (expected, actual))) =
        expected
            .iter()
            .zip(actual)
            .enumerate()
            .find(|(_, (expected, actual))| {
                serde_json::to_value(expected).ok() != serde_json::to_value(actual).ok()
            })
    {
        return Err(format!(
            "{context} 首个差异 Token={index} char_range={:?}\n全量={}\n增量={}",
            expected.bunsetsu.char_range,
            serde_json::to_string(expected)?,
            serde_json::to_string(actual)?
        )
        .into());
    }
    Ok(())
}

fn require_changed_indices(
    before: &[kotoclip_core::models::AnnotatedToken],
    expected_after: &[kotoclip_core::models::AnnotatedToken],
    actual_changed: &[usize],
    context: &str,
) -> Result<(), Box<dyn Error>> {
    let expected_changed = before
        .iter()
        .zip(expected_after)
        .enumerate()
        .filter_map(|(index, (before, after))| {
            (serde_json::to_value(before).ok() != serde_json::to_value(after).ok()).then_some(index)
        })
        .collect::<Vec<_>>();
    if expected_changed != actual_changed {
        return Err(format!(
            "{context} Patch Token 集不一致：全量差分 {:?}，增量报告 {:?}",
            expected_changed, actual_changed
        )
        .into());
    }
    Ok(())
}

fn print_nbest(pipeline: &Pipeline, text: &str, top_n: usize) {
    let candidates = pipeline.nbest_morphemes(text, top_n);
    let best = candidates
        .first()
        .map_or(0, |candidate| candidate.total_cost);
    for (index, candidate) in candidates.iter().enumerate() {
        let segmentation = candidate
            .morphemes
            .iter()
            .map(|morpheme| {
                format!(
                    "{}/{}:{}",
                    morpheme.surface, morpheme.base_form, morpheme.pos.major
                )
            })
            .collect::<Vec<_>>()
            .join("｜");
        println!(
            "#{:<2} cost={:<8} delta={:<8} {}",
            index + 1,
            candidate.total_cost,
            candidate.total_cost.saturating_sub(best),
            segmentation,
        );
    }
}

fn nbest(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let top_n = args.usize("top-n", 5).map_err(io::Error::other)?.max(1);
    print_nbest(&pipeline(args)?, &text, top_n);
    Ok(())
}

fn nbest_repl(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let pipeline = pipeline(args)?;
    let top_n = args.usize("top-n", 5).map_err(io::Error::other)?.max(1);
    println!("Vibrato lattice N-best 交互模式；直接输入日文，quit 退出。候选数：{top_n}");
    loop {
        print!("nbest> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if matches!(line, "quit" | "exit" | "q") {
            break;
        }
        if !line.is_empty() {
            print_nbest(&pipeline, line, top_n);
        }
    }
    Ok(())
}

fn nbest_rank(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let engine = engine(args)?;
    let tokens = engine.analyze_text_with_exposure(&text, false)?;
    for (index, token) in tokens.iter().enumerate() {
        println!(
            "[{index}] {} / {}",
            token.bunsetsu.surface, token.bunsetsu.head_word.base_form
        );
    }
    let index = args.usize("token", 0).map_err(io::Error::other)?;
    let token = tokens
        .get(index)
        .ok_or_else(|| format!("token 索引 {index} 超出范围"))?;
    let top_n = args.usize("top-n", 5).map_err(io::Error::other)?.max(1);
    println!(
        "{}",
        serde_json::to_string_pretty(&engine.get_candidates(token, top_n))?
    );
    Ok(())
}

fn nbest_choose(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let engine = engine(args)?;
    let tokens = engine.analyze_text_with_exposure(&text, false)?;
    let token_index = args.usize("token", 0).map_err(io::Error::other)?;
    let token = tokens
        .get(token_index)
        .ok_or_else(|| format!("token 索引 {token_index} 超出范围"))?;
    let pool = args.usize("top-n", 5).map_err(io::Error::other)?.max(1);
    let candidates = engine.get_candidates(token, pool);
    let candidate_index = args
        .usize("candidate", 1)
        .map_err(io::Error::other)?
        .saturating_sub(1);
    let candidate = candidates
        .get(candidate_index)
        .ok_or_else(|| format!("候选索引 {} 超出范围", candidate_index + 1))?;
    engine.choose_segmentation(token, candidate)?;
    println!(
        "已保存：{} -> {} (V{}, cost={})",
        token.bunsetsu.surface,
        candidate
            .tokens
            .iter()
            .map(|item| item.bunsetsu.surface.as_str())
            .collect::<Vec<_>>()
            .join("｜"),
        candidate.vibrato_rank,
        candidate.total_cost,
    );
    Ok(())
}

fn nbest_choices(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let engine = engine(args)?;
    if let Some(surface) = args.options.get("delete") {
        println!(
            "删除 {surface}：{}",
            engine.delete_segmentation_choice(surface)?
        );
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&engine.get_segmentation_choices()?)?
    );
    Ok(())
}

fn expression_list(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    println!(
        "{}",
        serde_json::to_string_pretty(&engine(args)?.get_expression_rules()?)?
    );
    Ok(())
}

fn print_expression_tokens(tokens: &[kotoclip_core::models::AnnotatedToken]) {
    for (index, token) in tokens.iter().enumerate() {
        let signature = token
            .bunsetsu
            .morphemes
            .iter()
            .filter(|morpheme| !morpheme.surface.trim().is_empty())
            .map(|morpheme| {
                let lemma = if morpheme.base_form == "*" || morpheme.base_form.is_empty() {
                    &morpheme.surface
                } else {
                    &morpheme.base_form
                };
                format!("{lemma}/{}", morpheme.pos.major)
            })
            .collect::<Vec<_>>()
            .join("+");
        let matches = token
            .expressions
            .iter()
            .map(|expression| format!("{}:{}", expression.label, expression.position))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "[{index:>3}] {:<18} {:<42} {}",
            token.bunsetsu.surface,
            signature,
            if matches.is_empty() { "" } else { &matches },
        );
    }
}

fn expression_preview(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let tokens = engine(args)?.analyze_text_with_exposure(&text, false)?;
    print_expression_tokens(&tokens);
    Ok(())
}

fn expression_add(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let engine = engine(args)?;
    let tokens = engine.analyze_text_with_exposure(&text, false)?;
    let start = args.usize("start-token", 0).map_err(io::Error::other)?;
    let end = args
        .usize("end-token", start + 1)
        .map_err(io::Error::other)?;
    if start >= end || end >= tokens.len() {
        return Err(format!(
            "无效 token 范围：{start}..={end}，当前共 {} 个 token",
            tokens.len()
        )
        .into());
    }
    let slot_indices = parse_index_list(args.options.get("slots").map(String::as_str))?;
    let bunsetsu_states: Vec<String> = (start..=end)
        .map(|idx| {
            if slot_indices.contains(&(idx - start)) {
                "slot".to_string()
            } else {
                "fixed".to_string()
            }
        })
        .collect();
    let rule = engine.add_expression_rule(
        &tokens[start..=end],
        args.options.get("label").map(String::as_str),
        args.options.get("description").map(String::as_str),
        &bunsetsu_states,
        &[],
        None,
    )?;
    println!("{}", serde_json::to_string_pretty(&rule)?);
    Ok(())
}

#[derive(Serialize)]
struct ScanItem {
    match_id: String,
    status: String,
    rule_id: String,
    label: String,
    origin: String,
    description: String,
    surface: String,
    token_range: (usize, usize),
    char_range: (usize, usize),
    matched_ranges: Vec<(usize, usize)>,
    captures: Vec<kotoclip_core::models::ExpressionCandidateCapture>,
    evidence: Vec<String>,
    counter_evidence: Vec<String>,
    rejection_reason: Option<String>,
    entry_key: Option<String>,
    context: String,
}

fn render_matched_context(text: &str, ranges: &[(usize, usize)], padding: usize) -> String {
    if ranges.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let start = ranges
        .iter()
        .map(|range| range.0)
        .min()
        .unwrap_or(0)
        .saturating_sub(padding);
    let end = ranges
        .iter()
        .map(|range| range.1)
        .max()
        .unwrap_or(0)
        .saturating_add(padding)
        .min(chars.len());
    let mut output = String::new();
    for index in start..end {
        if ranges.iter().any(|range| range.0 == index) {
            output.push('【');
        }
        output.push(chars[index]);
        if ranges.iter().any(|range| range.1 == index + 1) {
            output.push('】');
        }
    }
    output.replace(['\n', '\r'], " ")
}

fn expression_scan(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let tokens = engine(args)?.analyze_text_with_exposure(&text, false)?;
    let analyzed_text: String = tokens
        .iter()
        .map(|token| token.bunsetsu.surface.as_str())
        .collect();
    let mut shown = HashSet::new();
    let mut count = 0;
    let mut items = Vec::new();
    let write_json = args.options.get("json");
    let include_pending = args.flags.contains("include-pending");
    let include_rejected = args.flags.contains("include-rejected");
    let accepted_candidates: HashMap<_, _> =
        kotoclip_core::pipeline::expressions::builtin_expression_candidates(&tokens)
            .into_iter()
            .chain(kotoclip_core::pipeline::expressions::correlative_expression_candidates(&tokens))
            .map(|candidate| (candidate.candidate_id.clone(), candidate))
            .collect();

    for token in &tokens {
        for expression in token
            .expressions
            .iter()
            .filter(|item| item.position == "start")
        {
            if !shown.insert(expression.match_id.clone()) {
                continue;
            }
            let ranges = if expression.matched_ranges.is_empty() {
                vec![expression.char_range]
            } else {
                expression.matched_ranges.clone()
            };
            let clean_context = render_matched_context(&analyzed_text, &ranges, 24);

            if write_json.is_some() {
                let structured = accepted_candidates.get(&expression.match_id);
                items.push(ScanItem {
                    match_id: expression.match_id.clone(),
                    status: "accepted".to_string(),
                    rule_id: structured
                        .map(|candidate| candidate.rule_id.clone())
                        .unwrap_or_else(|| expression.rule_id.to_string()),
                    label: expression.label.clone(),
                    origin: expression.origin.clone(),
                    description: expression.description.clone(),
                    surface: expression.surface.clone(),
                    token_range: expression.token_range,
                    char_range: expression.char_range,
                    matched_ranges: ranges,
                    captures: structured
                        .map(|candidate| candidate.captures.clone())
                        .unwrap_or_default(),
                    evidence: structured
                        .map(|candidate| candidate.evidence.clone())
                        .unwrap_or_else(|| vec![format!("{}_rule_match", expression.origin)]),
                    counter_evidence: structured
                        .map(|candidate| candidate.counter_evidence.clone())
                        .unwrap_or_default(),
                    rejection_reason: None,
                    entry_key: structured.and_then(|candidate| candidate.entry_key.clone()),
                    context: clean_context,
                });
            } else {
                println!(
                    "[{}] {}\n  范围: token {}..{} / char {}..{}\n  含义: {}\n  上下文: {}\n",
                    expression.origin,
                    expression.label,
                    expression.token_range.0,
                    expression.token_range.1,
                    expression.char_range.0,
                    expression.char_range.1,
                    expression.description,
                    clean_context,
                );
            }
            count += 1;
        }
    }

    if include_pending || include_rejected {
        let dictionary = dictionary(args)?;
        for candidate in kotoclip_core::pipeline::expressions::dictionary_expression_candidates(
            &tokens,
            &dictionary,
        ) {
            let status = match candidate.status {
                kotoclip_core::models::ExpressionCandidateStatus::Accepted => "accepted",
                kotoclip_core::models::ExpressionCandidateStatus::Pending => "pending",
                kotoclip_core::models::ExpressionCandidateStatus::Rejected => "rejected",
            };
            if status == "pending" && !include_pending || status == "rejected" && !include_rejected
            {
                continue;
            }
            let context = render_matched_context(&analyzed_text, &candidate.matched_ranges, 24);
            items.push(ScanItem {
                match_id: candidate.candidate_id,
                status: status.to_string(),
                rule_id: candidate.rule_id,
                label: candidate.label,
                origin: candidate.origin,
                description: candidate.description,
                surface: candidate.surface,
                token_range: candidate.covered_token_range,
                char_range: candidate.char_range,
                matched_ranges: candidate.matched_ranges,
                captures: candidate.captures,
                evidence: candidate.evidence,
                counter_evidence: candidate.counter_evidence,
                rejection_reason: candidate.rejection_reason,
                entry_key: candidate.entry_key,
                context,
            });
        }
        if include_rejected {
            for candidate in
                kotoclip_core::pipeline::expressions::rejected_builtin_candidates(&tokens)
            {
                let context = render_matched_context(&analyzed_text, &candidate.matched_ranges, 24);
                items.push(ScanItem {
                    match_id: candidate.candidate_id,
                    status: "rejected".to_string(),
                    rule_id: candidate.rule_id,
                    label: candidate.label,
                    origin: candidate.origin,
                    description: candidate.description,
                    surface: candidate.surface,
                    token_range: candidate.covered_token_range,
                    char_range: candidate.char_range,
                    matched_ranges: candidate.matched_ranges,
                    captures: candidate.captures,
                    evidence: candidate.evidence,
                    counter_evidence: candidate.counter_evidence,
                    rejection_reason: candidate.rejection_reason,
                    entry_key: candidate.entry_key,
                    context,
                });
            }
        }
    }

    if let Some(json_path) = write_json {
        let json_data = serde_json::to_string_pretty(&items)?;
        std::fs::write(json_path, &json_data)?;
    }

    println!("共发现 {count} 个跨文节表达命中。");
    Ok(())
}

#[derive(Serialize)]
struct WordFormationScanItem {
    formation: kotoclip_core::models::WordFormationAnnotation,
    output_pos: kotoclip_core::models::PosTag,
    morpheme_signature: Vec<String>,
}

#[derive(Serialize)]
struct WordFormationRejectedItem {
    rule_id: String,
    morpheme_range: (usize, usize),
    char_range: (usize, usize),
    reason: String,
    morpheme_signature: Vec<String>,
}

#[derive(Serialize)]
struct WordFormationScanReport {
    schema_version: u32,
    accepted_count: usize,
    rejected_count: usize,
    conflict_count: usize,
    items: Vec<WordFormationScanItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rejected: Vec<WordFormationRejectedItem>,
}

fn word_formation_scan(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    // 保持与其他研究命令相同的临时 profile 调用契约，审计本身不会写入它。
    args.required("profile").map_err(io::Error::other)?;
    let text = read_text_selection(args)?;
    let pipeline = pipeline(args)?;
    let include_rejected = args.flags.contains("include-rejected");
    let mut items = Vec::new();
    let mut rejected = Vec::new();
    let mut rejected_count = 0;
    let mut conflict_count = 0;
    for segment in pipeline.inspect_word_formations(&text) {
        conflict_count += segment.result.conflicts;
        for formation in segment.result.accepted {
            let signature = segment.morphemes
                [formation.morpheme_range.0..formation.morpheme_range.1]
                .iter()
                .map(|morpheme| format!("{}/{}", morpheme.base_form, morpheme.pos.major))
                .collect();
            items.push(WordFormationScanItem {
                formation: formation.annotation,
                output_pos: formation.output_pos,
                morpheme_signature: signature,
            });
        }
        if include_rejected {
            rejected_count += segment.result.rejected.len();
            for item in segment.result.rejected {
                let end = item.morpheme_range.1.min(segment.morphemes.len());
                let signature = segment.morphemes[item.morpheme_range.0.min(end)..end]
                    .iter()
                    .map(|morpheme| format!("{}/{}", morpheme.base_form, morpheme.pos.major))
                    .collect();
                let char_range = if item.morpheme_range.0 < end {
                    (
                        segment.morphemes[item.morpheme_range.0].char_range.0,
                        segment.morphemes[end - 1].char_range.1,
                    )
                } else {
                    (0, 0)
                };
                rejected.push(WordFormationRejectedItem {
                    rule_id: item.rule_id,
                    morpheme_range: item.morpheme_range,
                    char_range,
                    reason: item.reason,
                    morpheme_signature: signature,
                });
            }
        }
    }
    let accepted_count = items.len();
    if let Some(path) = args.options.get("json") {
        let report = WordFormationScanReport {
            schema_version: 2,
            accepted_count,
            rejected_count,
            conflict_count,
            items,
            rejected,
        };
        std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    }
    println!("构词审计：接受 {accepted_count}，拒绝 {rejected_count}，冲突 {conflict_count}。");
    Ok(())
}

#[derive(Serialize)]
struct LexicalScanItem {
    candidate: kotoclip_core::models::DictionaryLexicalCandidate,
    morpheme_signature: Vec<String>,
}

#[derive(Serialize)]
struct LexicalScanReport {
    schema_version: u32,
    accepted_count: usize,
    pending_count: usize,
    rejected_count: usize,
    conflict_count: usize,
    reconstruction_ok: bool,
    range_integrity_ok: bool,
    items: Vec<LexicalScanItem>,
}

fn lexical_unit_scan(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    args.required("profile").map_err(io::Error::other)?;
    let text = read_text_selection(args)?;
    let pipeline = pipeline(args)?;
    let dictionary = dictionary(args)?;
    let include_pending = args.flags.contains("include-pending");
    let include_rejected = args.flags.contains("include-rejected");
    let mut accepted_count = 0;
    let mut pending_count = 0;
    let mut rejected_count = 0;
    let mut conflict_count = 0;
    let mut items = Vec::new();
    for segment in pipeline.inspect_dictionary_lexical_units(&text, &dictionary) {
        conflict_count += segment.result.conflicts;
        for candidate in segment.result.candidates {
            match candidate.status {
                kotoclip_core::models::LexicalCandidateStatus::Accepted => accepted_count += 1,
                kotoclip_core::models::LexicalCandidateStatus::Pending => pending_count += 1,
                kotoclip_core::models::LexicalCandidateStatus::Rejected => rejected_count += 1,
            }
            let include = candidate.status
                == kotoclip_core::models::LexicalCandidateStatus::Accepted
                || (include_pending
                    && candidate.status == kotoclip_core::models::LexicalCandidateStatus::Pending)
                || (include_rejected
                    && candidate.status == kotoclip_core::models::LexicalCandidateStatus::Rejected);
            if !include {
                continue;
            }
            let (start, end) = candidate.morpheme_range;
            let morpheme_signature = segment.morphemes[start..end]
                .iter()
                .map(|item| {
                    format!(
                        "{}/{}/{}/{}",
                        item.surface, item.base_form, item.pos.major, item.pos.sub1
                    )
                })
                .collect();
            items.push(LexicalScanItem {
                candidate,
                morpheme_signature,
            });
        }
    }
    let tokens = pipeline.process_with_dictionary(&text, &[], &dictionary);
    let reconstructed: String = tokens
        .iter()
        .map(|token| token.bunsetsu.surface.as_str())
        .collect();
    let canonical = ruby::prepare_text(&text).text;
    let reconstruction_ok = reconstructed == canonical;
    let range_integrity_ok = ranges_are_valid(&tokens, canonical.chars().count());
    if let Some(path) = args.options.get("json") {
        let report = LexicalScanReport {
            schema_version: 1,
            accepted_count,
            pending_count,
            rejected_count,
            conflict_count,
            reconstruction_ok,
            range_integrity_ok,
            items,
        };
        std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    }
    println!("词汇整体审计：接受 {accepted_count}，待定 {pending_count}，拒绝 {rejected_count}，冲突 {conflict_count}。");
    Ok(())
}

fn bunsetsu_scan(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    args.required("profile").map_err(io::Error::other)?;
    let text = read_text_selection(args)?;
    let pipeline = pipeline(args)?;
    let dictionary = dictionary(args)?;
    let include_alternatives = args.flags.contains("include-alternatives");
    let mut reports = pipeline.inspect_bunsetsu_with_dictionary(&text, &dictionary);
    if !include_alternatives {
        for report in &mut reports {
            for boundary in &mut report.boundaries {
                boundary.alternatives.clear();
            }
        }
    }
    let bunsetsu_count: usize = reports.iter().map(|report| report.bunsetsus.len()).sum();
    let determined: usize = reports.iter().map(|report| report.boundaries.len()).sum();
    let unresolved: usize = reports
        .iter()
        .map(|report| report.unresolved_boundaries)
        .sum();
    if let Some(path) = args.options.get("json") {
        std::fs::write(path, serde_json::to_string_pretty(&reports)?)?;
    }
    println!("文节审计：文节 {bunsetsu_count}，确定边界 {determined}，未决边界 {unresolved}。");
    Ok(())
}

#[derive(Serialize)]
struct VerifyItem {
    id: usize,
    match_id: String,
    label: String,
    origin: String,
    description: String,
    surface: String,
    token_range: (usize, usize),
    char_range: (usize, usize),
    matched_ranges: Vec<(usize, usize)>,
    context: String,
    decision: VerifyDecision,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
enum VerifyDecision {
    Pending,
    Verified,
    Incorrect,
}

fn expression_verify(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let engine = engine(args)?;
    let tokens = engine.analyze_text_with_exposure(&text, false)?;
    let analyzed_text: String = tokens
        .iter()
        .map(|token| token.bunsetsu.surface.as_str())
        .collect();

    let mut items = Vec::new();
    let mut seen = HashSet::new();

    for token in &tokens {
        for expression in &token.expressions {
            if expression.position == "start" && seen.insert(expression.match_id.clone()) {
                let matched_ranges = if expression.matched_ranges.is_empty() {
                    vec![expression.char_range]
                } else {
                    expression.matched_ranges.clone()
                };
                let context = render_matched_context(&analyzed_text, &matched_ranges, 36);

                items.push(VerifyItem {
                    id: 0,
                    match_id: expression.match_id.clone(),
                    label: expression.label.clone(),
                    origin: expression.origin.clone(),
                    description: expression.description.clone(),
                    surface: expression.surface.clone(),
                    token_range: expression.token_range,
                    char_range: expression.char_range,
                    matched_ranges,
                    context,
                    decision: VerifyDecision::Pending,
                });
            }
        }
    }

    // Sort items by token range start index to process sequentially
    items.sort_by_key(|item| item.token_range.0);

    for (idx, item) in items.iter_mut().enumerate() {
        item.id = idx + 1;
    }

    if items.is_empty() {
        println!("未检测到任何跨文节聚合项目。");
        return Ok(());
    }

    let page_size = 5;
    let total_pages = (items.len() + page_size - 1) / page_size;
    let mut current_page = 0;

    println!(
        "开始交互式跨文节聚合项目验证。总共 {} 个项目，共 {} 页。",
        items.len(),
        total_pages
    );
    println!("输入指令：");
    println!("  y / all y     一键确认当前页所有待验证项为 Verified");
    println!("  <id> y        确认指定序号的项目为 Verified");
    println!("  <id> n        标记指定序号的项目为 Incorrect (分词/范围有误)");
    println!("  <id> d        查看指定序号项目的底层分词与文节详情");
    println!("  n / next      下一页");
    println!("  p / prev      上一页");
    println!("  q / quit      结束验证并保存报告");

    loop {
        let start_idx = current_page * page_size;
        let end_idx = (start_idx + page_size).min(items.len());

        println!("\n=== 第 {} / {} 页 ===", current_page + 1, total_pages);
        for item in &items[start_idx..end_idx] {
            let status_str = match item.decision {
                VerifyDecision::Pending => "\x1b[33m[Pending]\x1b[0m",
                VerifyDecision::Verified => "\x1b[32m[Verified]\x1b[0m",
                VerifyDecision::Incorrect => "\x1b[31m[Incorrect]\x1b[0m",
            };
            println!(
                "[{}] {} {} ({}) - {}\n    上下文: {}",
                item.id, status_str, item.label, item.origin, item.description, item.context
            );
        }

        print!("\n(y/all y/n/p/q/<id> y/n/d) verify> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line == "q" || line == "quit" {
            break;
        } else if line == "n" || line == "next" {
            if current_page + 1 < total_pages {
                current_page += 1;
            } else {
                println!("已经是最后一页。");
            }
        } else if line == "p" || line == "prev" {
            if current_page > 0 {
                current_page -= 1;
            } else {
                println!("已经是第一页。");
            }
        } else if line == "y" || line == "all y" {
            for item in &mut items[start_idx..end_idx] {
                if item.decision == VerifyDecision::Pending {
                    item.decision = VerifyDecision::Verified;
                }
            }
            println!("已将当前页所有待验证项标记为 Verified。");
            if current_page + 1 < total_pages {
                current_page += 1;
            }
        } else {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 {
                if let Ok(id) = parts[0].parse::<usize>() {
                    if id > 0 && id <= items.len() {
                        let op = parts[1];
                        let item_idx = id - 1;
                        match op {
                            "y" => {
                                items[item_idx].decision = VerifyDecision::Verified;
                                println!("已标记项目 [{}] 为 Verified。", id);
                            }
                            "n" => {
                                items[item_idx].decision = VerifyDecision::Incorrect;
                                println!("已标记项目 [{}] 为 Incorrect。", id);
                            }
                            "d" => {
                                let item = &items[item_idx];
                                println!("\n--------------------------------------------------------------------------------");
                                println!("底层分词与文节结构 (项目 [{}]):", item.id);
                                for token_idx in item.token_range.0..item.token_range.1 {
                                    let token = &tokens[token_idx];
                                    println!(
                                        "  文节 [{}] (表层: {}):",
                                        token_idx, token.bunsetsu.surface
                                    );
                                    for (m_idx, morpheme) in
                                        token.bunsetsu.morphemes.iter().enumerate()
                                    {
                                        println!(
                                            "    语素 [{}] '{}' (原形: '{}', 词性: {}-{}-{}-{})",
                                            m_idx,
                                            morpheme.surface,
                                            morpheme.base_form,
                                            morpheme.pos.major,
                                            morpheme.pos.sub1,
                                            morpheme.pos.sub2,
                                            morpheme.pos.sub3
                                        );
                                    }
                                }
                                println!("--------------------------------------------------------------------------------");
                            }
                            _ => {
                                println!("未知操作。请输入 y, n 或 d。例如：1 d");
                            }
                        }
                    } else {
                        println!("项目序号 {} 超出范围 (1..={})", id, items.len());
                    }
                } else {
                    println!("无法解析序号。请输入类似 1 y 或 1 d 的指令。");
                }
            } else {
                println!("无法识别指令。请输入 y, all y, next, prev, quit 或 <id> y/n/d");
            }
        }
    }

    let total = items.len();
    let verified = items
        .iter()
        .filter(|i| i.decision == VerifyDecision::Verified)
        .count();
    let incorrect = items
        .iter()
        .filter(|i| i.decision == VerifyDecision::Incorrect)
        .count();
    let pending = items
        .iter()
        .filter(|i| i.decision == VerifyDecision::Pending)
        .count();
    let pass_rate = if total - pending > 0 {
        (verified as f64) / ((total - pending) as f64) * 100.0
    } else {
        0.0
    };

    println!("\n================================================================================");
    println!("验证报告总结:");
    println!("- 总计项目数: {}", total);
    println!("- 确认无误 (Verified): {}", verified);
    println!("- 分词/范围有误 (Incorrect): {}", incorrect);
    println!("- 未处理 (Pending): {}", pending);
    if total - pending > 0 {
        println!("- 确认通过率 (Verified / Audited): {:.2}%", pass_rate);
    }

    if incorrect > 0 {
        println!("\n有误项目详情 (Incorrect):");
        for item in items
            .iter()
            .filter(|i| i.decision == VerifyDecision::Incorrect)
        {
            println!(
                "  [{}] {} ({}) - {}",
                item.id, item.label, item.origin, item.surface
            );
            println!("    上下文: {}", item.context);
        }
    }
    println!("================================================================================");

    if let Some(json_path) = args.options.get("json") {
        #[derive(Serialize)]
        struct VerifyReport {
            total: usize,
            verified: usize,
            incorrect: usize,
            pending: usize,
            pass_rate: f64,
            items: Vec<VerifyItem>,
        }

        let report = VerifyReport {
            total,
            verified,
            incorrect,
            pending,
            pass_rate,
            items,
        };

        let json_data = serde_json::to_string_pretty(&report)?;
        std::fs::write(json_path, &json_data)?;
        println!("完整验证报告已写入 JSON 文件：{}", json_path);
    }

    Ok(())
}

fn expression_repl(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let engine = engine(args)?;
    let mut current_text = String::new();
    let mut current_tokens = Vec::new();
    println!("跨文节表达交互模式");
    println!(
        "命令：analyze 文本；select 起点 终点 槽位(-或0,1) [标签]；rules；delete ID；show；quit"
    );
    loop {
        print!("expr> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if matches!(line, "quit" | "exit" | "q") {
            break;
        }
        if let Some(text) = line.strip_prefix("analyze ") {
            current_text = text.to_string();
            current_tokens = engine.analyze_text_with_exposure(&current_text, false)?;
            print_expression_tokens(&current_tokens);
        } else if line == "show" {
            print_expression_tokens(&current_tokens);
        } else if line == "rules" {
            println!(
                "{}",
                serde_json::to_string_pretty(&engine.get_expression_rules()?)?
            );
        } else if let Some(value) = line.strip_prefix("delete ") {
            let id: i64 = value.trim().parse()?;
            println!("删除规则 {id}：{}", engine.delete_expression_rule(id)?);
            if !current_text.is_empty() {
                current_tokens = engine.analyze_text_with_exposure(&current_text, false)?;
            }
        } else if let Some(value) = line.strip_prefix("select ") {
            let mut parts = value.splitn(4, ' ');
            let start: usize = parts.next().ok_or("缺少起点")?.parse()?;
            let end: usize = parts.next().ok_or("缺少终点")?.parse()?;
            let slots = parts.next().ok_or("缺少槽位；无槽位请输入 -")?;
            let slot_indices = parse_index_list((slots != "-").then_some(slots))?;
            let label = parts.next();
            if start >= end || end >= current_tokens.len() {
                println!(
                    "范围无效。请先 analyze，并选择至少两个 token。当前共 {} 个。",
                    current_tokens.len()
                );
                continue;
            }
            let bunsetsu_states: Vec<String> = (start..=end)
                .map(|idx| {
                    if slot_indices.contains(&(idx - start)) {
                        "slot".to_string()
                    } else {
                        "fixed".to_string()
                    }
                })
                .collect();
            let rule = engine.add_expression_rule(
                &current_tokens[start..=end],
                label,
                None,
                &bunsetsu_states,
                &[],
                None,
            )?;
            println!("已保存 #{}：{}", rule.id, rule.label);
            current_tokens = engine.analyze_text_with_exposure(&current_text, false)?;
            print_expression_tokens(&current_tokens);
        } else if !line.is_empty() {
            println!("无法识别命令。请输入 analyze、select、rules、delete、show 或 quit。");
        }
    }
    Ok(())
}

fn parse_index_list(value: Option<&str>) -> Result<Vec<usize>, Box<dyn Error>> {
    value.map_or(Ok(Vec::new()), |value| {
        value
            .split(',')
            .filter(|part| !part.trim().is_empty())
            .map(|part| Ok(part.trim().parse::<usize>()?))
            .collect()
    })
}

fn repl(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let dictionary = dictionary(args)?;
    let pipeline = pipeline(args)?;
    println!("Kotoclip 交互模式。命令：lookup 词 [读音]；analyze 文本；stats；quit");
    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if matches!(line, "quit" | "exit" | "q") {
            break;
        }
        if line == "stats" {
            println!("{}", serde_json::to_string_pretty(&dictionary.stats())?);
        } else if let Some(rest) = line.strip_prefix("lookup ") {
            let mut parts = rest.split_whitespace();
            let word = parts.next().unwrap_or_default();
            let reading = parts.next();
            println!(
                "{}",
                serde_json::to_string_pretty(&dictionary.lookup(word, reading))?
            );
        } else if let Some(text) = line.strip_prefix("analyze ") {
            let tokens = pipeline.process_with_dictionary(text, &[], &dictionary);
            println!("{}", serde_json::to_string_pretty(&tokens)?);
        } else if !line.is_empty() {
            println!("无法识别命令。请输入 lookup、analyze、stats 或 quit。");
        }
    }
    Ok(())
}

fn read_text_argument(args: &CliArgs) -> Result<String, Box<dyn Error>> {
    if let Some(text) = args.options.get("text") {
        return Ok(text.clone());
    }
    if let Some(path) = args.options.get("source") {
        return Ok(std::fs::read_to_string(path)?);
    }
    Err("需要 --text 或 --source".into())
}

fn read_text_selection(args: &CliArgs) -> Result<String, Box<dyn Error>> {
    if let Some(text) = args.options.get("text") {
        return Ok(text.clone());
    }
    let path = args.required("source").map_err(io::Error::other)?;
    let source = std::fs::read_to_string(path)?;
    let selected = extract_chapter(&source, args.options.get("chapter").map(String::as_str))?;
    let lines: Vec<&str> = selected.lines().collect();
    let page_lines = args.usize("page-lines", 0).map_err(io::Error::other)?;
    let page = args.usize("page", 1).map_err(io::Error::other)?.max(1);
    let start = if page_lines > 0 {
        (page - 1).saturating_mul(page_lines)
    } else {
        args.usize("start-line", 1)
            .map_err(io::Error::other)?
            .saturating_sub(1)
    };
    let count = if page_lines > 0 {
        page_lines
    } else {
        args.usize("line-count", lines.len())
            .map_err(io::Error::other)?
    };
    if start >= lines.len() {
        return Err(format!("起始行 {} 超出文本范围 {}", start + 1, lines.len()).into());
    }
    Ok(lines[start..(start + count).min(lines.len())].join("\n"))
}

fn extract_chapter<'a>(source: &'a str, chapter: Option<&str>) -> Result<&'a str, Box<dyn Error>> {
    let Some(chapter) = chapter else {
        return Ok(source);
    };
    let requested = chapter.trim().trim_start_matches('#').trim();
    let mut line_start = 0;
    let mut body_start = None;
    for line in source.split_inclusive('\n') {
        let title = line.trim_end_matches(['\r', '\n']).trim();
        if title
            .strip_prefix("## ")
            .is_some_and(|value| value.trim() == requested)
        {
            body_start = Some(line_start + line.len());
            break;
        }
        line_start += line.len();
    }
    let body_start = body_start.ok_or_else(|| format!("找不到章节标题：{chapter}"))?;
    let body = &source[body_start..];
    let end = body.find("\n## ").unwrap_or(body.len());
    Ok(&body[..end])
}

/// 基准采样按字符截取，避免为定位复杂度问题重复运行整章。
fn limit_benchmark_text(text: &str, max_characters: usize) -> String {
    if max_characters == 0 {
        return text.to_string();
    }
    text.chars().take(max_characters).collect()
}

fn is_lexical(pos: &str) -> bool {
    matches!(
        pos,
        "名詞" | "動詞" | "形容詞" | "副詞" | "連体詞" | "接頭詞" | "感動詞" | "接続詞"
    )
}

fn ranges_are_valid(tokens: &[kotoclip_core::models::AnnotatedToken], char_count: usize) -> bool {
    let total_morphemes: usize = tokens
        .iter()
        .map(|token| token.bunsetsu.morphemes.len())
        .sum();
    tokens.iter().all(|token| {
        let bunsetsu = &token.bunsetsu;
        bunsetsu.char_range.0 <= bunsetsu.char_range.1
            && bunsetsu.char_range.1 <= char_count
            && bunsetsu.morphemes.iter().all(|morpheme| {
                morpheme.char_range.0 <= morpheme.char_range.1
                    && morpheme.char_range.1 <= char_count
            })
            && bunsetsu.grammar_tags.iter().all(|tag| {
                tag.morpheme_range.0 <= tag.morpheme_range.1
                    && tag.morpheme_range.1 <= total_morphemes
                    && tag.char_range.0 <= tag.char_range.1
                    && tag.char_range.1 <= char_count
            })
    })
}

fn print_help() {
    println!(
        r#"Kotoclip CLI

公共参数：
  --dict-dir PATH       SQLite 词典目录，默认 data/dicts
  --dict-source-dir PATH 词典源包目录，默认 data/dict-sources
  --system-dict PATH    Vibrato system.dic，默认 ipadic/system.dic
  --quiet               与 --json 同用时不在终端重复输出 JSON

命令：
  dict-info
  lookup --word WORD [--reading READING] [--full --timing]
  dict-bubble-html --word WORD [--reading READING] [--pos-major POS --pos-sub1 POS]
        [--output PATH] [--raw --json PATH --timing --no-open]
  analyze (--text TEXT | --source PATH)
  grammar-inspect (--text TEXT | --source PATH)
  grammar-scan (--text TEXT | --source PATH) [--chapter TITLE]
        [--include-pending --include-rejected --json PATH]
  grammar-residual (--text TEXT | --source PATH) [--chapter TITLE --json PATH]
  grammar-catalog [--query TEXT --family NAME --jlpt N --status STATUS]
        [--source-ref TEXT --json PATH]
  grammar-explain (--concept ID | --text TEXT | --source PATH)
        [--occurrence ID --json PATH]
  grammar-library-audit [--json PATH]
  grammar-audit [--cases PATH --json PATH]
  grammar-compare --before PATH --after PATH [--json PATH]
  grammar-review (--text TEXT | --source PATH) [--chapter TITLE]
        [--family NAME --batch N --batch-size 20..50 --issues-first]
        [--lines 12,35,81 --include-atoms --include-morphemes --group-residuals]
        [--sample-count N --json PATH]
  audit --source PATH [--chapter TITLE] [--page-lines N --page N]
        [--start-line N --line-count N --sample-every N]
        [--json PATH --min-coverage 0.10 --max-misses N]
  benchmark --source PATH --profile PATH [--chapter TITLE --max-chars N]
        [--no-record-exposure] [--json PATH]
  reader-benchmark --source PATH --profile PATH [--chapter TITLE --max-chars N]
        [--no-record-exposure] [--json PATH]
  session-benchmark --source PATH --profile PATH [--chapter TITLE --max-chars N]
        [--cache-dir PATH] [--json PATH]
  incremental-consistency --source PATH --profile PATH [--chapter TITLE]
        [--seed N --load-cases N --rule-cases N --json PATH]
  nbest (--text TEXT | --source PATH) [--top-n N]
  nbest-rank --profile PATH (--text TEXT | --source PATH)
        [--token N --top-n N]
  nbest-choose --profile PATH (--text TEXT | --source PATH)
        [--token N --candidate N --top-n N]
  nbest-choices --profile PATH [--delete SURFACE]
  nbest-repl [--top-n N]
  expression-list --profile PATH
  expression-preview --profile PATH (--text TEXT | --source PATH)
        [--chapter TITLE --page-lines N --page N]
  expression-scan --profile PATH (--text TEXT | --source PATH)
        [--chapter TITLE --page-lines N --page N] [--json PATH]
        [--include-pending --include-rejected]
  word-formation-scan --profile PATH (--text TEXT | --source PATH)
        [--chapter TITLE --page-lines N --page N] [--json PATH --include-rejected]
  lexical-unit-scan --profile PATH (--text TEXT | --source PATH)
        [--chapter TITLE --page-lines N --page N] [--json PATH]
        [--include-pending --include-rejected]
  bunsetsu-scan --profile PATH (--text TEXT | --source PATH)
        [--chapter TITLE --page-lines N --page N] [--json PATH --include-alternatives]
  expression-verify --profile PATH (--text TEXT | --source PATH)
        [--chapter TITLE --page-lines N --page N] [--json PATH]
  expression-add --profile PATH (--text TEXT | --source PATH)
        --start-token N --end-token N [--slots 0,1]
        [--label LABEL --description TEXT]
  expression-repl --profile PATH
  schema-audit [--json PATH]
  repl
"#
    );
}

#[cfg(test)]
mod tests {
    use super::extract_chapter;

    #[test]
    fn extracts_requested_markdown_chapter() {
        let source = "# 书\n第一話\n第二話\n## 第一話\n甲\n乙\n## 第二話\n丙";
        assert_eq!(extract_chapter(source, Some("第一話")).unwrap(), "甲\n乙");
        assert_eq!(extract_chapter(source, Some("## 第二話")).unwrap(), "丙");
    }
}
