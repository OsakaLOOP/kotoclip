use anyhow::{bail, Context, Result};
use kotoclip_quality_audit::{
    build_substrate, freeze_library_corpus, gc_substrates, publish_history, run_containment_audit,
    BuildSubstrateOptions, ContainmentAuditOptions,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Default)]
struct Args {
    options: HashMap<String, String>,
    flags: HashSet<String>,
}

impl Args {
    fn parse(values: impl Iterator<Item = String>) -> Result<Self> {
        let values: Vec<String> = values.collect();
        let mut result = Self::default();
        let mut index = 0;
        while index < values.len() {
            let value = &values[index];
            if !value.starts_with("--") {
                bail!("无法识别的位置参数：{value}");
            }
            let key = value.trim_start_matches("--").to_string();
            if index + 1 < values.len() && !values[index + 1].starts_with("--") {
                result.options.insert(key, values[index + 1].clone());
                index += 2;
            } else {
                result.flags.insert(key);
                index += 1;
            }
        }
        Ok(result)
    }

    fn path(&self, name: &str) -> Result<PathBuf> {
        self.options
            .get(name)
            .map(PathBuf::from)
            .with_context(|| format!("缺少 --{name}"))
    }

    fn u64(&self, name: &str, default: u64) -> Result<u64> {
        self.options
            .get(name)
            .map(|value| {
                value
                    .parse()
                    .with_context(|| format!("--{name} 必须是整数"))
            })
            .transpose()
            .map(|value| value.unwrap_or(default))
    }

    fn string(&self, name: &str, default: &str) -> String {
        self.options
            .get(name)
            .cloned()
            .unwrap_or_else(|| default.to_string())
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
    let command = values.next().unwrap_or_else(|| "help".to_string());
    let args = Args::parse(values)?;
    match command.as_str() {
        "freeze-library" => {
            freeze_library_corpus(&args.path("library")?, &args.path("output")?)?;
        }
        "build-substrate" => {
            let directory = build_substrate(&BuildSubstrateOptions {
                corpus_spec: args.path("corpus")?,
                system_dictionary: args.path("system-dict")?,
                output_root: args.path("output-root")?,
                max_bytes: args.u64("max-bytes", 512 * 1024 * 1024)?,
            })?;
            println!("{}", directory.display());
        }
        "audit-containment" => {
            let output = run_containment_audit(&ContainmentAuditOptions {
                repository_root: args.path("repository-root")?,
                substrate_directory: args.path("substrate")?,
                system_dictionary: args.path("system-dict")?,
                dictionary_directory: args.path("dict-dir")?,
                output_directory: args.path("output")?,
                before_revision: args
                    .string("before-revision", "semantic:reject_non_equal_overlap"),
                after_revision: args.string("after-revision", "semantic:reject_crossing_overlap"),
                verify_full_domain: args.flags.contains("verify-full-domain"),
                max_elapsed: Duration::from_secs(args.u64("max-seconds", 150)?),
                max_peak_rss_bytes: args.u64("max-rss-bytes", 1024 * 1024 * 1024)?,
                max_temporary_bytes: args.u64("max-temp-bytes", 1024 * 1024 * 1024)?,
                max_artifact_bytes: args.u64("max-artifact-bytes", 256 * 1024 * 1024)?,
            })?;
            if let Some(history_root) = args.options.get("history-root") {
                publish_history(&PathBuf::from(history_root), &output)?;
            }
            println!("{}", output.display());
        }
        "gc-substrates" => {
            let keep: HashSet<String> = args
                .options
                .get("keep")
                .map(|value| value.split(',').map(str::to_string).collect())
                .unwrap_or_default();
            let removed = gc_substrates(
                &args.path("root")?,
                &keep,
                args.u64("max-bytes", 512 * 1024 * 1024)?,
            )?;
            println!("removed_bytes={removed}");
        }
        "help" | "--help" | "-h" => print_help(),
        _ => bail!("未知命令：{command}"),
    }
    Ok(())
}

fn print_help() {
    println!(
        "kotoclip-quality-audit freeze-library --library DIR --output PATH\n\
         kotoclip-quality-audit build-substrate --corpus PATH --system-dict PATH --output-root DIR [--max-bytes N]\n\
         kotoclip-quality-audit audit-containment --repository-root DIR --substrate DIR --system-dict PATH --dict-dir DIR --output DIR [--before-revision ID] [--after-revision ID] [--history-root DIR] [--verify-full-domain]\n\
         kotoclip-quality-audit gc-substrates --root DIR --max-bytes N [--keep ID,ID]"
    );
}
