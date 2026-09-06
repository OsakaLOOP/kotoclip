use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

use serde_json::json;
use vibrato::{dictionary::LexType, Dictionary, Tokenizer};

fn run(path: &str, source: &str) -> serde_json::Value {
    let load_started = Instant::now();
    let dict = Dictionary::read(BufReader::new(File::open(path).expect("无法打开字典")))
        .expect("无法读取 Vibrato 字典");
    let tokenizer = Tokenizer::new(dict);
    let load_ms = load_started.elapsed().as_secs_f64() * 1000.0;
    let mut worker = tokenizer.new_worker();
    let started = Instant::now();
    let mut chars = 0usize;
    let mut tokens = 0usize;
    let mut unknown = 0usize;
    for line in BufReader::new(File::open(source).expect("无法打开文本")).lines() {
        let line = line.expect("无法读取文本");
        if line.is_empty() {
            continue;
        }
        chars += line.chars().count();
        worker.reset_sentence(&line);
        worker.tokenize();
        tokens += worker.num_tokens();
        for index in 0..worker.num_tokens() {
            if worker.token(index).lex_type() == LexType::Unknown {
                unknown += 1;
            }
        }
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    json!({
        "dictionary": path,
        "characters": chars,
        "tokens": tokens,
        "unknown_tokens": unknown,
        "unknown_ratio": if tokens == 0 { 0.0 } else { unknown as f64 / tokens as f64 },
        "dictionary_load_ms": load_ms,
        "elapsed_ms": elapsed_ms,
        "characters_per_second": if elapsed_ms == 0.0 { 0.0 } else { chars as f64 / (elapsed_ms / 1000.0) }
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let source = args.next().ok_or("用法: unidic-benchmark SOURCE IPADIC CWJ CSJ")?;
    let ipadic = args.next().ok_or("缺少 IPADIC 字典")?;
    let cwj = args.next().ok_or("缺少 CWJ 字典")?;
    let csj = args.next().ok_or("缺少 CSJ 字典")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "source": source,
            "results": [run(&ipadic, &source), run(&cwj, &source), run(&csj, &source)]
        }))?
    );
    Ok(())
}
