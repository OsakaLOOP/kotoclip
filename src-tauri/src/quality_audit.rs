use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

const QUALITY_ROOT: &str = "experiments/quality-audit-series";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityAuditComparison {
    comparison_id: String,
    manifest: Value,
    summary: Value,
    reading_index: Value,
    gate: Option<Value>,
    lifecycle: Option<Value>,
    memory_profile: Option<Value>,
}

#[derive(Deserialize)]
struct ReadingIndex {
    units: Vec<ReadingIndexUnit>,
}

#[derive(Deserialize)]
struct ReadingIndexUnit {
    unit_id: String,
    offset: u64,
    bytes: u64,
}

fn repository_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法解析仓库根目录".to_string())
}

fn quality_root() -> Result<PathBuf, String> {
    if !cfg!(debug_assertions) {
        return Err("语言质量审计只在开发构建中开放".to_string());
    }
    let root = repository_root()?.join(QUALITY_ROOT);
    if !root.is_dir() {
        return Err(format!("语言质量实验目录不存在：{}", root.display()));
    }
    root.canonicalize()
        .map_err(|error| format!("无法解析语言质量实验目录：{error}"))
}

fn validate_comparison_id(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("comparison ID 不能为空".to_string());
    }
    let path = Path::new(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("comparison ID 只能包含相对目录片段".to_string());
    }
    Ok(())
}

fn comparison_dir(root: &Path, comparison_id: &str) -> Result<PathBuf, String> {
    validate_comparison_id(comparison_id)?;
    let directory = root.join(comparison_id);
    let canonical = directory
        .canonicalize()
        .map_err(|error| format!("对比轮次不存在：{comparison_id}（{error}）"))?;
    if !canonical.starts_with(root) || !canonical.is_dir() {
        return Err("对比轮次超出语言质量实验目录".to_string());
    }
    Ok(canonical)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let file = File::open(path).map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    serde_json::from_reader(file).map_err(|error| format!("无法解析 {}：{error}", path.display()))
}

fn read_gzip_json(path: &Path) -> Result<Value, String> {
    let file = File::open(path).map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    serde_json::from_reader(GzDecoder::new(file))
        .map_err(|error| format!("无法解析 {}：{error}", path.display()))
}

fn read_optional_json(path: &Path) -> Result<Option<Value>, String> {
    if path.is_file() {
        read_json(path).map(Some)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn quality_audit_history() -> Result<Value, String> {
    read_json(&quality_root()?.join("history.json"))
}

#[tauri::command]
pub fn quality_audit_comparison(comparison_id: String) -> Result<QualityAuditComparison, String> {
    let root = quality_root()?;
    let directory = comparison_dir(&root, &comparison_id)?;
    let build_root = if directory.file_name().is_some_and(|name| name == "diff") {
        directory.parent().unwrap_or(&directory)
    } else {
        directory.as_path()
    };
    Ok(QualityAuditComparison {
        comparison_id,
        manifest: read_json(&directory.join("manifest.json"))?,
        summary: read_json(&directory.join("summary.json"))?,
        reading_index: read_gzip_json(&directory.join("reading-index.json.gz")).map_err(
            |error| format!("{error}；该轮次缺少分页索引，请使用当前审计工具重新生成 diff"),
        )?,
        gate: read_optional_json(&directory.join("gate.json"))?,
        lifecycle: read_optional_json(&build_root.join("lifecycle.json"))?,
        memory_profile: read_optional_json(&directory.join("memory-profile.json"))?,
    })
}

#[tauri::command]
pub fn quality_audit_reading_units(
    comparison_id: String,
    unit_ids: Vec<String>,
) -> Result<Vec<Value>, String> {
    if unit_ids.len() > 20 {
        return Err("单次最多读取 20 个审计条目".to_string());
    }
    let root = quality_root()?;
    let directory = comparison_dir(&root, &comparison_id)?;
    let index: ReadingIndex =
        serde_json::from_value(read_gzip_json(&directory.join("reading-index.json.gz"))?)
            .map_err(|error| format!("无法解析 reading index：{error}"))?;
    let mut entries: HashMap<String, ReadingIndexUnit> = index
        .units
        .into_iter()
        .map(|entry| (entry.unit_id.clone(), entry))
        .collect();
    let bundle_path = directory.join("reading-units.bin");
    let mut bundle = File::open(&bundle_path)
        .map_err(|error| format!("无法读取 {}：{error}", bundle_path.display()))?;
    let mut result = Vec::with_capacity(unit_ids.len());
    for unit_id in unit_ids {
        let entry = entries
            .remove(&unit_id)
            .ok_or_else(|| format!("审计条目不存在：{unit_id}"))?;
        if entry.bytes > 16 * 1024 * 1024 {
            return Err(format!("审计条目压缩体积异常：{unit_id}"));
        }
        bundle
            .seek(SeekFrom::Start(entry.offset))
            .map_err(|error| format!("无法定位审计条目 {unit_id}：{error}"))?;
        let mut member = vec![0_u8; entry.bytes as usize];
        bundle
            .read_exact(&mut member)
            .map_err(|error| format!("无法读取审计条目 {unit_id}：{error}"))?;
        let value = serde_json::from_reader(GzDecoder::new(member.as_slice()))
            .map_err(|error| format!("无法解析审计条目 {unit_id}：{error}"))?;
        result.push(value);
    }
    Ok(result)
}
