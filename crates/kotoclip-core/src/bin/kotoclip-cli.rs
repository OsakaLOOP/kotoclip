use kotoclip_core::dictionary::lookup::DictionaryEngine;
use kotoclip_core::pipeline::{bunsetsu, ruby, Pipeline};
use kotoclip_core::transport::CompactAnalysis;
use kotoclip_core::Engine;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Instant;

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
        "analyze" => analyze(&args),
        "audit" => audit(&args),
        "benchmark" => benchmark(&args),
        "reader-benchmark" => reader_benchmark(&args),
        "nbest" => nbest(&args),
        "nbest-rank" => nbest_rank(&args),
        "nbest-choose" => nbest_choose(&args),
        "nbest-choices" => nbest_choices(&args),
        "nbest-repl" => nbest_repl(&args),
        "expression-list" => expression_list(&args),
        "expression-preview" => expression_preview(&args),
        "expression-scan" => expression_scan(&args),
        "word-formation-scan" => word_formation_scan(&args),
        "bunsetsu-scan" => bunsetsu_scan(&args),
        "expression-verify" => expression_verify(&args),
        "expression-add" => expression_add(&args),
        "expression-repl" => expression_repl(&args),
        "repl" => repl(&args),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        _ => Err(format!("未知命令：{command}。运行 help 查看用法。").into()),
    }
}

fn dictionary(args: &CliArgs) -> Result<DictionaryEngine, Box<dyn Error>> {
    Ok(DictionaryEngine::new(
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
    Ok(Engine::new(
        args.options
            .get("system-dict")
            .map_or("ipadic/system.dic", String::as_str),
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
    let results = dictionary(args)?.lookup(word, reading);
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
    Ok(())
}

fn analyze(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_argument(args)?;
    let dictionary = dictionary(args)?;
    let pipeline = pipeline(args)?;
    let mut tokens = pipeline.process(&text, &[]);
    for token in &mut tokens {
        bunsetsu::resolve_lexical_boundaries(std::slice::from_mut(&mut token.bunsetsu), |word| {
            dictionary.contains_exact(word)
        });
    }
    println!("{}", serde_json::to_string_pretty(&tokens)?);
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
        let mut tokens = pipeline.process(line, &[]);
        for token in &mut tokens {
            bunsetsu::resolve_lexical_boundaries(
                std::slice::from_mut(&mut token.bunsetsu),
                |word| dictionary.contains_exact(word),
            );
        }
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
    let text = extract_chapter(&source, args.options.get("chapter").map(String::as_str))?;
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
        text,
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
    let text = extract_chapter(&source, chapter.as_deref())?;
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
        engine.analyze_text_profiled(text, !args.flags.contains("no-record-exposure"))?;
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
        analyzed_characters: ruby::prepare_text(text).text.chars().count(),
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
    label: String,
    origin: String,
    description: String,
    surface: String,
    token_range: (usize, usize),
    char_range: (usize, usize),
    context: String,
}

fn expression_scan(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let tokens = engine(args)?.analyze_text_with_exposure(&text, false)?;
    let mut shown = HashSet::new();
    let mut count = 0;
    let mut items = Vec::new();
    let write_json = args.options.get("json");

    for (index, token) in tokens.iter().enumerate() {
        for expression in token
            .expressions
            .iter()
            .filter(|item| item.position == "start")
        {
            if !shown.insert(expression.match_id.clone()) {
                continue;
            }
            let context_start = index.saturating_sub(2);
            let context_end = (expression.token_range.1 + 3).min(tokens.len());
            let context: String = tokens[context_start..context_end]
                .iter()
                .map(|item| item.bunsetsu.surface.as_str())
                .collect();
            let clean_context = context.replace(['\n', '\r'], " ");

            if write_json.is_some() {
                items.push(ScanItem {
                    match_id: expression.match_id.clone(),
                    label: expression.label.clone(),
                    origin: expression.origin.clone(),
                    description: expression.description.clone(),
                    surface: expression.surface.clone(),
                    token_range: expression.token_range,
                    char_range: expression.char_range,
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
            let signature = segment.morphemes[formation.morpheme_range.0..formation.morpheme_range.1]
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
            schema_version: 1,
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

fn bunsetsu_scan(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    args.required("profile").map_err(io::Error::other)?;
    let text = read_text_selection(args)?;
    let pipeline = pipeline(args)?;
    let include_alternatives = args.flags.contains("include-alternatives");
    let mut reports = pipeline.inspect_bunsetsu(&text);
    if !include_alternatives {
        for report in &mut reports {
            for boundary in &mut report.boundaries {
                boundary.alternatives.clear();
            }
        }
    }
    let bunsetsu_count: usize = reports.iter().map(|report| report.bunsetsus.len()).sum();
    let determined: usize = reports.iter().map(|report| report.boundaries.len()).sum();
    let unresolved: usize = reports.iter().map(|report| report.unresolved_boundaries).sum();
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

    let mut items = Vec::new();
    let mut seen = HashSet::new();

    for token in &tokens {
        for expression in &token.expressions {
            if expression.position == "start" && seen.insert(expression.match_id.clone()) {
                let context_start = expression.token_range.0.saturating_sub(6);
                let context_end = (expression.token_range.1 + 6).min(tokens.len());
                let left_context: String = tokens[context_start..expression.token_range.0]
                    .iter()
                    .map(|t| t.bunsetsu.surface.as_str())
                    .collect();
                let match_surface: String = tokens
                    [expression.token_range.0..expression.token_range.1]
                    .iter()
                    .map(|t| t.bunsetsu.surface.as_str())
                    .collect();
                let right_context: String = tokens[expression.token_range.1..context_end]
                    .iter()
                    .map(|t| t.bunsetsu.surface.as_str())
                    .collect();
                let context = format!(
                    "{}【{}】{}",
                    left_context.replace('\n', " "),
                    match_surface,
                    right_context.replace('\n', " ")
                );

                items.push(VerifyItem {
                    id: 0,
                    match_id: expression.match_id.clone(),
                    label: expression.label.clone(),
                    origin: expression.origin.clone(),
                    description: expression.description.clone(),
                    surface: expression.surface.clone(),
                    token_range: expression.token_range,
                    char_range: expression.char_range,
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
            let mut tokens = pipeline.process(text, &[]);
            for token in &mut tokens {
                bunsetsu::resolve_lexical_boundaries(
                    std::slice::from_mut(&mut token.bunsetsu),
                    |word| dictionary.contains_exact(word),
                );
            }
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
    let marker = source
        .find(chapter)
        .ok_or_else(|| format!("找不到章节标题：{chapter}"))?;
    let body_start = source[marker..]
        .find('\n')
        .map_or(source.len(), |offset| marker + offset + 1);
    let body = &source[body_start..];
    let end = body.find("\n## ").unwrap_or(body.len());
    Ok(&body[..end])
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
  --system-dict PATH    Vibrato system.dic，默认 ipadic/system.dic

命令：
  dict-info
  lookup --word WORD [--reading READING] [--full]
  analyze (--text TEXT | --source PATH)
  audit --source PATH [--chapter TITLE] [--page-lines N --page N]
        [--start-line N --line-count N --sample-every N]
        [--json PATH --min-coverage 0.10 --max-misses N]
  benchmark --source PATH --profile PATH [--chapter TITLE]
        [--no-record-exposure] [--json PATH]
  reader-benchmark --source PATH --profile PATH [--chapter TITLE]
        [--no-record-exposure] [--json PATH]
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
  word-formation-scan --profile PATH (--text TEXT | --source PATH)
        [--chapter TITLE --page-lines N --page N] [--json PATH --include-rejected]
  bunsetsu-scan --profile PATH (--text TEXT | --source PATH)
        [--chapter TITLE --page-lines N --page N] [--json PATH --include-alternatives]
  expression-verify --profile PATH (--text TEXT | --source PATH)
        [--chapter TITLE --page-lines N --page N] [--json PATH]
  expression-add --profile PATH (--text TEXT | --source PATH)
        --start-token N --end-token N [--slots 0,1]
        [--label LABEL --description TEXT]
  expression-repl --profile PATH
  repl
"#
    );
}

#[cfg(test)]
mod tests {
    use super::extract_chapter;

    #[test]
    fn extracts_requested_markdown_chapter() {
        let source = "# 书\n## 第一話\n甲\n乙\n## 第二話\n丙";
        assert_eq!(
            extract_chapter(source, Some("## 第一話")).unwrap(),
            "甲\n乙"
        );
    }
}
