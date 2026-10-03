use crate::fingerprint::sha256_file;
use crate::model::{AuditManifest, AUDIT_SCHEMA_VERSION};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const HISTORY_SCHEMA_VERSION: &str = "kotoclip.quality.comparison-history.v1";

pub fn publish_history(history_root: &Path, audit_directory: &Path) -> Result<PathBuf> {
    fs::create_dir_all(history_root)?;
    let root = history_root
        .canonicalize()
        .with_context(|| format!("无法解析审计历史根目录：{}", history_root.display()))?;
    let audit = audit_directory
        .canonicalize()
        .with_context(|| format!("无法解析已完成审计目录：{}", audit_directory.display()))?;
    let relative = audit
        .strip_prefix(&root)
        .with_context(|| format!("审计目录必须位于历史根目录内：{}", audit.display()))?;
    if relative.as_os_str().is_empty() {
        bail!("审计目录不能等于历史根目录");
    }
    let comparison_id = relative.to_string_lossy().replace('\\', "/");
    let manifest: AuditManifest =
        serde_json::from_reader(File::open(audit.join("manifest.json"))?)?;
    if manifest.schema_version != AUDIT_SCHEMA_VERSION || !manifest.complete {
        bail!("不能发布不完整或不兼容的选择式审计轮次");
    }
    let summary: Value = serde_json::from_reader(File::open(audit.join("summary.json"))?)?;
    let gate: Value = serde_json::from_reader(File::open(audit.join("gate.json"))?)?;
    let history_path = root.join("history.json");
    let mut history = if history_path.is_file() {
        serde_json::from_reader::<_, Value>(File::open(&history_path)?)?
    } else {
        json!({
            "schema_version": HISTORY_SCHEMA_VERSION,
            "root": ".",
            "comparisons": [],
        })
    };
    if history.get("schema_version").and_then(Value::as_str) != Some(HISTORY_SCHEMA_VERSION) {
        bail!("history.json schema 不兼容");
    }
    let comparisons = history
        .get_mut("comparisons")
        .and_then(Value::as_array_mut)
        .context("history.json 缺少 comparisons 数组")?;
    comparisons.retain(|record| {
        record.get("comparison_id").and_then(Value::as_str) != Some(comparison_id.as_str())
    });
    comparisons.push(history_record(
        &root,
        &audit,
        &comparison_id,
        &manifest,
        summary,
        gate,
    )?);
    comparisons.sort_by(|left, right| {
        let left_key = (
            left.get("created_at").and_then(Value::as_str).unwrap_or(""),
            left.get("comparison_id")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        let right_key = (
            right
                .get("created_at")
                .and_then(Value::as_str)
                .unwrap_or(""),
            right
                .get("comparison_id")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        right_key.cmp(&left_key)
    });
    atomic_write_json(&history_path, &history)?;
    Ok(history_path)
}

fn history_record(
    root: &Path,
    audit: &Path,
    comparison_id: &str,
    manifest: &AuditManifest,
    summary: Value,
    gate: Value,
) -> Result<Value> {
    let adapter = if manifest
        .plan
        .deltas
        .iter()
        .all(|delta| delta.owner == "pipeline.word_formation")
    {
        "rust_selective_word_formation_catalog"
    } else {
        "rust_selective_proper_containment"
    };
    Ok(json!({
        "comparison_id": comparison_id,
        "created_at": manifest.created_at,
        "adapter": adapter,
        "before": history_side(&manifest.before, manifest),
        "after": history_side(&manifest.after, manifest),
        "summary": summary,
        "gate_status": gate.get("status").cloned().unwrap_or(Value::Null),
        "manifest": artifact(root, &audit.join("manifest.json"))?,
        "summary_artifact": artifact(root, &audit.join("summary.json"))?,
        "diff": artifact(root, &audit.join("changes.jsonl.gz"))?,
        "reading_diff": Value::Null,
        "reading_index": artifact(root, &audit.join("reading-index.json.gz"))?,
        "reading_units": artifact(root, &audit.join("reading-units.bin"))?,
        "gate": artifact(root, &audit.join("gate.json"))?,
        "lifecycle": artifact(root, &audit.join("lifecycle.json"))?,
        "memory_profile": Value::Null,
    }))
}

fn history_side(endpoint: &crate::model::AuditEndpoint, manifest: &AuditManifest) -> Value {
    json!({
        "label": endpoint.label,
        "implementation": {
            "git_commit": endpoint.revision,
            "git_dirty": Value::Null,
            "semantic_mode": endpoint.semantic_mode,
        },
        "corpus": {
            "id": manifest.substrate_id,
            "selected_characters": manifest.counts.characters,
            "analysis_characters": manifest.counts.executed_paragraph_characters,
        },
    })
}

fn artifact(root: &Path, path: &Path) -> Result<Value> {
    let relative = path.strip_prefix(root)?;
    Ok(json!({
        "url": relative.to_string_lossy().replace('\\', "/"),
        "bytes": fs::metadata(path)?.len(),
        "sha256": sha256_file(path)?,
    }))
}

fn atomic_write_json(path: &Path, value: &Value) -> Result<()> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let file_name = path
        .file_name()
        .context("history 文件名为空")?
        .to_string_lossy();
    let temporary = path.with_file_name(format!(
        ".{file_name}.staging-{}-{nonce}",
        std::process::id()
    ));
    let result = (|| -> Result<()> {
        let mut writer = BufWriter::new(File::create(&temporary)?);
        serde_json::to_writer_pretty(&mut writer, value)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        atomic_replace(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(windows)]
fn atomic_replace(source: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error()).context("无法原子发布 history.json");
    }
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, destination: &Path) -> Result<()> {
    fs::rename(source, destination).context("无法原子发布 history.json")
}
