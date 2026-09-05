use std::env;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use vibrato::{mecab::generate_bigram_info, SystemDictionaryBuilder};

fn argument(name: &str) -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    while let Some(value) = args.next() {
        if value == name {
            return args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("缺少参数 {name}"));
        }
    }
    Err(format!("缺少参数 {name}"))
}

fn required(root: &Path, name: &str) -> Result<PathBuf, String> {
    let path = root.join(name);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("缺少 UniDic 构建输入: {}", path.display()))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = argument("--input")?;
    let output = argument("--output")?;
    let output = output.canonicalize().unwrap_or(output);
    let work = output.with_extension("vibrato-work");
    fs::create_dir_all(&work)?;

    let lex = required(&input, "lex.csv")?;
    let matrix = required(&input, "matrix.def")?;
    let char_def = required(&input, "char.def")?;
    let unk = required(&input, "unk.def")?;
    let feature = required(&input, "feature.def")?;
    let right_id = required(&input, "right-id.def")?;
    let left_id = required(&input, "left-id.def")?;
    let model = required(&input, "model.def")?;

    let bigram_right = work.join("bigram.right");
    let bigram_left = work.join("bigram.left");
    let bigram_cost = work.join("bigram.cost");
    eprintln!("生成紧凑连接模型...");
    generate_bigram_info(
        File::open(feature)?,
        File::open(right_id)?,
        File::open(left_id)?,
        File::open(model)?,
        700.0,
        File::create(&bigram_right)?,
        File::create(&bigram_left)?,
        File::create(&bigram_cost)?,
    )?;

    eprintln!("编译 Vibrato 字典（跳过 {}）...", matrix.display());
    let dictionary = SystemDictionaryBuilder::from_readers_with_bigram_info(
        File::open(lex)?,
        File::open(&bigram_right)?,
        File::open(&bigram_left)?,
        File::open(&bigram_cost)?,
        File::open(char_def)?,
        File::open(unk)?,
        true,
    )?;
    let mut writer = File::create(&output)?;
    let bytes = dictionary.write(&mut writer)?;
    eprintln!("已写入 {} 字节: {}", bytes, output.display());
    Ok(())
}
