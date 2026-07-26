mod normalize;
mod sort;

use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Default)]
struct Args {
    options: HashMap<String, String>,
    flags: HashSet<String>,
}

impl Args {
    fn parse(values: impl Iterator<Item = String>) -> Result<Self> {
        let values: Vec<String> = values.collect();
        let mut parsed = Self::default();
        let mut index = 0;
        while index < values.len() {
            let value = &values[index];
            if !value.starts_with("--") {
                bail!("无法识别的位置参数：{value}");
            }
            let key = value.trim_start_matches("--").to_owned();
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

    fn required_path(&self, key: &str) -> Result<PathBuf> {
        self.options
            .get(key)
            .map(PathBuf::from)
            .with_context(|| format!("缺少 --{key}"))
    }

    fn required(&self, key: &str) -> Result<&str> {
        self.options
            .get(key)
            .map(String::as_str)
            .with_context(|| format!("缺少 --{key}"))
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("错误：{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut values = std::env::args().skip(1);
    let command = values.next().unwrap_or_else(|| "help".to_owned());
    let args = Args::parse(values)?;
    match command.as_str() {
        "normalize" => normalize::run(
            &args.required_path("manifest")?,
            &args.required_path("output-root")?,
            &args.required_path("metadata")?,
        ),
        "sort" => sort::run(
            &args.required_path("input")?,
            &args.required_path("output")?,
            args.required("mode")?,
        ),
        "candidates" => normalize::run_candidates(
            &args.required_path("before-manifest")?,
            &args.required_path("after-manifest")?,
            &args.required_path("output")?,
            &args.required_path("metadata")?,
            args.options.get("count-cache").map(Path::new),
            args.options.get("before-reading-output").map(Path::new),
            args.options.get("after-reading-output").map(Path::new),
        ),
        "reading-sentences" => normalize::run_reading_sentences(
            &args.required_path("manifest")?,
            &args.required_path("ranges")?,
            &args.required_path("output")?,
            &args.required_path("metadata")?,
        ),
        "compare" => compare(&args),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        _ if command.starts_with("--") => {
            bail!("请显式使用 compare 子命令")
        }
        _ => bail!("未知命令：{command}"),
    }
}

fn compare(args: &Args) -> Result<()> {
    let python = args
        .options
        .get("python")
        .map(String::as_str)
        .unwrap_or("python");
    let script = args.required_path("python-script")?;
    let executable = std::env::current_exe().context("无法定位 Rust diff 可执行文件")?;
    let mut command = Command::new(python);
    command
        .arg(&script)
        .arg("--before-run")
        .arg(args.required_path("before-run")?)
        .arg("--after-run")
        .arg(args.required_path("after-run")?)
        .arg("--output-dir")
        .arg(args.required_path("output-dir")?)
        .env("KOTOCLIP_QUALITY_DIFF_ACCELERATOR", executable);
    if args.flags.contains("keep-reading-spool") {
        command.arg("--keep-reading-spool");
    }
    let status = command.status().context("无法启动 Python 兼容层")?;
    if !status.success() {
        bail!("Python 兼容层退出码：{}", status.code().unwrap_or(-1));
    }
    Ok(())
}

fn print_help() {
    println!(
        "kotoclip-quality-diff compare --before-run PATH --after-run PATH --output-dir DIR --python-script PATH\n\
         kotoclip-quality-diff normalize --manifest PATH --output-root DIR --metadata PATH\n\
         kotoclip-quality-diff candidates --before-manifest PATH --after-manifest PATH --output PATH --metadata PATH [--count-cache DIR] [--before-reading-output PATH --after-reading-output PATH]\n\
         kotoclip-quality-diff reading-sentences --manifest PATH --ranges PATH --output PATH --metadata PATH\n\
         kotoclip-quality-diff sort --input PATH --output PATH --mode key|anchor"
    );
}

fn _path_display(path: &Path) -> String {
    path.display().to_string()
}
