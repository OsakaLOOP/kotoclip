use crate::model::{AuditManifest, LexicalObservation};
use anyhow::{bail, Context, Result};
use flate2::{write::GzEncoder, Compression};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ArtifactTransaction {
    final_directory: PathBuf,
    staging_directory: PathBuf,
    committed: bool,
}

impl ArtifactTransaction {
    pub fn new(final_directory: &Path) -> Result<Self> {
        if final_directory.exists() {
            bail!("审计输出已存在：{}", final_directory.display());
        }
        let parent = final_directory.parent().context("审计输出必须有父目录")?;
        fs::create_dir_all(parent)?;
        let name = final_directory
            .file_name()
            .context("审计输出目录名为空")?
            .to_string_lossy();
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let staging_directory =
            parent.join(format!(".{name}.staging-{}-{nonce}", std::process::id()));
        fs::create_dir(&staging_directory)?;
        Ok(Self {
            final_directory: final_directory.to_path_buf(),
            staging_directory,
            committed: false,
        })
    }

    pub fn staging_directory(&self) -> &Path {
        &self.staging_directory
    }

    pub fn write_json<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        let path = self.staging_directory.join(name);
        let writer = BufWriter::new(File::create(&path)?);
        serde_json::to_writer_pretty(writer, value)
            .with_context(|| format!("无法写入 {}", path.display()))?;
        Ok(())
    }

    pub fn write_changes(&self, changes: &[LexicalObservation]) -> Result<()> {
        let path = self.staging_directory.join("changes.jsonl.gz");
        let file = File::create(&path)?;
        let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::fast());
        for change in changes {
            serde_json::to_writer(
                &mut encoder,
                &json!({
                    "schema_version": "kotoclip.quality.pipeline-change.v1",
                    "change_id": change.change_id,
                    "primary_domain": change.primary_domain,
                    "reading_unit_id": reading_unit_id(change),
                    "book_id": change.book_id,
                    "paragraph_id": change.paragraph_id,
                    "reading_sentence_id": change.reading_sentence_id,
                    "sentence_text": change.sentence_text,
                    "char_range": change.char_range,
                    "changed_ranges": change.changed_ranges,
                    "before_units": change.before_units,
                    "after_units": change.after_units,
                    "before_formations": word_formations(&change.before_tokens),
                    "after_formations": word_formations(&change.after_tokens),
                }),
            )?;
            encoder.write_all(b"\n")?;
        }
        encoder.finish()?.flush()?;
        Ok(())
    }

    pub fn write_reading_bundle(&self, changes: &[LexicalObservation]) -> Result<()> {
        let bundle_path = self.staging_directory.join("reading-units.bin");
        let mut bundle = BufWriter::with_capacity(1024 * 1024, File::create(&bundle_path)?);
        let mut offset = 0_u64;
        let mut index_units = Vec::with_capacity(changes.len());
        for (chunk_index, chunk) in changes.chunks(20).enumerate() {
            let units: Vec<Value> = chunk
                .iter()
                .enumerate()
                .map(|(member_index, change)| reading_unit(change, chunk_index * 20 + member_index))
                .collect();
            let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
            serde_json::to_writer(&mut encoder, &units)?;
            let compressed = encoder.finish()?;
            let bytes = compressed.len() as u64;
            bundle.write_all(&compressed)?;
            for (member_index, unit) in units.into_iter().enumerate() {
                let mut index_unit = unit;
                let object = index_unit
                    .as_object_mut()
                    .context("reading unit 必须是对象")?;
                object.remove("change_ids");
                object.remove("before").and_then(|side| {
                    side.as_object().map(|side| {
                        object.insert(
                            "before".to_string(),
                            json!({
                                "char_range": side.get("char_range"),
                                "text": side.get("text"),
                            }),
                        );
                    })
                });
                object.remove("after").and_then(|side| {
                    side.as_object().map(|side| {
                        object.insert(
                            "after".to_string(),
                            json!({
                                "char_range": side.get("char_range"),
                                "text": side.get("text"),
                            }),
                        );
                    })
                });
                object.insert("offset".to_string(), json!(offset));
                object.insert("bytes".to_string(), json!(bytes));
                object.insert("member_index".to_string(), json!(member_index));
                index_units.push(index_unit);
            }
            offset += bytes;
        }
        bundle.flush()?;
        self.write_gzip_json(
            "reading-index.json.gz",
            &json!({
                "schema_version": "kotoclip.quality.reading-index.v2",
                "reading_schema_version": "kotoclip.quality.reading-diff.v1",
                "bundle_schema_version": "kotoclip.quality.reading-bundle.v2",
                "chunk_units": 20,
                "unit_count": index_units.len(),
                "units": index_units,
            }),
        )
    }

    pub fn write_gzip_json<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        let path = self.staging_directory.join(name);
        let file = File::create(&path)?;
        let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::fast());
        serde_json::to_writer(&mut encoder, value)?;
        encoder.finish()?.flush()?;
        Ok(())
    }

    pub fn commit(mut self, manifest: &AuditManifest) -> Result<PathBuf> {
        // manifest 最后写入；没有 manifest 的 staging 永远不是有效轮次。
        self.write_json("manifest.json", manifest)?;
        fs::rename(&self.staging_directory, &self.final_directory).with_context(|| {
            format!(
                "无法原子发布审计产物 {} -> {}",
                self.staging_directory.display(),
                self.final_directory.display()
            )
        })?;
        self.committed = true;
        Ok(self.final_directory.clone())
    }

    pub fn projected_commit_size(&self, manifest: &AuditManifest) -> Result<u64> {
        Ok(directory_size(&self.staging_directory)?
            + serde_json::to_vec_pretty(manifest)?.len() as u64)
    }
}

fn reading_unit(change: &LexicalObservation, index: usize) -> Value {
    let changed_start = change
        .changed_ranges
        .iter()
        .map(|range| range.start)
        .min()
        .unwrap_or(change.char_range.start);
    let changed_end = change
        .changed_ranges
        .iter()
        .map(|range| range.end)
        .max()
        .unwrap_or(change.char_range.end);
    json!({
        "unit_id": reading_unit_id(change),
        "sentence_index": index,
        "changed_range": [changed_start, changed_end],
        "changed_ranges": change.changed_ranges,
        "primary_change_count": 1,
        "evidence_change_count": 0,
        "domains": {change.primary_domain.clone(): 1, "bunsetsu": 1, "ui_projection": 1},
        "stages": [change.primary_domain.clone(), "bunsetsu", "ui_projection"],
        "change_ids": [change.change_id],
        "before": {
            "char_range": change.char_range,
            "text": change.sentence_text,
            "tokens": change.before_tokens.iter().map(reading_token).collect::<Vec<_>>(),
        },
        "after": {
            "char_range": change.char_range,
            "text": change.sentence_text,
            "tokens": change.after_tokens.iter().map(reading_token).collect::<Vec<_>>(),
        },
    })
}

fn reading_unit_id(change: &LexicalObservation) -> String {
    format!(
        "{}:{}:{}:{}",
        change.book_id, change.char_range.start, change.char_range.end, change.change_id
    )
}

fn word_formations(
    tokens: &[kotoclip_core::models::AnnotatedToken],
) -> Vec<&kotoclip_core::models::WordFormationAnnotation> {
    tokens
        .iter()
        .flat_map(|token| token.bunsetsu.word_formations.iter())
        .collect()
}

fn reading_token(token: &kotoclip_core::models::AnnotatedToken) -> Value {
    json!({
        "surface": token.bunsetsu.surface,
        "char_range": token.bunsetsu.char_range,
        "head_word": token.bunsetsu.head_word,
        "morphemes": token.bunsetsu.morphemes,
        "morphology": token.bunsetsu.morphology,
        "word_formations": token.bunsetsu.word_formations,
        "lexical_units": token.bunsetsu.lexical_units,
        "grammar_occurrences": token.bunsetsu.grammar_occurrences,
        "grammar_tags": token.bunsetsu.grammar_tags,
        "functional_residuals": token.bunsetsu.functional_residuals,
        "function": token.bunsetsu.function,
        "expressions": token.expressions,
        "display_class": token.display_class,
    })
}

impl Drop for ArtifactTransaction {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.staging_directory);
        }
    }
}

pub fn directory_size(path: &Path) -> Result<u64> {
    let mut total = 0;
    if !path.exists() {
        return Ok(0);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            total += directory_size(&entry.path())?;
        } else {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_transaction_removes_staging() {
        let temp = tempfile::tempdir().unwrap();
        let final_path = temp.path().join("result");
        let staging;
        {
            let transaction = ArtifactTransaction::new(&final_path).unwrap();
            staging = transaction.staging_directory().to_path_buf();
            fs::write(staging.join("partial"), b"partial").unwrap();
        }
        assert!(!staging.exists());
        assert!(!final_path.exists());
    }
}
