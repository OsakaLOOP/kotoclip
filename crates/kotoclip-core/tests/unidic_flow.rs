use kotoclip_core::analysis::{AnalysisService, Request, ResourcePaths};
use kotoclip_nlp::model::{Register, UnifiedDocument};
use kotoclip_nlp::syntax::{SyntaxArtifact, SyntaxProviderDescriptor, SyntaxSpan};
use std::path::PathBuf;

#[test]
fn real_resources_preserve_ranges_fields_and_query_targets() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut service = AnalysisService::new(ResourcePaths::development(&root));
    for register in [Register::Cwj, Register::Csj] {
        let response = service.dispatch(Request::Analyze {
            register,
            text: "  警察へ向かった。\r\n𠮷野は云う。\t".into(),
        });
        assert!(response.error.is_none(), "{:?}", response.error);
        let doc: UnifiedDocument = serde_json::from_value(response.result.unwrap()).unwrap();
        assert_eq!(doc.morphology.schema, kotoclip_nlp::morphology::SCHEMA);
        assert!(!doc.morphology.chains.is_empty());
        assert!(doc.grammar.occurrences.iter().all(|item| item.status == kotoclip_nlp::grammar::GrammarStatus::Candidate));
        assert!(doc.projection.targets.iter().all(|item| item.layer == "grammar"));
        let chars: Vec<char> = doc.text.chars().collect();
        let mut coverage = vec![0; chars.len()];
        for source in &doc.source.tokens {
            assert_eq!(source.fields.len(), 29);
            assert_eq!(
                chars[source.char_range[0]..source.char_range[1]]
                    .iter()
                    .collect::<String>(),
                source.surface
            );
            assert_eq!(
                &doc.text[source.byte_range[0]..source.byte_range[1]],
                source.surface
            );
            for count in &mut coverage[source.char_range[0]..source.char_range[1]] {
                *count += 1;
            }
        }
        for gap in &doc.gaps {
            for count in &mut coverage[gap.char_range[0]..gap.char_range[1]] {
                *count += 1;
            }
        }
        assert!(coverage.iter().all(|c| *c == 1));
        let police = doc.morphemes.iter().find(|t| t.surface == "警察").unwrap();
        let raw = &doc.source.tokens[police.source_index];
        assert_eq!(raw.fields[9].value.as_deref(), Some("ケーサツ"));
        assert_eq!(police.query_forms[0].reading.as_deref(), Some("ケイサツ"));
        let verb = doc
            .morphemes
            .iter()
            .find(|t| t.surface == "向かっ")
            .unwrap();
        let base = verb
            .query_forms
            .iter()
            .find(|q| q.form == "向かう")
            .unwrap();
        assert_eq!(base.reading.as_deref(), Some("ムカウ"));
        let lookup = service.dispatch(Request::Query {
            analysis_id: doc.id.clone(),
            token_id: police.id.clone(),
            selected_form: None,
        });
        assert!(lookup.error.is_none(), "{:?}", lookup.error);
        assert!(lookup.result.unwrap()["groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| !g["entries"].as_array().unwrap().is_empty()));
        let invalid = service.dispatch(Request::Query {
            analysis_id: doc.id,
            token_id: "missing".into(),
            selected_form: None,
        });
        assert!(invalid.error.is_some());
    }
}

#[test]
fn ruby_validation_and_automatic_routing_preserve_actual_source_runs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut service = AnalysisService::new(ResourcePaths::development(&root));

    let response = service.dispatch(Request::Analyze {
        register: Register::Cwj,
        text: "煙草《たばこ》と古《ふる》川《かわ》".into(),
    });
    assert!(response.error.is_none(), "{:?}", response.error);
    let document: UnifiedDocument = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(document.text, "煙草と古川");
    assert_eq!(document.ruby_validations.len(), 2);
    assert!(document
        .ruby_validations
        .iter()
        .all(|validation| validation.status == "matched"));
    let old_river = document
        .ruby_validations
        .iter()
        .find(|validation| validation.base == "古川")
        .unwrap();
    assert_eq!(old_river.ruby_reading, "フルカワ");
    assert_eq!(
        old_river.token_range.map(|range| range[1] - range[0]),
        Some(1)
    );

    let response = service.dispatch(Request::AnalyzeRouted {
        policy: kotoclip_nlp::routing::RegisterPolicy::Auto,
        text: "𠮷野は「これはとても長い会話文を含んでいます」と言った。".into(),
    });
    assert!(response.error.is_none(), "{:?}", response.error);
    let document: UnifiedDocument = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(document.routing.requested, kotoclip_nlp::routing::RegisterPolicy::Auto);
    assert_eq!(document.routing.selected, None);
    assert_eq!(document.source.runs.len(), 3);
    assert!(document.source.runs[0].provider.id.contains("cwj"));
    assert!(document.source.runs[1].provider.id.contains("csj"));
    assert!(document.source.runs[2].provider.id.contains("cwj"));
    for token in &document.source.tokens {
        assert_eq!(&document.text[token.byte_range[0]..token.byte_range[1]], token.surface);
        assert_eq!(document.text.chars().skip(token.char_range[0]).take(token.char_range[1] - token.char_range[0]).collect::<String>(), token.surface);
    }
}

#[test]
fn ruby_validation_accepts_full_size_digraphs_and_okurigana() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut service = AnalysisService::new(ResourcePaths::development(&root));
    for text in ["驚愕《キヨウガク》", "可愛《カワイ》らしい"] {
        let response = service.dispatch(Request::Analyze {
            register: Register::Cwj,
            text: text.into(),
        });
        assert!(response.error.is_none(), "{:?}", response.error);
        let document: UnifiedDocument = serde_json::from_value(response.result.unwrap()).unwrap();
        assert_eq!(document.ruby_validations.len(), 1, "{text}");
        let validation = &document.ruby_validations[0];
        assert_eq!(validation.status, "matched", "{text}: {validation:?}");
        assert_eq!(
            validation.observed_reading.as_deref(),
            Some(if text.starts_with("驚") {
                "キョウガク"
            } else {
                "カワイ"
            })
        );
    }
}

#[test]
fn ruby_validation_resolves_partial_compounds_and_reports_reading_variants() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut service = AnalysisService::new(ResourcePaths::development(&root));
    let response = service.dispatch(Request::Analyze {
        register: Register::Cwj,
        text: "産業廃《はい》棄《き》物《ぶつ》・死体喰《く》う・覗《のぞ》き見る・喰い神《がみ》"
            .into(),
    });
    assert!(response.error.is_none(), "{:?}", response.error);
    let document: UnifiedDocument = serde_json::from_value(response.result.unwrap()).unwrap();

    let waste = document
        .ruby_validations
        .iter()
        .find(|validation| validation.base == "廃棄物")
        .unwrap();
    assert_eq!(waste.status, "matched");
    assert_eq!(waste.observed_reading.as_deref(), Some("ハイキブツ"));

    let corpse = document
        .ruby_validations
        .iter()
        .find(|validation| validation.base == "喰")
        .unwrap();
    assert_eq!(corpse.status, "matched");

    let peek = document
        .ruby_validations
        .iter()
        .find(|validation| validation.base == "覗")
        .unwrap();
    assert_eq!(peek.status, "matched");

    let god = document
        .ruby_validations
        .iter()
        .find(|validation| validation.base == "神")
        .unwrap();
    assert_eq!(god.status, "variant");
    assert_eq!(god.observed_reading.as_deref(), Some("カミ"));
}

#[test]
fn analyze_with_artifacts_preserves_bunsetsu_head_mapping() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut service = AnalysisService::new(ResourcePaths::development(&root));
    let artifact = SyntaxArtifact {
        schema: kotoclip_nlp::syntax::SCHEMA.into(),
        segment_id: Some("fixture".into()),
        provider: SyntaxProviderDescriptor {
            id: "ginza".into(), version: Some("5.2.1".into()),
            capabilities: vec!["bunsetsu".into()], license: Some("MIT".into()),
        },
        text_characters: 8,
        text_sha256: kotoclip_nlp::external::text_digest("警察へ向かった。"),
        spans: vec![
            SyntaxSpan {
                id: "b0".into(), kind: "bunsetsu".into(), char_range: [0, 3],
                head_char_range: Some([0, 2]), source_id: "fixture".into(),
                surface: Some("警察へ".into()), labels: vec!["nominal".into()],
            },
            SyntaxSpan {
                id: "c0".into(), kind: "compound".into(), char_range: [0, 2],
                head_char_range: None, source_id: "fixture".into(),
                surface: Some("警察".into()), labels: Vec::new(),
            },
        ],
    };
    let response = service.dispatch(Request::AnalyzeWithArtifacts {
        register: Register::Cwj, text: "警察へ向かった。".into(), artifacts: vec![artifact], grammar: None, expression: None,
    });
    assert!(response.error.is_none(), "{:?}", response.error);
    let document: UnifiedDocument = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(document.bunsetsu.nodes.len(), 1);
    assert_eq!(document.bunsetsu.nodes[0].head_morpheme_index, Some(0));
    assert_eq!(document.bunsetsu.nodes[0].labels, vec!["nominal", "alignment:complete_morphemes"]);
    assert!(!document.clause.sentences.is_empty());
    assert!(!document.clause.clauses.is_empty());
    assert!(document.structure_diagnostics.iter().all(|item| item.status == "aligned"));
    let candidate = document.dictionary_candidates.candidates.iter().find(|item| item.kind == "compound").unwrap();
    assert_eq!(candidate.query_forms[0].reading.as_deref(), Some("ケイサツ"));
    let lookup = service.dispatch(Request::QueryCandidate { analysis_id: document.id, candidate_id: candidate.id.clone(), selected_form: None });
    assert!(lookup.error.is_none(), "{:?}", lookup.error);
    assert!(lookup.result.unwrap()["groups"].as_array().unwrap().iter().any(|group| !group["entries"].as_array().unwrap().is_empty()));
}
