use std::env;
use std::fs::File;
use std::io::BufReader;

use vibrato::{Dictionary, Tokenizer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let dictionary = args.next().ok_or("用法: unidic-inspect DICT TEXT")?;
    let text = args.collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Err("TEXT 不能为空".into());
    }
    let dict = Dictionary::read(BufReader::new(File::open(dictionary)?))?;
    let tokenizer = Tokenizer::new(dict);
    let mut worker = tokenizer.new_worker();
    worker.reset_sentence(&text);
    worker.tokenize();
    println!("surface\tchar_start\tchar_end\traw_feature");
    for index in 0..worker.num_tokens() {
        let token = worker.token(index);
        let range = token.range_char();
        println!(
            "{}\t{}\t{}\t{}",
            token.surface(),
            range.start,
            range.end,
            token.feature()
        );
    }
    Ok(())
}
