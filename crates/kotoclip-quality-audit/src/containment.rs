use crate::artifact::{directory_size, ArtifactTransaction};
use crate::fingerprint::{execution_fingerprint, sha256_bytes, sha256_file};
use crate::model::{
    AuditCounts, AuditEndpoint, AuditManifest, CharRange, LexicalObservation, ResourceUsage,
    StageResourceUsage, AUDIT_SCHEMA_VERSION,
};
use crate::planner::{containment_delta, plan};
use crate::substrate::{read_book_chunk, read_manifest};
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use kotoclip_core::dictionary::lookup::DictionaryEngine;
use kotoclip_core::document::offset_token_ranges;
use kotoclip_core::models::{AnnotatedToken, DictionaryLexicalUnitAnnotation};
use kotoclip_core::pipeline::lexical::{
    prepare_dictionary_lexical_candidates, resolve_dictionary_lexical_candidates_with_policy,
    DictionaryLexicalCandidates, WordFormationOverlapPolicy,
};
use kotoclip_core::pipeline::ruby::{self, RubyAnnotation};
use kotoclip_core::pipeline::word_formation::{AcceptedWordFormation, WordFormationMatcher};
use kotoclip_core::pipeline::{Pipeline, PreanalyzedContentSegment};
use kotoclip_core::reader_markdown::compile_analysis_text;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ContainmentAuditOptions {
    pub repository_root: PathBuf,
    pub substrate_directory: PathBuf,
    pub system_dictionary: PathBuf,
    pub dictionary_directory: PathBuf,
    pub output_directory: PathBuf,
    pub before_revision: String,
    pub after_revision: String,
    pub verify_full_domain: bool,
    pub max_elapsed: Duration,
    pub max_peak_rss_bytes: u64,
    pub max_temporary_bytes: u64,
    pub max_artifact_bytes: u64,
}

struct SelectedClause {
    book_index: usize,
    clause_id: usize,
    paragraph_id: usize,
    character_count: usize,
    morphemes: Vec<kotoclip_core::models::Morpheme>,
    formations: Vec<AcceptedWordFormation>,
    candidates: DictionaryLexicalCandidates,
}

pub(crate) struct SelectedParagraph {
    pub book_id: String,
    pub paragraph_id: usize,
    pub range: CharRange,
    pub text: String,
    pub annotations: Vec<RubyAnnotation>,
    pub content_segments: Vec<PreanalyzedContentSegment>,
    pub reading_sentences: Vec<(usize, CharRange, String)>,
}

pub(crate) struct MemorySampler {
    stop: Arc<AtomicBool>,
    peak: Arc<AtomicU64>,
    phase_peak: Arc<AtomicU64>,
    worker: Option<thread::JoinHandle<()>>,
}

impl MemorySampler {
    pub fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicU64::new(current_rss_bytes()));
        let phase_peak = Arc::new(AtomicU64::new(current_rss_bytes()));
        let worker_stop = Arc::clone(&stop);
        let worker_peak = Arc::clone(&peak);
        let worker_phase_peak = Arc::clone(&phase_peak);
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                let rss = current_rss_bytes();
                worker_peak.fetch_max(rss, Ordering::Relaxed);
                worker_phase_peak.fetch_max(rss, Ordering::Relaxed);
                thread::sleep(Duration::from_millis(25));
            }
        });
        Self {
            stop,
            peak,
            phase_peak,
            worker: Some(worker),
        }
    }

    pub fn overall_peak(&self) -> u64 {
        self.peak.load(Ordering::Relaxed)
    }

    pub fn take_phase_peak(&self) -> u64 {
        let rss = current_rss_bytes();
        self.peak.fetch_max(rss, Ordering::Relaxed);
        self.phase_peak.swap(rss, Ordering::Relaxed).max(rss)
    }

    pub fn finish(mut self) -> u64 {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.peak.fetch_max(current_rss_bytes(), Ordering::Relaxed);
        self.peak.load(Ordering::Relaxed)
    }
}

pub fn run_containment_audit(options: &ContainmentAuditOptions) -> Result<PathBuf> {
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
    let execution_fingerprint = execution_fingerprint(
        &options.repository_root,
        &options.system_dictionary,
        &options.dictionary_directory,
    )?;
    let execution_plan = plan(vec![containment_delta()], true);
    let matcher = WordFormationMatcher::new().map_err(|error| anyhow!(error.to_string()))?;
    let dictionary = DictionaryEngine::new(&options.dictionary_directory)
        .map_err(|error| anyhow!(error.to_string()))?;
    let pipeline =
        Pipeline::new(&options.system_dictionary).map_err(|error| anyhow!(error.to_string()))?;
    verify_known_example(&pipeline, &dictionary)?;
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
    let mut selected = Vec::new();
    let mut dictionary_queries = HashSet::new();
    let selector_started = Instant::now();

    for (book_index, descriptor) in substrate_manifest.books.iter().enumerate() {
        ensure_source_unchanged(descriptor)?;
        let chunk = read_book_chunk(&options.substrate_directory, descriptor)?;
        counts.clauses += chunk.clauses.len();
        for clause in chunk.clauses {
            let formation_result = matcher.match_morphemes(&clause.morphemes);
            counts.formation_candidates += formation_result.accepted.len();
            if formation_result.accepted.is_empty() {
                continue;
            }
            let candidates = prepare_dictionary_lexical_candidates(&clause.morphemes);
            counts.lexical_candidates += candidates.len();
            if !candidates.has_proper_containment(&formation_result.accepted) {
                continue;
            }
            candidates.extend_queries(&mut dictionary_queries);
            selected.push(SelectedClause {
                book_index,
                clause_id: clause.clause_id,
                paragraph_id: clause.paragraph_id,
                character_count: clause.range.end.saturating_sub(clause.range.start),
                morphemes: clause.morphemes,
                formations: formation_result.accepted,
                candidates,
            });
        }
        enforce_runtime_budget(
            started,
            sampler.overall_peak(),
            transaction.staging_directory(),
            options,
        )?;
    }
    counts.selected_clauses = selected.len();
    counts.selected_clause_characters = selected.iter().map(|item| item.character_count).sum();
    counts.dictionary_queries = dictionary_queries.len();
    counts.selector_dictionary_queries = dictionary_queries.len();
    record_stage(&mut stages, &sampler, "selector_scan", selector_started);
    let lexical_compare_started = Instant::now();
    let dictionary_matches = dictionary.resolve_exact_forms_batch(&dictionary_queries);
    let mut changed_paragraphs = BTreeSet::new();
    let mut selective_changed_clauses = BTreeSet::new();

    for item in selected {
        let old = resolve_dictionary_lexical_candidates_with_policy(
            &item.morphemes,
            item.candidates.clone(),
            &dictionary_matches,
            &item.formations,
            WordFormationOverlapPolicy::RejectNonEqualOverlap,
        );
        let new = resolve_dictionary_lexical_candidates_with_policy(
            &item.morphemes,
            item.candidates,
            &dictionary_matches,
            &item.formations,
            WordFormationOverlapPolicy::RejectCrossingOverlap,
        );
        if accepted_signature(&old.accepted) != accepted_signature(&new.accepted) {
            selective_changed_clauses.insert((item.book_index, item.clause_id));
            changed_paragraphs.insert((item.book_index, item.paragraph_id));
        }
    }
    counts.selected_paragraphs = changed_paragraphs.len();
    record_stage(
        &mut stages,
        &sampler,
        "lexical_compare",
        lexical_compare_started,
    );

    if options.verify_full_domain {
        let oracle_started = Instant::now();
        verify_full_domain(
            options,
            &substrate_manifest,
            &matcher,
            &dictionary,
            &selective_changed_clauses,
        )?;
        record_stage(&mut stages, &sampler, "full_domain_oracle", oracle_started);
    }

    let paragraph_reanalysis_started = Instant::now();
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
            prepare_dictionary_lexical_candidates(&segment.morphemes)
                .extend_queries(&mut observation_queries);
        }
    }
    counts.observation_dictionary_queries = observation_queries.len();
    let observation_matches = dictionary.resolve_exact_forms_batch(&observation_queries);
    let mut changes = Vec::new();
    for paragraph in paragraphs {
        let (mut before_tokens, mut after_tokens) = pipeline
            .process_preanalyzed_dictionary_overlap_pair(
                &paragraph.text,
                &paragraph.annotations,
                &paragraph.content_segments,
                &[],
                &observation_matches,
            )
            .map_err(|error| anyhow!(error))?;
        offset_token_ranges(&mut before_tokens, paragraph.range.start, 0);
        offset_token_ranges(&mut after_tokens, paragraph.range.start, 0);
        let token_changes = classify_token_ranges(&before_tokens, &after_tokens)?;
        counts.ordinal_only_token_propagations += token_changes.ordinal_only.len();
        let changed_ranges = token_changes.visible;
        for (reading_sentence_id, sentence_range, sentence_text) in paragraph.reading_sentences {
            let sentence_changed: Vec<CharRange> = changed_ranges
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
                change_id: "lexical.word_formation_overlap.proper_containment".to_string(),
                primary_domain: "lexical_unit".to_string(),
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
    record_stage(
        &mut stages,
        &sampler,
        "paragraph_reanalysis",
        paragraph_reanalysis_started,
    );
    counts.changed_paragraphs = changes
        .iter()
        .map(|change| (change.book_id.as_str(), change.paragraph_id))
        .collect::<BTreeSet<_>>()
        .len();
    counts.changes = changes.len();
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
            "root_changes": usize::from(!changes.is_empty()),
            "propagated_candidates": counts.ordinal_only_token_propagations,
            "churn_rate": if counts.characters == 0 { 0.0 } else { changes.len() as f64 / counts.characters as f64 },
            "heavy_pipeline_character_rate": if counts.characters == 0 { 0.0 } else { counts.executed_paragraph_characters as f64 / counts.characters as f64 },
            "reading_units": changes.len(),
            "affected_sentences": changes.len(),
            "selection": counts,
            "stages": [
                {"stage": "morpheme", "coverage": 1.0},
                {"stage": "word_formation", "coverage": 1.0},
                {"stage": "lexical_unit", "coverage": 1.0},
                {"stage": "bunsetsu", "coverage": 1.0},
                {"stage": "ui_projection", "coverage": 1.0}
            ],
        }),
    )?;
    transaction.write_json(
        "gate.json",
        &serde_json::json!({
            "status": "passed",
            "complete": true,
            "selector_oracle_status": if options.verify_full_domain { "passed" } else { "not_run" },
            "known_example_status": "passed",
            "known_example_id": "lexical.word_formation_overlap.proper_containment.fixture.v1",
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
            semantic_mode: "reject_non_equal_overlap".to_string(),
            label: "overlap && unequal -> reject".to_string(),
        },
        after: AuditEndpoint {
            revision: options.after_revision.clone(),
            semantic_mode: "reject_crossing_overlap".to_string(),
            label: "crossing overlap -> reject".to_string(),
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
            measurement_scope: "single_process_inline_before_after".to_string(),
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

pub(crate) fn prepare_selected_paragraphs(
    substrate_directory: &Path,
    manifest: &crate::model::SubstrateManifest,
    selected: &BTreeSet<(usize, usize)>,
) -> Result<Vec<SelectedParagraph>> {
    let mut by_book: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for &(book_index, paragraph_id) in selected {
        by_book.entry(book_index).or_default().insert(paragraph_id);
    }
    let mut result = Vec::with_capacity(selected.len());
    for (book_index, paragraph_ids) in by_book {
        let descriptor = manifest
            .books
            .get(book_index)
            .context("选择结果 book_index 越界")?;
        let chunk = read_book_chunk(substrate_directory, descriptor)?;
        let source = fs::read_to_string(&descriptor.source_path)?;
        let analysis_text = compile_analysis_text(&source);
        let prepared = ruby::prepare_text(&analysis_text);
        if sha256_bytes(prepared.text.as_bytes()) != descriptor.normalized_text_sha256
            || prepared.annotations != chunk.ruby_annotations
        {
            bail!("执行时规范文本或 ruby 注释发生漂移：{}", descriptor.book_id);
        }
        let chars: Vec<char> = prepared.text.chars().collect();
        for paragraph_id in paragraph_ids {
            let range = chunk
                .paragraph_ranges
                .get(paragraph_id)
                .copied()
                .context("底座 paragraph_id 越界")?;
            let text: String = chars[range.start..range.end].iter().collect();
            let annotations = chunk
                .ruby_annotations
                .iter()
                .filter(|annotation| {
                    CharRange::new(annotation.char_range.0, annotation.char_range.1)
                        .intersects(range)
                })
                .cloned()
                .map(|mut annotation| {
                    annotation.char_range.0 -= range.start;
                    annotation.char_range.1 -= range.start;
                    annotation
                })
                .collect();
            let content_segments = chunk
                .clauses
                .iter()
                .filter(|clause| clause.paragraph_id == paragraph_id)
                .cloned()
                .map(|clause| {
                    let mut morphemes = clause.morphemes;
                    for morpheme in &mut morphemes {
                        morpheme.char_range.0 -= range.start;
                        morpheme.char_range.1 -= range.start;
                    }
                    PreanalyzedContentSegment {
                        char_range: (
                            clause.range.start - range.start,
                            clause.range.end - range.start,
                        ),
                        morphemes,
                    }
                })
                .collect();
            let reading_sentences = chunk
                .reading_sentence_ranges
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, sentence_range)| sentence_range.intersects(range))
                .map(|(reading_sentence_id, sentence_range)| {
                    (
                        reading_sentence_id,
                        sentence_range,
                        chars[sentence_range.start..sentence_range.end]
                            .iter()
                            .collect(),
                    )
                })
                .collect();
            result.push(SelectedParagraph {
                book_id: descriptor.book_id.clone(),
                paragraph_id,
                range,
                text,
                annotations,
                content_segments,
                reading_sentences,
            });
        }
    }
    Ok(result)
}

fn verify_known_example(pipeline: &Pipeline, dictionary: &DictionaryEngine) -> Result<()> {
    const TEXT: &str = "中学時代は不登校気味で迷惑かけてごめん。";
    let before = pipeline.process_with_dictionary_overlap_policy(
        TEXT,
        &[],
        dictionary,
        WordFormationOverlapPolicy::RejectNonEqualOverlap,
    );
    let after = pipeline.process_with_dictionary_overlap_policy(
        TEXT,
        &[],
        dictionary,
        WordFormationOverlapPolicy::RejectCrossingOverlap,
    );
    let before_has_target = lexical_units(&before)
        .iter()
        .any(|unit| unit.surface == "不登校");
    let after_has_target = lexical_units(&after)
        .iter()
        .any(|unit| unit.surface == "不登校");
    if before_has_target
        || !after_has_target
        || classify_token_ranges(&before, &after)?.visible.is_empty()
    {
        bail!("已知 proper-containment 样例未产生预期 lexical unit 变化");
    }
    Ok(())
}

pub(crate) fn record_stage(
    stages: &mut BTreeMap<String, StageResourceUsage>,
    sampler: &MemorySampler,
    name: &str,
    started: Instant,
) {
    stages.insert(
        name.to_string(),
        StageResourceUsage {
            elapsed_ms: started.elapsed().as_millis(),
            peak_rss_bytes: sampler.take_phase_peak(),
        },
    );
}

fn enforce_final_budget(usage: &ResourceUsage, options: &ContainmentAuditOptions) -> Result<()> {
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

fn verify_full_domain(
    options: &ContainmentAuditOptions,
    manifest: &crate::model::SubstrateManifest,
    matcher: &WordFormationMatcher,
    dictionary: &DictionaryEngine,
    selective_changed: &BTreeSet<(usize, usize)>,
) -> Result<()> {
    let mut all_queries = HashSet::new();
    for descriptor in &manifest.books {
        let chunk = read_book_chunk(&options.substrate_directory, descriptor)?;
        for clause in chunk.clauses {
            prepare_dictionary_lexical_candidates(&clause.morphemes)
                .extend_queries(&mut all_queries);
        }
    }
    let matches = dictionary.resolve_exact_forms_batch(&all_queries);
    let mut full_changed = BTreeSet::new();
    for (book_index, descriptor) in manifest.books.iter().enumerate() {
        let chunk = read_book_chunk(&options.substrate_directory, descriptor)?;
        for clause in chunk.clauses {
            let formations = matcher.match_morphemes(&clause.morphemes).accepted;
            let candidates = prepare_dictionary_lexical_candidates(&clause.morphemes);
            let old = resolve_dictionary_lexical_candidates_with_policy(
                &clause.morphemes,
                candidates.clone(),
                &matches,
                &formations,
                WordFormationOverlapPolicy::RejectNonEqualOverlap,
            );
            let new = resolve_dictionary_lexical_candidates_with_policy(
                &clause.morphemes,
                candidates,
                &matches,
                &formations,
                WordFormationOverlapPolicy::RejectCrossingOverlap,
            );
            if accepted_signature(&old.accepted) != accepted_signature(&new.accepted) {
                full_changed.insert((book_index, clause.clause_id));
            }
        }
    }
    if full_changed != *selective_changed {
        bail!(
            "选择器与全范围 oracle 不等价：selective={} full={}",
            selective_changed.len(),
            full_changed.len()
        );
    }
    Ok(())
}

pub(crate) fn ensure_source_unchanged(
    descriptor: &crate::model::SubstrateBookDescriptor,
) -> Result<()> {
    if sha256_file(&descriptor.source_path)? != descriptor.source_sha256 {
        bail!("书库在底座生成后发生变化：{}", descriptor.book_id);
    }
    Ok(())
}

fn accepted_signature(
    values: &[kotoclip_core::pipeline::lexical::AcceptedDictionaryLexicalUnit],
) -> Vec<(usize, usize, String, String)> {
    let mut result: Vec<_> = values
        .iter()
        .map(|value| {
            (
                value.morpheme_range.0,
                value.morpheme_range.1,
                value.annotation.surface.clone(),
                value.annotation.base_form.clone(),
            )
        })
        .collect();
    result.sort();
    result
}

pub(crate) struct ClassifiedTokenRanges {
    pub visible: Vec<CharRange>,
    pub ordinal_only: Vec<CharRange>,
}

pub(crate) fn classify_token_ranges(
    before: &[AnnotatedToken],
    after: &[AnnotatedToken],
) -> Result<ClassifiedTokenRanges> {
    let mut visible = BTreeSet::new();
    let mut ordinal_only = BTreeSet::new();
    let mut before_by_range = BTreeMap::new();
    let mut after_by_range = BTreeMap::new();
    for token in before {
        let full = serde_json::to_value(token)?;
        before_by_range.insert(
            token.bunsetsu.char_range,
            (full.clone(), visible_token_observation(full)),
        );
    }
    for token in after {
        let full = serde_json::to_value(token)?;
        after_by_range.insert(
            token.bunsetsu.char_range,
            (full.clone(), visible_token_observation(full)),
        );
    }
    for range in before_by_range.keys().chain(after_by_range.keys()) {
        let before_value = before_by_range.get(range);
        let after_value = after_by_range.get(range);
        if before_value.map(|value| &value.1) != after_value.map(|value| &value.1) {
            visible.insert(CharRange::new(range.0, range.1));
        } else if before_value.map(|value| &value.0) != after_value.map(|value| &value.0) {
            ordinal_only.insert(CharRange::new(range.0, range.1));
        }
    }
    Ok(ClassifiedTokenRanges {
        visible: visible.into_iter().collect(),
        ordinal_only: ordinal_only.into_iter().collect(),
    })
}

/// token ordinal 是句内执行布局，不属于阅读器可见身份；字符范围和语义身份仍完整比较。
fn visible_token_observation(mut token: serde_json::Value) -> serde_json::Value {
    if let Some(occurrences) = token
        .pointer_mut("/bunsetsu/grammar_occurrences")
        .and_then(serde_json::Value::as_array_mut)
    {
        for occurrence in occurrences {
            if let Some(object) = occurrence.as_object_mut() {
                object.remove("covered_token_range");
            }
        }
    }
    if let Some(expressions) = token
        .get_mut("expressions")
        .and_then(serde_json::Value::as_array_mut)
    {
        for expression in expressions {
            if let Some(object) = expression.as_object_mut() {
                object.remove("token_range");
            }
        }
    }
    token
}

pub(crate) fn tokens_in_range(tokens: &[AnnotatedToken], range: CharRange) -> Vec<AnnotatedToken> {
    tokens
        .iter()
        .filter(|token| {
            CharRange::new(token.bunsetsu.char_range.0, token.bunsetsu.char_range.1)
                .intersects(range)
        })
        .cloned()
        .collect()
}

pub(crate) fn lexical_units(tokens: &[AnnotatedToken]) -> Vec<DictionaryLexicalUnitAnnotation> {
    tokens
        .iter()
        .flat_map(|token| token.bunsetsu.lexical_units.iter().cloned())
        .collect()
}

fn enforce_runtime_budget(
    started: Instant,
    peak_rss: u64,
    staging: &Path,
    options: &ContainmentAuditOptions,
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

#[cfg(windows)]
fn current_rss_bytes() -> u64 {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        PageFaultCount: 0,
        PeakWorkingSetSize: 0,
        WorkingSetSize: 0,
        QuotaPeakPagedPoolUsage: 0,
        QuotaPagedPoolUsage: 0,
        QuotaPeakNonPagedPoolUsage: 0,
        QuotaNonPagedPoolUsage: 0,
        PagefileUsage: 0,
        PeakPagefileUsage: 0,
    };
    let ok = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    };
    if ok == 0 {
        0
    } else {
        counters.WorkingSetSize as u64
    }
}

#[cfg(not(windows))]
fn current_rss_bytes() -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::visible_token_observation;

    #[test]
    fn token_ordinal_propagation_is_not_a_visible_observation_change() {
        let before = serde_json::json!({
            "bunsetsu": {
                "char_range": [20, 24],
                "grammar_occurrences": [{
                    "occurrence_id": "grammar:20:24",
                    "matched_ranges": [[20, 24]],
                    "covered_token_range": [3, 4]
                }]
            },
            "expressions": [{
                "match_id": "expression:20:24",
                "char_range": [20, 24],
                "matched_ranges": [[20, 24]],
                "token_range": [3, 4]
            }]
        });
        let mut after = before.clone();
        after["bunsetsu"]["grammar_occurrences"][0]["covered_token_range"] =
            serde_json::json!([2, 3]);
        after["expressions"][0]["token_range"] = serde_json::json!([2, 3]);

        assert_ne!(before, after);
        assert_eq!(
            visible_token_observation(before.clone()),
            visible_token_observation(after.clone())
        );

        after["expressions"][0]["char_range"] = serde_json::json!([19, 24]);
        assert_ne!(
            visible_token_observation(before),
            visible_token_observation(after)
        );
    }
}
