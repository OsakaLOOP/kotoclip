use crate::model::*;
use crate::prepare::PreparedText;
use crate::ruby::validate_ruby;
use crate::structure::StructureProvider;
use sha2::{Digest, Sha256};

pub fn unify(
    prepared: &PreparedText,
    source: SourceAnalysis,
    routing: RegisterRouting,
) -> Result<UnifiedDocument, String> {
    unify_with_external(prepared, source, routing, &[])
}

/// 原生 provider 接收预处理后的原文；UniDic 只在结构合并后映射范围和查询字段。
pub fn unify_with_native<P: crate::native::NativeStructureProvider>(
    prepared: &PreparedText,
    source: SourceAnalysis,
    routing: RegisterRouting,
    provider: &P,
) -> Result<UnifiedDocument, String> {
    let mut document = unify(prepared, source, routing)?;
    let input = crate::native::NativeProviderInput::from_document(&document);
    let artifact = provider.analyze(input).map_err(|diagnostic| {
        serde_json::to_string(&diagnostic).unwrap_or_else(|_| "native provider failed".into())
    })?;
    let (structure, diagnostics) = crate::structure::merge_external(document.structure, &artifact, &document.text)?;
    document.structure = structure;
    document.structure_diagnostics.extend(diagnostics);
    document.provider_token_alignments.push(crate::alignment::align_artifact(&artifact, &document.morphemes));
    document.formation = crate::formation::collect_formations(&document.text, &document.morphemes, &document.structure)?;
    document.bunsetsu = crate::bunsetsu::collect_bunsetsu(&document.text, &document.morphemes, &document.structure, &document.formation)?;
    document.clause = crate::clause::collect_clauses(&document.text, &document.morphemes, &document.structure)?;
    document.dictionary_candidates = crate::lexical::collect_dictionary_candidates(&document.text, &document.morphemes, &document.formation)?;
    Ok(document)
}

/// 在统一词元结果上追加外部结构证据；每个 provider 的范围和对齐诊断保持可追溯。
pub fn unify_with_external(
    prepared: &PreparedText,
    source: SourceAnalysis,
    routing: RegisterRouting,
    external: &[crate::syntax::SyntaxArtifact],
) -> Result<UnifiedDocument, String> {
    let text = &prepared.text;
    let chars: Vec<char> = text.chars().collect();
    let id = format!(
        "{:x}",
        Sha256::digest(format!(
            "{SCHEMA}\0{}\0{text}",
            source.provider.dictionary_sha256
        ))
    );
    let mut gaps = Vec::new();
    let mut morphemes = Vec::new();
    let mut end = 0;
    for token in &source.tokens {
        let [start, next] = token.char_range;
        if start < end
            || next <= start
            || next > chars.len()
            || chars[start..next].iter().collect::<String>() != token.surface
        {
            return Err(format!("来源 token {} 的字符范围与原文不符", token.index));
        }
        if start > end {
            gaps.push(TextGap {
                char_range: [end, start],
                surface: chars[end..start].iter().collect(),
            });
        }
        let value = |index: usize| token.fields[index].value.clone();
        let mut forms: Vec<QueryForm> = Vec::new();
        for (kind, form, reading, field) in [
            ("observed", Some(token.surface.clone()), value(20), "kana"),
            ("base", value(10), value(21), "kanaBase"),
            ("lemma", value(7), value(6), "lForm"),
        ] {
            if let Some(form) = form.filter(|f| !f.trim().is_empty()) {
                if !forms.iter().any(|f| f.form == form) {
                    forms.push(QueryForm {
                        kind: kind.into(),
                        form,
                        reading_field: reading.as_ref().map(|_| field.into()),
                        reading,
                    });
                }
            }
        }
        morphemes.push(MorphemeToken {
            id: format!("m{}", token.index),
            source_index: token.index,
            surface: token.surface.clone(),
            char_range: token.char_range,
            pos: std::array::from_fn(value),
            lemma: value(7),
            reading: value(20),
            query_forms: forms,
        });
        end = next;
    }
    if end < chars.len() {
        gaps.push(TextGap {
            char_range: [end, chars.len()],
            surface: chars[end..].iter().collect(),
        });
    }
    let ruby_validations = validate_ruby(&chars, &source.tokens, &prepared.annotations);
    let mut structure = crate::structure::LocalBoundaryProvider.analyze(text);
    let mut structure_diagnostics = Vec::new();
    let mut provider_token_alignments = Vec::new();
    for artifact in external {
        let (merged, diagnostics) = crate::structure::merge_external(structure, artifact, text)?;
        structure = merged;
        structure_diagnostics.extend(diagnostics);
        provider_token_alignments.push(crate::alignment::align_artifact(artifact, &morphemes));
    }
    let formation = crate::formation::collect_formations(text, &morphemes, &structure)?;
    let bunsetsu = crate::bunsetsu::collect_bunsetsu(text, &morphemes, &structure, &formation)?;
    let clause = crate::clause::collect_clauses(text, &morphemes, &structure)?;
    let dictionary_candidates = crate::lexical::collect_dictionary_candidates(text, &morphemes, &formation)?;
    let grammar = crate::grammar::collect_functional_candidates(&source.tokens, &morphemes)?;
    let expression = crate::expression::empty();
    let projection = crate::projection::from_layers(&grammar, &expression);
    let morphology = crate::morphology::collect(&source.tokens, &morphemes)?;
    Ok(UnifiedDocument {
        schema: SCHEMA.into(),
        id,
        text: text.into(),
        characters: chars.len(),
        source,
        routing,
        ruby_validations,
        morphemes,
        gaps,
        structure,
        formation,
        bunsetsu,
        clause,
        dictionary_candidates,
        grammar,
        expression,
        projection,
        morphology,
        structure_diagnostics,
        provider_token_alignments,
        external_sources: Vec::new(),
        elapsed_ms: 0.0,
    })
}
