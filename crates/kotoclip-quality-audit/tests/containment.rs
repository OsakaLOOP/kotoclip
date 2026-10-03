use flate2::read::GzDecoder;
use kotoclip_core::dictionary::lookup::DictionaryEngine;
use kotoclip_core::pipeline::lexical::WordFormationOverlapPolicy;
use kotoclip_core::pipeline::Pipeline;
use kotoclip_quality_audit::{
    build_substrate, publish_history, run_containment_audit, BuildSubstrateOptions,
    ContainmentAuditOptions,
};
use serde_json::Value;
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;

fn reading_projection(tokens: &[kotoclip_core::models::AnnotatedToken]) -> Value {
    serde_json::json!(tokens
        .iter()
        .map(|token| serde_json::json!({
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
        }))
        .collect::<Vec<_>>())
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .unwrap()
        .to_path_buf()
}

#[test]
fn real_containment_audit_matches_full_domain_and_writes_ui_bundle() {
    let repository = repository_root();
    let system_dictionary = repository.join("ipadic/system.dic");
    let dictionary_directory = repository.join("data/dicts");
    if !system_dictionary.is_file() || !dictionary_directory.is_dir() {
        eprintln!("测试跳过：缺少真实 IPADIC 或词典目录");
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("book.md");
    fs::write(
        &source_path,
        "---\ntitle: 审计样例\n---\n\n## 目次\n\n- [[#プロローグ]]\n\n## プロローグ\n\n中学《ちゅうがく》時代は不登校気味で迷惑かけてごめん。\n\n![插图](./cover.jpeg)\n\n電子書籍を読む。\n",
    )
    .unwrap();
    let corpus_path = temp.path().join("corpus.json");
    fs::write(
        &corpus_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": "kotoclip.quality.corpus.v1",
            "books": [{"book_id": "fixture", "path": source_path}],
        }))
        .unwrap(),
    )
    .unwrap();
    let substrate = build_substrate(&BuildSubstrateOptions {
        corpus_spec: corpus_path,
        system_dictionary: system_dictionary.clone(),
        output_root: temp.path().join("substrates"),
        max_bytes: 64 * 1024 * 1024,
    })
    .unwrap();
    let substrate_manifest: Value =
        serde_json::from_slice(&fs::read(substrate.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        substrate_manifest["fingerprint"]["analysis_text_protocol"],
        kotoclip_core::reader_markdown::ANALYSIS_TEXT_PROTOCOL_VERSION
    );
    let output = run_containment_audit(&ContainmentAuditOptions {
        repository_root: repository,
        substrate_directory: substrate,
        system_dictionary,
        dictionary_directory,
        output_directory: temp.path().join("result"),
        before_revision: "fixture-before".to_string(),
        after_revision: "fixture-after".to_string(),
        verify_full_domain: true,
        max_elapsed: Duration::from_secs(60),
        max_peak_rss_bytes: 1024 * 1024 * 1024,
        max_temporary_bytes: 64 * 1024 * 1024,
        max_artifact_bytes: 64 * 1024 * 1024,
    })
    .unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(output.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["complete"], true);
    assert_eq!(manifest["n_best_in_scope"], false);
    assert!(manifest["counts"]["changes"].as_u64().unwrap() >= 1);
    assert!(output.join("reading-index.json.gz").is_file());
    assert!(output.join("reading-units.bin").is_file());
    assert!(output.join("changes.jsonl.gz").is_file());

    let mut change_lines = String::new();
    GzDecoder::new(File::open(output.join("changes.jsonl.gz")).unwrap())
        .read_to_string(&mut change_lines)
        .unwrap();
    let first_change: Value = serde_json::from_str(change_lines.lines().next().unwrap()).unwrap();
    assert_eq!(
        first_change["sentence_text"],
        "中学時代は不登校気味で迷惑かけてごめん。"
    );
    assert!(!change_lines.contains("[[#プロローグ]]"));
    assert!(!change_lines.contains("## プロローグ"));
    let pipeline = Pipeline::new(repository_root().join("ipadic/system.dic")).unwrap();
    let dictionary = DictionaryEngine::new(&repository_root().join("data/dicts")).unwrap();
    let before = pipeline.process_with_dictionary_overlap_policy(
        "中学《ちゅうがく》時代は不登校気味で迷惑かけてごめん。",
        &[],
        &dictionary,
        WordFormationOverlapPolicy::RejectNonEqualOverlap,
    );
    let after = pipeline.process_with_dictionary_overlap_policy(
        "中学《ちゅうがく》時代は不登校気味で迷惑かけてごめん。",
        &[],
        &dictionary,
        WordFormationOverlapPolicy::RejectCrossingOverlap,
    );
    let mut before = before;
    let mut after = after;
    for tokens in [&mut before, &mut after] {
        kotoclip_core::pipeline::expressions::apply_builtin_expressions(tokens);
        kotoclip_core::pipeline::expressions::apply_correlative_expressions(tokens);
        kotoclip_core::pipeline::expressions::resolve_expression_conflicts(tokens);
        kotoclip_core::pipeline::expressions::stabilize_expression_ids(tokens);
        kotoclip_core::document::offset_token_ranges(
            tokens,
            first_change["char_range"]["start"].as_u64().unwrap() as usize,
            0,
        );
    }
    let reading_index: Value = serde_json::from_reader(GzDecoder::new(
        File::open(output.join("reading-index.json.gz")).unwrap(),
    ))
    .unwrap();
    let first = &reading_index["units"][0];
    let mut bundle = File::open(output.join("reading-units.bin")).unwrap();
    bundle
        .seek(SeekFrom::Start(first["offset"].as_u64().unwrap()))
        .unwrap();
    let mut compressed = vec![0; first["bytes"].as_u64().unwrap() as usize];
    bundle.read_exact(&mut compressed).unwrap();
    let member: Value = serde_json::from_reader(GzDecoder::new(compressed.as_slice())).unwrap();
    let member_index = first["member_index"].as_u64().unwrap() as usize;
    assert_eq!(member[member_index]["unit_id"], first["unit_id"]);
    assert_eq!(first_change["reading_unit_id"], first["unit_id"]);
    assert!(first_change.get("before_tokens").is_none());
    assert_eq!(
        member[member_index]["before"]["tokens"],
        reading_projection(&before)
    );
    assert_eq!(
        member[member_index]["after"]["tokens"],
        reading_projection(&after)
    );
    assert_eq!(
        member[member_index]["before"]["text"],
        first["before"]["text"]
    );

    let history_path = publish_history(temp.path(), &output).unwrap();
    let history: Value = serde_json::from_reader(File::open(history_path).unwrap()).unwrap();
    assert_eq!(history["comparisons"][0]["comparison_id"], "result");
    assert_eq!(history["comparisons"][0]["gate_status"], "passed");
}

#[test]
fn zero_corpus_changes_still_passes_independent_known_example_gate() {
    let repository = repository_root();
    let system_dictionary = repository.join("ipadic/system.dic");
    let dictionary_directory = repository.join("data/dicts");
    if !system_dictionary.is_file() || !dictionary_directory.is_dir() {
        eprintln!("测试跳过：缺少真实 IPADIC 或词典目录");
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("book.md");
    fs::write(&source_path, "電子書籍を読む。\n").unwrap();
    let corpus_path = temp.path().join("corpus.json");
    fs::write(
        &corpus_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": "kotoclip.quality.corpus.v1",
            "books": [{"book_id": "unchanged", "path": source_path}],
        }))
        .unwrap(),
    )
    .unwrap();
    let substrate = build_substrate(&BuildSubstrateOptions {
        corpus_spec: corpus_path,
        system_dictionary: system_dictionary.clone(),
        output_root: temp.path().join("substrates"),
        max_bytes: 64 * 1024 * 1024,
    })
    .unwrap();
    let output = run_containment_audit(&ContainmentAuditOptions {
        repository_root: repository,
        substrate_directory: substrate,
        system_dictionary,
        dictionary_directory,
        output_directory: temp.path().join("result"),
        before_revision: "fixture-before".to_string(),
        after_revision: "fixture-after".to_string(),
        verify_full_domain: false,
        max_elapsed: Duration::from_secs(60),
        max_peak_rss_bytes: 1024 * 1024 * 1024,
        max_temporary_bytes: 64 * 1024 * 1024,
        max_artifact_bytes: 64 * 1024 * 1024,
    })
    .unwrap();
    let gate: Value =
        serde_json::from_reader(File::open(output.join("gate.json")).unwrap()).unwrap();
    let summary: Value =
        serde_json::from_reader(File::open(output.join("summary.json")).unwrap()).unwrap();
    assert_eq!(gate["status"], "passed");
    assert_eq!(gate["known_example_status"], "passed");
    assert_eq!(gate["selector_oracle_status"], "not_run");
    assert_eq!(summary["changes"], 0);
}
