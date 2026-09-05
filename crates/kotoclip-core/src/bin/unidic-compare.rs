use std::env;
use std::fs::File;
use std::io::BufReader;

use serde_json::{json, Value};
use vibrato::{Dictionary, Tokenizer};

fn tokenizer(path: &str) -> Tokenizer {
    let dict = Dictionary::read(BufReader::new(File::open(path).expect("无法打开字典")))
        .expect("无法读取 Vibrato 字典");
    Tokenizer::new(dict)
}

fn tokenize(tokenizer: &Tokenizer, text: &str) -> Vec<(String, usize, usize)> {
    let mut worker = tokenizer.new_worker();
    worker.reset_sentence(text);
    worker.tokenize();
    (0..worker.num_tokens())
        .map(|index| {
            let token = worker.token(index);
            let range = token.range_char();
            (token.surface().to_string(), range.start, range.end)
        })
        .collect()
}

fn target_boundaries(text: &str, bunsetsu: &[Value]) -> Vec<usize> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut boundaries = Vec::with_capacity(bunsetsu.len().saturating_sub(1));
    let mut offset = 0;
    for (index, value) in bunsetsu.iter().enumerate() {
        let surface = value.as_str().expect("目标文节必须是字符串");
        let length = surface.chars().count();
        assert_eq!(chars[offset..offset + length].iter().collect::<String>(), surface);
        offset += length;
        if index + 1 < bunsetsu.len() {
            boundaries.push(offset);
        }
    }
    assert_eq!(offset, chars.len());
    boundaries
}

fn boundary_match(tokens: &[(String, usize, usize)], expected: &[usize]) -> bool {
    let actual = tokens.iter().map(|(_, _, end)| *end).collect::<Vec<_>>();
    expected.iter().all(|boundary| actual.contains(boundary))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let old = args.next().ok_or("用法: unidic-compare OLD CWJ CSJ FIXTURE")?;
    let cwj = args.next().ok_or("缺少 CWJ 字典")?;
    let csj = args.next().ok_or("缺少 CSJ 字典")?;
    let fixture = args.next().ok_or("缺少 fixture")?;
    let cases: Vec<Value> = serde_json::from_reader(BufReader::new(File::open(fixture)?))?;
    let old_tokenizer = tokenizer(&old);
    let cwj_tokenizer = tokenizer(&cwj);
    let csj_tokenizer = tokenizer(&csj);
    let mut rows = Vec::new();
    for case in cases {
        let id = case["id"].as_str().unwrap_or_default();
        let text = case["text"].as_str().unwrap_or_default();
        let target = case["expected_target"]["bunsetsu"]
            .as_array()
            .map(|items| target_boundaries(text, items))
            .unwrap_or_default();
        let old_tokens = tokenize(&old_tokenizer, text);
        let cwj_tokens = tokenize(&cwj_tokenizer, text);
        let csj_tokens = tokenize(&csj_tokenizer, text);
        rows.push(json!({
            "id": id,
            "text": text,
            "target_boundaries": target,
            "ipadic": {"surfaces": old_tokens.iter().map(|(s,_,_)| s).collect::<Vec<_>>(), "target_boundary_match": boundary_match(&old_tokens, &target)},
            "unidic_cwj": {"surfaces": cwj_tokens.iter().map(|(s,_,_)| s).collect::<Vec<_>>(), "target_boundary_match": boundary_match(&cwj_tokens, &target)},
            "unidic_csj": {"surfaces": csj_tokens.iter().map(|(s,_,_)| s).collect::<Vec<_>>(), "target_boundary_match": boundary_match(&csj_tokens, &target)}
        }));
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
