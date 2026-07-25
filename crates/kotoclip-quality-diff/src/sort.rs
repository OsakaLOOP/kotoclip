use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

#[derive(Deserialize)]
struct EntityHeader<'a> {
    #[serde(borrow)]
    key: &'a str,
    #[serde(borrow)]
    anchor: &'a str,
    #[serde(borrow)]
    value: &'a RawValue,
}

struct Record {
    key: String,
    anchor: String,
    value: String,
    line: String,
}

pub fn run(input: &Path, output: &Path, mode: &str) -> Result<()> {
    if mode != "key" && mode != "anchor" {
        bail!("--mode 只能是 key 或 anchor");
    }
    let source = File::open(input).with_context(|| format!("无法读取 {}", input.display()))?;
    let reader: Box<dyn BufRead> = if input.extension().is_some_and(|value| value == "gz") {
        Box::new(BufReader::with_capacity(
            1024 * 1024,
            GzDecoder::new(source),
        ))
    } else {
        Box::new(BufReader::with_capacity(1024 * 1024, source))
    };
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let header: EntityHeader<'_> = serde_json::from_str(&line)?;
        records.push(Record {
            key: header.key.to_owned(),
            anchor: header.anchor.to_owned(),
            value: header.value.get().to_owned(),
            line,
        });
    }
    records.sort_unstable_by(|left, right| compare(left, right, mode));
    let mut writer = BufWriter::with_capacity(1024 * 1024, File::create(output)?);
    for record in records {
        writer.write_all(record.line.as_bytes())?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn compare(left: &Record, right: &Record, mode: &str) -> Ordering {
    if mode == "anchor" {
        left.anchor
            .cmp(&right.anchor)
            .then_with(|| left.key.cmp(&right.key))
            .then_with(|| left.value.cmp(&right.value))
    } else {
        left.key
            .cmp(&right.key)
            .then_with(|| left.value.cmp(&right.value))
    }
}
