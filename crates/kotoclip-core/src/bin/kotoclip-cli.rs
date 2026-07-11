use kotoclip_core::dictionary::lookup::DictionaryEngine;
use kotoclip_core::pipeline::{bunsetsu, ruby, Pipeline};
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
        self.options.get(key).map(String::as_str).ok_or_else(|| format!("缺少 --{key}"))
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
        "nbest" => nbest(&args),
        "nbest-rank" => nbest_rank(&args),
        "nbest-choose" => nbest_choose(&args),
        "nbest-choices" => nbest_choices(&args),
        "nbest-repl" => nbest_repl(&args),
        "expression-list" => expression_list(&args),
        "expression-preview" => expression_preview(&args),
        "expression-scan" => expression_scan(&args),
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
    Ok(DictionaryEngine::new(args.options.get("dict-dir").map_or("data/dicts", String::as_str))?)
}

fn pipeline(args: &CliArgs) -> Result<Pipeline, Box<dyn Error>> {
    Ok(Pipeline::new(args.options.get("system-dict").map_or("ipadic/system.dic", String::as_str))?)
}

fn engine(args: &CliArgs) -> Result<Engine, Box<dyn Error>> {
    Ok(Engine::new(
        args.options.get("system-dict").map_or("ipadic/system.dic", String::as_str),
        args.options.get("dict-dir").map_or("data/dicts", String::as_str),
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
        println!("[{}] {} / {} / {}\n{}\n", index + 1, entry.dict_name, entry.match_type, entry.headword, definition);
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
    let explicit_start = args.usize("start-line", 1).map_err(io::Error::other)?.max(1);
    let start = if page_lines > 0 { (page - 1) * page_lines + 1 } else { explicit_start };
    let default_count = if page_lines > 0 { page_lines } else { all_lines.len() };
    let line_count = args.usize("line-count", default_count).map_err(io::Error::other)?;
    let end_exclusive = start.saturating_sub(1).saturating_add(line_count).min(all_lines.len());
    let sample_every = args.usize("sample-every", 1).map_err(io::Error::other)?.max(1);

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

    for (offset, line) in all_lines[start.saturating_sub(1)..end_exclusive].iter().enumerate() {
        if offset % sample_every != 0 || line.trim().is_empty() { continue; }
        analyzed_lines += 1;
        let prepared = ruby::prepare_text(line);
        analyzed_characters += prepared.text.chars().count();
        let mut tokens = pipeline.process(line, &[]);
        for token in &mut tokens {
            bunsetsu::resolve_lexical_boundaries(std::slice::from_mut(&mut token.bunsetsu), |word| {
                dictionary.contains_exact(word)
            });
        }
        let reconstructed: String = tokens.iter().map(|token| token.bunsetsu.surface.as_str()).collect();
        if reconstructed == prepared.text { reconstruction_passes += 1; }
        if ranges_are_valid(&tokens, prepared.text.chars().count()) { integrity_passes += 1; }

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
                    let miss = misses.entry((head.base_form.clone(), head.reading.clone())).or_default();
                    miss.count += 1;
                    miss.surfaces.insert(head.surface.clone());
                }
            }
        }
    }

    let unmatched = lexical_occurrences.saturating_sub(headword_matches + reading_matches);
    let ratio = |passed: usize, total: usize| if total == 0 { 1.0 } else { passed as f64 / total as f64 };
    let mut missed_lexemes: Vec<MissedLexeme> = misses.into_iter().map(|((base_form, reading), aggregate)| {
        let mut surfaces: Vec<String> = aggregate.surfaces.into_iter().collect();
        surfaces.sort();
        MissedLexeme { base_form, reading, surfaces, occurrences: aggregate.count }
    }).collect();
    missed_lexemes.sort_by(|left, right| right.occurrences.cmp(&left.occurrences).then_with(|| left.base_form.cmp(&right.base_form)));
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
        return Err(format!("覆盖率 {:.2}% 低于门槛 {:.2}%", report.coverage_rate * 100.0, minimum * 100.0).into());
    }
    Ok(())
}

fn benchmark(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let source_path = PathBuf::from(args.required("source").map_err(io::Error::other)?);
    let source = std::fs::read_to_string(&source_path)?;
    let text = extract_chapter(&source, args.options.get("chapter").map(String::as_str))?;
    let profile = args.required("profile").map_err(io::Error::other)?;
    let engine = Engine::new(
        args.options.get("system-dict").map_or("ipadic/system.dic", String::as_str),
        args.options.get("dict-dir").map_or("data/dicts", String::as_str),
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
            if current_phase.as_ref().map_or(true, |(current, _)| current != &phase) {
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

fn print_nbest(pipeline: &Pipeline, text: &str, top_n: usize) {
    let candidates = pipeline.nbest_morphemes(text, top_n);
    let best = candidates.first().map_or(0, |candidate| candidate.total_cost);
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
        if io::stdin().read_line(&mut line)? == 0 { break; }
        let line = line.trim();
        if matches!(line, "quit" | "exit" | "q") { break; }
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
        println!("[{index}] {} / {}", token.bunsetsu.surface, token.bunsetsu.head_word.base_form);
    }
    let index = args.usize("token", 0).map_err(io::Error::other)?;
    let token = tokens.get(index).ok_or_else(|| format!("token 索引 {index} 超出范围"))?;
    let top_n = args.usize("top-n", 5).map_err(io::Error::other)?.max(1);
    println!("{}", serde_json::to_string_pretty(&engine.get_candidates(token, top_n))?);
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
    let candidate_index = args.usize("candidate", 1).map_err(io::Error::other)?.saturating_sub(1);
    let candidate = candidates
        .get(candidate_index)
        .ok_or_else(|| format!("候选索引 {} 超出范围", candidate_index + 1))?;
    engine.choose_segmentation(token, candidate)?;
    println!(
        "已保存：{} -> {} (V{}, cost={})",
        token.bunsetsu.surface,
        candidate.tokens.iter().map(|item| item.bunsetsu.surface.as_str()).collect::<Vec<_>>().join("｜"),
        candidate.vibrato_rank,
        candidate.total_cost,
    );
    Ok(())
}

fn nbest_choices(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let engine = engine(args)?;
    if let Some(surface) = args.options.get("delete") {
        println!("删除 {surface}：{}", engine.delete_segmentation_choice(surface)?);
    }
    println!("{}", serde_json::to_string_pretty(&engine.get_segmentation_choices()?)?);
    Ok(())
}

fn expression_list(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string_pretty(&engine(args)?.get_expression_rules()?)?);
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
    let end = args.usize("end-token", start + 1).map_err(io::Error::other)?;
    if start >= end || end >= tokens.len() {
        return Err(format!("无效 token 范围：{start}..={end}，当前共 {} 个 token", tokens.len()).into());
    }
    let slot_indices = parse_index_list(args.options.get("slots").map(String::as_str))?;
    let rule = engine.add_expression_rule(
        &tokens[start..=end],
        args.options.get("label").map(String::as_str),
        args.options.get("description").map(String::as_str),
        &slot_indices,
    )?;
    println!("{}", serde_json::to_string_pretty(&rule)?);
    Ok(())
}

fn expression_scan(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let text = read_text_selection(args)?;
    let tokens = engine(args)?.analyze_text_with_exposure(&text, false)?;
    let mut shown = HashSet::new();
    let mut count = 0;
    for (index, token) in tokens.iter().enumerate() {
        for expression in token.expressions.iter().filter(|item| item.position == "start") {
            if !shown.insert(expression.match_id.clone()) {
                continue;
            }
            let context_start = index.saturating_sub(2);
            let context_end = (expression.token_range.1 + 3).min(tokens.len());
            let context: String = tokens[context_start..context_end]
                .iter()
                .map(|item| item.bunsetsu.surface.as_str())
                .collect();
            println!(
                "[{}] {}\n  范围: token {}..{} / char {}..{}\n  含义: {}\n  上下文: {}\n",
                expression.origin,
                expression.label,
                expression.token_range.0,
                expression.token_range.1,
                expression.char_range.0,
                expression.char_range.1,
                expression.description,
                context.replace(['\n', '\r'], " "),
            );
            count += 1;
        }
    }
    println!("共发现 {count} 个跨文节表达命中。");
    Ok(())
}

fn expression_repl(args: &CliArgs) -> Result<(), Box<dyn Error>> {
    let engine = engine(args)?;
    let mut current_text = String::new();
    let mut current_tokens = Vec::new();
    println!("跨文节表达交互模式");
    println!("命令：analyze 文本；select 起点 终点 槽位(-或0,1) [标签]；rules；delete ID；show；quit");
    loop {
        print!("expr> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 { break; }
        let line = line.trim();
        if matches!(line, "quit" | "exit" | "q") { break; }
        if let Some(text) = line.strip_prefix("analyze ") {
            current_text = text.to_string();
            current_tokens = engine.analyze_text_with_exposure(&current_text, false)?;
            print_expression_tokens(&current_tokens);
        } else if line == "show" {
            print_expression_tokens(&current_tokens);
        } else if line == "rules" {
            println!("{}", serde_json::to_string_pretty(&engine.get_expression_rules()?)?);
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
                println!("范围无效。请先 analyze，并选择至少两个 token。当前共 {} 个。", current_tokens.len());
                continue;
            }
            let rule = engine.add_expression_rule(
                &current_tokens[start..=end],
                label,
                None,
                &slot_indices,
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
        if io::stdin().read_line(&mut line)? == 0 { break; }
        let line = line.trim();
        if matches!(line, "quit" | "exit" | "q") { break; }
        if line == "stats" {
            println!("{}", serde_json::to_string_pretty(&dictionary.stats())?);
        } else if let Some(rest) = line.strip_prefix("lookup ") {
            let mut parts = rest.split_whitespace();
            let word = parts.next().unwrap_or_default();
            let reading = parts.next();
            println!("{}", serde_json::to_string_pretty(&dictionary.lookup(word, reading))?);
        } else if let Some(text) = line.strip_prefix("analyze ") {
            let mut tokens = pipeline.process(text, &[]);
            for token in &mut tokens {
                bunsetsu::resolve_lexical_boundaries(std::slice::from_mut(&mut token.bunsetsu), |word| dictionary.contains_exact(word));
            }
            println!("{}", serde_json::to_string_pretty(&tokens)?);
        } else if !line.is_empty() {
            println!("无法识别命令。请输入 lookup、analyze、stats 或 quit。");
        }
    }
    Ok(())
}

fn read_text_argument(args: &CliArgs) -> Result<String, Box<dyn Error>> {
    if let Some(text) = args.options.get("text") { return Ok(text.clone()); }
    if let Some(path) = args.options.get("source") { return Ok(std::fs::read_to_string(path)?); }
    Err("需要 --text 或 --source".into())
}

fn read_text_selection(args: &CliArgs) -> Result<String, Box<dyn Error>> {
    if let Some(text) = args.options.get("text") {
        return Ok(text.clone());
    }
    let path = args.required("source").map_err(io::Error::other)?;
    let source = std::fs::read_to_string(path)?;
    let selected = extract_chapter(
        &source,
        args.options.get("chapter").map(String::as_str),
    )?;
    let lines: Vec<&str> = selected.lines().collect();
    let page_lines = args.usize("page-lines", 0).map_err(io::Error::other)?;
    let page = args.usize("page", 1).map_err(io::Error::other)?.max(1);
    let start = if page_lines > 0 {
        (page - 1).saturating_mul(page_lines)
    } else {
        args.usize("start-line", 1).map_err(io::Error::other)?.saturating_sub(1)
    };
    let count = if page_lines > 0 {
        page_lines
    } else {
        args.usize("line-count", lines.len()).map_err(io::Error::other)?
    };
    if start >= lines.len() {
        return Err(format!("起始行 {} 超出文本范围 {}", start + 1, lines.len()).into());
    }
    Ok(lines[start..(start + count).min(lines.len())].join("\n"))
}

fn extract_chapter<'a>(source: &'a str, chapter: Option<&str>) -> Result<&'a str, Box<dyn Error>> {
    let Some(chapter) = chapter else { return Ok(source) };
    let marker = source.find(chapter).ok_or_else(|| format!("找不到章节标题：{chapter}"))?;
    let body_start = source[marker..].find('\n').map_or(source.len(), |offset| marker + offset + 1);
    let body = &source[body_start..];
    let end = body.find("\n## ").unwrap_or(body.len());
    Ok(&body[..end])
}

fn is_lexical(pos: &str) -> bool {
    matches!(pos, "名詞" | "動詞" | "形容詞" | "副詞" | "連体詞" | "接頭詞" | "感動詞" | "接続詞")
}

fn ranges_are_valid(tokens: &[kotoclip_core::models::AnnotatedToken], char_count: usize) -> bool {
    let total_morphemes: usize = tokens.iter().map(|token| token.bunsetsu.morphemes.len()).sum();
    tokens.iter().all(|token| {
        let bunsetsu = &token.bunsetsu;
        bunsetsu.char_range.0 <= bunsetsu.char_range.1
            && bunsetsu.char_range.1 <= char_count
            && bunsetsu.morphemes.iter().all(|morpheme| {
                morpheme.char_range.0 <= morpheme.char_range.1 && morpheme.char_range.1 <= char_count
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
    println!(r#"Kotoclip CLI

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
        [--chapter TITLE --page-lines N --page N]
  expression-add --profile PATH (--text TEXT | --source PATH)
        --start-token N --end-token N [--slots 0,1]
        [--label LABEL --description TEXT]
  expression-repl --profile PATH
  repl
"#);
}

#[cfg(test)]
mod tests {
    use super::extract_chapter;

    #[test]
    fn extracts_requested_markdown_chapter() {
        let source = "# 书\n## 第一話\n甲\n乙\n## 第二話\n丙";
        assert_eq!(extract_chapter(source, Some("## 第一話")).unwrap(), "甲\n乙");
    }
}
