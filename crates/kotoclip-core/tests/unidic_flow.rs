use kotoclip_core::analysis::{AnalysisService, Request, ResourcePaths};
use kotoclip_nlp::model::{Register, UnifiedDocument};
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
        });
        assert!(invalid.error.is_some());
    }
}

#[test]
fn ruby_validation_uses_the_complete_token_span_and_long_dialogue_routes_to_csj() {
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

    let response = service.dispatch(Request::Analyze {
        register: Register::Cwj,
        text: "「これはとても長い会話文を含んでいます」".into(),
    });
    assert!(response.error.is_none(), "{:?}", response.error);
    let document: UnifiedDocument = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(document.routing.requested, Register::Cwj);
    assert_eq!(document.routing.selected, Register::Csj);
    assert_eq!(document.routing.reason.as_deref(), Some("long_dialogue"));
    assert!(document.source.provider.id.contains("csj"));
}
