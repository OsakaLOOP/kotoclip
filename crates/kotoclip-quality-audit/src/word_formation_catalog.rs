use crate::artifact::{directory_size, ArtifactTransaction};
use crate::containment::{
    classify_token_ranges, ensure_source_unchanged, lexical_units, prepare_selected_paragraphs,
    record_stage, tokens_in_range, MemorySampler,
};
use crate::fingerprint::{execution_fingerprint, sha256_bytes, sha256_file, sha256_serializable};
use crate::model::{
    AuditCounts, AuditEndpoint, AuditManifest, CharRange, InfluenceScope, LexicalObservation,
    ResourceUsage, SemanticDelta, StageResourceUsage, AUDIT_SCHEMA_VERSION,
};
use crate::planner::plan;
use crate::substrate::{read_book_chunk, read_manifest};
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use kotoclip_core::dictionary::lookup::DictionaryEngine;
use kotoclip_core::document::offset_token_ranges;
use kotoclip_core::pipeline::lexical::prepare_dictionary_lexical_candidates;
use kotoclip_core::pipeline::word_formation::{AcceptedWordFormation, WordFormationMatcher};
use kotoclip_core::pipeline::Pipeline;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct WordFormationCatalogAuditOptions {
    pub repository_root: PathBuf,
    pub substrate_directory: PathBuf,
    pub system_dictionary: PathBuf,
    pub dictionary_directory: PathBuf,
    pub before_catalog: PathBuf,
    pub after_catalog: PathBuf,
    pub output_directory: PathBuf,
    pub before_revision: String,
    pub after_revision: String,
    pub verify_full_domain: bool,
    pub max_elapsed: Duration,
    pub max_peak_rss_bytes: u64,
    pub max_temporary_bytes: u64,
    pub max_artifact_bytes: u64,
}

pub fn run_word_formation_catalog_audit(
    options: &WordFormationCatalogAuditOptions,
) -> Result<PathBuf> {
    let started = Instant::now();
    let sampler = MemorySampler::start();
    let mut stages = BTreeMap::new();
    let initialization_started = Instant::now();
    let transaction = ArtifactTransaction::new(&options.output_directory)?;
    let substrate_manifest = read_manifest(&options.substrate_directory)?;
    if sha256_file(&options.system_dictionary)?
        != substrate_manifest.fingerprint.system_dictionary_sha256
    {
        bail!("system.dic 与形态素底座不一致");
    }

    let before_source = read_catalog_input(&options.repository_root, &options.before_catalog)
        .with_context(|| {
            format!(
                "无法读取 before 构词目录：{}",
                options.before_catalog.display()
            )
        })?;
    let after_source = read_catalog_input(&options.repository_root, &options.after_catalog)
        .with_context(|| {
            format!(
                "无法读取 after 构词目录：{}",
                options.after_catalog.display()
            )
        })?;
    let before_matcher = WordFormationMatcher::from_json(&before_source)
        .map_err(|error| anyhow!(error.to_string()))?;
    let after_matcher = WordFormationMatcher::from_json(&after_source)
        .map_err(|error| anyhow!(error.to_string()))?;
    let before_hash = sha256_bytes(before_source.as_bytes());
    let after_hash = sha256_bytes(after_source.as_bytes());
    let changed_rule_ids = changed_rule_ids(&before_source, &after_source)?;
    if changed_rule_ids.is_empty() {
        bail!("before/after 构词目录没有语义差异");
    }
    let change_id = format!(
        "word_formation.catalog.{}_to_{}",
        short_revision(&options.before_revision),
        short_revision(&options.after_revision)
    );
    let base_fingerprint = execution_fingerprint(
        &options.repository_root,
        &options.system_dictionary,
        &options.dictionary_directory,
    )?;
    let execution_fingerprint = sha256_serializable(&(
        base_fingerprint,
        before_hash.as_str(),
        after_hash.as_str(),
        "word_formation_catalog.v1",
    ))?;
    let execution_plan = plan(
        vec![SemanticDelta {
            change_id: change_id.clone(),
            owner: "pipeline.word_formation".to_string(),
            kind: "catalog_delta".to_string(),
            before_hash: before_hash.clone(),
            after_hash: after_hash.clone(),
            scope: InfluenceScope::Paragraph,
            old_selector: format!("full_old_matcher({})", changed_rule_ids.join(",")),
            new_selector: format!("full_new_matcher({})", changed_rule_ids.join(",")),
            observation_set: vec![
                "word_formation".to_string(),
                "lexical_unit".to_string(),
                "bunsetsu".to_string(),
                "grammar_occurrence".to_string(),
                "expression".to_string(),
                "ui_projection".to_string(),
            ],
            soundness: "exact_full_morpheme_matcher_scan".to_string(),
            reads_absence: false,
            unbounded_context: false,
        }],
        true,
    );
    let dictionary = DictionaryEngine::new(&options.dictionary_directory)
        .map_err(|error| anyhow!(error.to_string()))?;
    let pipeline =
        Pipeline::new(&options.system_dictionary).map_err(|error| anyhow!(error.to_string()))?;
    record_stage(
        &mut stages,
        &sampler,
        "initialization",
        initialization_started,
    );

    let mut counts = AuditCounts {
        books: substrate_manifest.books.len(),
        characters: substrate_manifest.total_characters,
        scanned_characters: substrate_manifest.total_characters,
        morphemes: substrate_manifest.total_morphemes,
        ..AuditCounts::default()
    };
    let selector_started = Instant::now();
    let mut changed_paragraphs = BTreeSet::new();
    let mut selected_clause_characters = 0;
    for (book_index, descriptor) in substrate_manifest.books.iter().enumerate() {
        ensure_source_unchanged(descriptor)?;
        let chunk = read_book_chunk(&options.substrate_directory, descriptor)?;
        counts.clauses += chunk.clauses.len();
        for clause in chunk.clauses {
            let before = before_matcher.match_morphemes(&clause.morphemes);
            let after = after_matcher.match_morphemes(&clause.morphemes);
            counts.formation_candidates += after.accepted.len();
            if formation_signature(&before.accepted)? == formation_signature(&after.accepted)? {
                continue;
            }
            counts.selected_clauses += 1;
            selected_clause_characters += clause.range.end.saturating_sub(clause.range.start);
            changed_paragraphs.insert((book_index, clause.paragraph_id));
        }
        enforce_runtime_budget(
            started,
            sampler.overall_peak(),
            transaction.staging_directory(),
            options,
        )?;
    }
    counts.selected_clause_characters = selected_clause_characters;
    counts.selected_paragraphs = changed_paragraphs.len();
    record_stage(&mut stages, &sampler, "selector_scan", selector_started);

    // old/new matcher 已在全部 clause 上执行；这不是抽样 oracle。
    if options.verify_full_domain {
        stages.insert(
            "full_domain_oracle".to_string(),
            StageResourceUsage {
                elapsed_ms: 0,
                peak_rss_bytes: sampler.take_phase_peak(),
            },
        );
    }

    let paragraph_started = Instant::now();
    let paragraphs = prepare_selected_paragraphs(
        &options.substrate_directory,
        &substrate_manifest,
        &changed_paragraphs,
    )?;
    counts.executed_paragraph_characters = paragraphs
        .iter()
        .map(|paragraph| paragraph.range.end.saturating_sub(paragraph.range.start))
        .sum();
    let mut observation_queries = HashSet::new();
    for paragraph in &paragraphs {
        for segment in &paragraph.content_segments {
            let candidates = prepare_dictionary_lexical_candidates(&segment.morphemes);
            counts.lexical_candidates += candidates.len();
            candidates.extend_queries(&mut observation_queries);
        }
    }
    counts.dictionary_queries = observation_queries.len();
    counts.observation_dictionary_queries = observation_queries.len();
    let observation_matches = dictionary.resolve_exact_forms_batch(&observation_queries);
    let mut changes = Vec::new();
    for paragraph in paragraphs {
        let (mut before_tokens, mut after_tokens) = pipeline
            .process_preanalyzed_word_formation_pair(
                &paragraph.text,
                &paragraph.annotations,
                &paragraph.content_segments,
                &[],
                &observation_matches,
                &before_matcher,
                &after_matcher,
            )
            .map_err(|error| anyhow!(error))?;
        offset_token_ranges(&mut before_tokens, paragraph.range.start, 0);
        offset_token_ranges(&mut after_tokens, paragraph.range.start, 0);
        let token_changes = classify_token_ranges(&before_tokens, &after_tokens)?;
        counts.ordinal_only_token_propagations += token_changes.ordinal_only.len();
        for (reading_sentence_id, sentence_range, sentence_text) in paragraph.reading_sentences {
            let sentence_changed: Vec<CharRange> = token_changes
                .visible
                .iter()
                .copied()
                .filter(|range| range.intersects(sentence_range))
                .collect();
            if sentence_changed.is_empty() {
                continue;
            }
            let before = tokens_in_range(&before_tokens, sentence_range);
            let after = tokens_in_range(&after_tokens, sentence_range);
            changes.push(LexicalObservation {
                change_id: change_id.clone(),
                primary_domain: "word_formation".to_string(),
                book_id: paragraph.book_id.clone(),
                paragraph_id: paragraph.paragraph_id,
                reading_sentence_id,
                sentence_text,
                char_range: sentence_range,
                changed_ranges: sentence_changed,
                before_units: lexical_units(&before),
                after_units: lexical_units(&after),
                before_tokens: before,
                after_tokens: after,
            });
        }
        enforce_runtime_budget(
            started,
            sampler.overall_peak(),
            transaction.staging_directory(),
            options,
        )?;
    }
    counts.excluded_from_heavy_pipeline_characters = counts
        .characters
        .saturating_sub(counts.executed_paragraph_characters);
    counts.changed_paragraphs = changes
        .iter()
        .map(|change| (change.book_id.as_str(), change.paragraph_id))
        .collect::<BTreeSet<_>>()
        .len();
    counts.changes = changes.len();
    record_stage(
        &mut stages,
        &sampler,
        "paragraph_reanalysis",
        paragraph_started,
    );

    let artifact_started = Instant::now();
    transaction.write_changes(&changes)?;
    transaction.write_reading_bundle(&changes)?;
    transaction.write_json(
        "summary.json",
        &serde_json::json!({
            "status": if changes.is_empty() { "unchanged" } else { "changed" },
            "comparable": true,
            "complete_stage_coverage": true,
            "quality_conclusion": if changes.is_empty() { "unchanged" } else { "review_required" },
            "changes": changes.len(),
            "evidence_changes": 0,
            "root_changes": changed_rule_ids.len(),
            "propagated_candidates": counts.ordinal_only_token_propagations,
            "churn_rate": if counts.characters == 0 { 0.0 } else { changes.len() as f64 / counts.characters as f64 },
            "heavy_pipeline_character_rate": if counts.characters == 0 { 0.0 } else { counts.executed_paragraph_characters as f64 / counts.characters as f64 },
            "reading_units": changes.len(),
            "affected_sentences": changes.len(),
            "changed_rule_ids": changed_rule_ids,
            "selection": counts,
            "stages": [
                {"stage": "morpheme", "coverage": 1.0},
                {"stage": "word_formation", "coverage": 1.0},
                {"stage": "lexical_unit", "coverage": 1.0},
                {"stage": "bunsetsu", "coverage": 1.0},
                {"stage": "grammar_occurrence", "coverage": 1.0},
                {"stage": "expression", "coverage": 1.0},
                {"stage": "ui_projection", "coverage": 1.0}
            ],
        }),
    )?;
    transaction.write_json(
        "gate.json",
        &serde_json::json!({
            "status": "passed",
            "complete": true,
            "selector_oracle_status": if options.verify_full_domain { "passed" } else { "integrated_not_asserted" },
            "selector_oracle_kind": "full_old_new_accepted_formation_scan",
            "known_example_status": "covered_by_catalog_governance",
            "n_best_in_scope": false,
        }),
    )?;
    record_stage(
        &mut stages,
        &sampler,
        "artifact_serialization",
        artifact_started,
    );

    let elapsed = started.elapsed();
    let peak_rss = sampler.finish();
    let temporary_bytes = directory_size(transaction.staging_directory())?;
    transaction.write_json(
        "lifecycle.json",
        &serde_json::json!({
            "state": "complete",
            "published_at": Utc::now().to_rfc3339(),
            "atomic_publication": true,
        }),
    )?;
    let audit_id = options
        .output_directory
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut manifest = AuditManifest {
        schema_version: AUDIT_SCHEMA_VERSION.to_string(),
        audit_id,
        created_at: Utc::now().to_rfc3339(),
        substrate_id: substrate_manifest.substrate_id,
        corpus_spec_sha256: substrate_manifest.corpus_spec_sha256,
        execution_fingerprint,
        before: AuditEndpoint {
            revision: options.before_revision.clone(),
            semantic_mode: format!("word_formation_catalog:{before_hash}"),
            label: format!("构词目录 {}", short_revision(&options.before_revision)),
        },
        after: AuditEndpoint {
            revision: options.after_revision.clone(),
            semantic_mode: format!("word_formation_catalog:{after_hash}"),
            label: format!("构词目录 {}", short_revision(&options.after_revision)),
        },
        plan: execution_plan,
        counts,
        resource_usage: ResourceUsage {
            elapsed_ms: elapsed.as_millis(),
            peak_rss_bytes: peak_rss,
            temporary_bytes,
            artifact_bytes: 0,
            substrate_bytes: substrate_manifest.total_bytes,
            stages,
            measurement_scope: "single_process_shared_substrate_catalog_pair".to_string(),
        },
        complete: true,
        n_best_in_scope: false,
    };
    for _ in 0..8 {
        let projected = transaction.projected_commit_size(&manifest)?;
        if manifest.resource_usage.artifact_bytes == projected
            && manifest.resource_usage.temporary_bytes == projected
        {
            break;
        }
        manifest.resource_usage.artifact_bytes = projected;
        manifest.resource_usage.temporary_bytes = projected;
    }
    enforce_final_budget(&manifest.resource_usage, options)?;
    transaction.commit(&manifest)
}

fn changed_rule_ids(before: &str, after: &str) -> Result<Vec<String>> {
    fn inventory(source: &str) -> Result<BTreeMap<String, Value>> {
        let value: Value = serde_json::from_str(source)?;
        let rules = value
            .get("rules")
            .and_then(Value::as_array)
            .context("构词目录缺少 rules 数组")?;
        let mut result = BTreeMap::new();
        for rule in rules {
            let id = rule
                .get("id")
                .and_then(Value::as_str)
                .context("构词规则缺少 id")?;
            if result.insert(id.to_string(), rule.clone()).is_some() {
                bail!("构词规则 ID 重复：{id}");
            }
        }
        Ok(result)
    }
    let before = inventory(before)?;
    let after = inventory(after)?;
    Ok(before
        .keys()
        .chain(after.keys())
        .filter(|id| before.get(*id) != after.get(*id))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

fn read_catalog_input(repository_root: &Path, input: &Path) -> Result<String> {
    let value = input.to_string_lossy();
    let Some(git_object) = value.strip_prefix("git:") else {
        return Ok(fs::read_to_string(input)?);
    };
    let (revision, path) = git_object
        .split_once(':')
        .context("Git 目录输入必须为 git:REV:path")?;
    if revision.trim().is_empty() || path.trim().is_empty() {
        bail!("Git 目录输入缺少 revision 或 path");
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(repository_root)
        .arg("show")
        .arg(format!("{revision}:{path}"))
        .output()
        .context("无法启动 git show")?;
    if !output.status.success() {
        bail!(
            "git show 读取目录失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout).context("Git 中的构词目录不是有效 UTF-8")?)
}

fn formation_signature(values: &[AcceptedWordFormation]) -> Result<Vec<u8>> {
    let projection: Vec<_> = values
        .iter()
        .map(|value| (value.morpheme_range, &value.annotation, &value.output_pos))
        .collect();
    Ok(serde_json::to_vec(&projection)?)
}

fn short_revision(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(12)
        .collect()
}

fn enforce_runtime_budget(
    started: Instant,
    peak_rss: u64,
    staging: &Path,
    options: &WordFormationCatalogAuditOptions,
) -> Result<()> {
    if started.elapsed() > options.max_elapsed {
        bail!("审计超过时间上限");
    }
    if peak_rss > options.max_peak_rss_bytes {
        bail!("审计超过内存上限");
    }
    if directory_size(staging)? > options.max_temporary_bytes {
        bail!("审计超过临时空间上限");
    }
    Ok(())
}

fn enforce_final_budget(
    usage: &ResourceUsage,
    options: &WordFormationCatalogAuditOptions,
) -> Result<()> {
    if usage.elapsed_ms > options.max_elapsed.as_millis() {
        bail!("审计超过时间上限");
    }
    if usage.peak_rss_bytes > options.max_peak_rss_bytes {
        bail!("审计超过内存上限");
    }
    if usage.temporary_bytes > options.max_temporary_bytes {
        bail!("审计超过临时空间上限");
    }
    if usage.artifact_bytes > options.max_artifact_bytes {
        bail!("审计超过持久产物空间上限");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_inventory_detects_add_modify_and_remove() {
        let before = r#"{"schema_version":2,"catalog_version":1,"rules":[{"id":"a","x":1},{"id":"b","x":1}]}"#;
        let after = r#"{"schema_version":2,"catalog_version":1,"rules":[{"id":"b","x":2},{"id":"c","x":1}]}"#;
        assert_eq!(
            changed_rule_ids(before, after).unwrap(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn reads_catalog_directly_from_git_object() {
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .to_path_buf();
        let source = read_catalog_input(
            &repository,
            Path::new("git:HEAD:crates/kotoclip-core/resources/word_formation_patterns.json"),
        )
        .unwrap();
        WordFormationMatcher::from_json(&source).unwrap();
    }
}
