use kotoclip_core::dictionary::lookup::DictionaryEngine;
use kotoclip_core::pipeline::{bunsetsu, ruby, Pipeline};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;

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
