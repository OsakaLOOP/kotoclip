//! 来源分析、查询和快照分别加锁，模型推理期间允许前台查词。
use crate::{analysis::ResourcePaths, analysis_cache::AnalysisCache, dictionary::lookup::DictionaryEngine, output, providers::ProviderManager};
use kotoclip_nlp::{model::{MorphemeToken, QueryForm, Register, RegisterRouting, SourceAnalysis, SourceRun, UnifiedDocument}, prepare::PreparedText, sources::UniDicProvider, syntax::SyntaxArtifact};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex, atomic::AtomicU64}, time::Instant};

pub struct AnalysisEngine {
    pub paths: ResourcePaths,
    providers: Mutex<HashMap<Register, UniDicProvider>>,
    dictionary: Mutex<Option<DictionaryEngine>>,
    documents: Mutex<AnalysisCache<UnifiedDocument>>,
    pub external: Mutex<ProviderManager>,
    pub cancellation: Arc<AtomicU64>,
}

impl AnalysisEngine {
    pub fn new(paths: ResourcePaths) -> Self {
        let external = ProviderManager::new(paths.provider_config.clone(), paths.provider_script.clone(), paths.provider_defaults.clone());
        let cancellation = external.cancellation.clone();
        Self { paths, providers: Mutex::new(HashMap::new()), dictionary: Mutex::new(None), documents: Mutex::new(AnalysisCache::new(64 * 1024 * 1024)), external: Mutex::new(external), cancellation }
    }

    pub fn status(&self) -> Value {
        let providers = self.providers.lock().unwrap();
        json!({"schema": kotoclip_nlp::model::SCHEMA, "providers": [
            {"register":"cwj", "available": self.paths.cwj.is_file(), "loaded": providers.contains_key(&Register::Cwj)},
            {"register":"csj", "available": self.paths.csj.is_file(), "loaded": providers.contains_key(&Register::Csj)}]})
    }

    pub fn remember(&self, document: Arc<UnifiedDocument>) {
        self.documents.lock().unwrap().insert(format!("document:{}", document.id), document);
    }

    pub fn document(&self, id: &str) -> Result<Arc<UnifiedDocument>, String> {
        self.documents.lock().unwrap().get(&format!("document:{id}")).ok_or_else(|| "分析结果已释放，请重新请求正文范围".into())
    }

    pub fn analyze(&self, prepared: &PreparedText, routing: RegisterRouting, artifacts: &[SyntaxArtifact],
        grammar: Option<kotoclip_nlp::grammar::GrammarArtifact>, expression: Option<kotoclip_nlp::expression::ExpressionArtifact>,
    ) -> Result<Arc<UnifiedDocument>, String> {
        let started = Instant::now();
        let mut providers = self.providers.lock().unwrap();
        for run in &routing.runs {
            if !providers.contains_key(&run.selected) {
                let path = if run.selected == Register::Cwj { &self.paths.cwj } else { &self.paths.csj };
                providers.insert(run.selected, UniDicProvider::open(run.selected, path)?);
            }
        }
        let resources: Vec<_> = routing.runs.iter().map(|run| providers[&run.selected].metadata()).collect();
        let key = format!("base:{}", kotoclip_nlp::external::text_digest(&serde_json::to_string(&(
            kotoclip_nlp::model::SCHEMA, &prepared.mapping.source_sha256, &routing, resources, artifacts, &grammar, &expression,
        )).map_err(|e| e.to_string())?));
        let cached = self.documents.lock().unwrap().get(&key);
        if let Some(document) = cached {
            drop(providers);
            self.remember(document.clone());
            return Ok(document);
        }
        let offsets: Vec<_> = prepared.text.char_indices().map(|(i, _)| i).chain(std::iter::once(prepared.text.len())).collect();
        let mut source = SourceAnalysis { runs: Vec::new(), tokens: Vec::new() };
        for run in &routing.runs {
            let [start, end] = run.char_range;
            let byte_start = offsets[start];
            let part = providers[&run.selected].analyze(&prepared.text[byte_start..offsets[end]])?;
            let token_start = source.tokens.len();
            source.runs.push(SourceRun { provider: part.runs[0].provider.clone(), char_range: run.char_range, token_range: [token_start, token_start + part.tokens.len()] });
            for mut token in part.tokens {
                token.index = source.tokens.len();
                token.char_range = [start + token.char_range[0], start + token.char_range[1]];
                token.byte_range = [byte_start + token.byte_range[0], byte_start + token.byte_range[1]];
                source.tokens.push(token);
            }
        }
        drop(providers);
        let mut document = kotoclip_nlp::unify::unify_with_external(prepared, source, routing, artifacts)?;
        document.expression = crate::expression_catalog::collect(&document.text, &document.morphemes)?;
        if let Some(value) = grammar {
            kotoclip_nlp::grammar::validate(&value, &document.text, &document.morphemes)?;
            document.grammar = kotoclip_nlp::grammar::merge(document.grammar, value);
        }
        if let Some(value) = expression {
            kotoclip_nlp::expression::validate(&value, &document.text, &document.morphemes)?;
            document.expression = kotoclip_nlp::expression::merge(document.expression, value);
        }
        document.projection = kotoclip_nlp::projection::from_layers(&document.grammar, &document.expression);
        document.elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        let document = Arc::new(document);
        self.documents.lock().unwrap().insert(key, document.clone());
        self.remember(document.clone());
        Ok(document)
    }

    pub fn enrich(&self, document: &UnifiedDocument, generation: u64) -> Result<(Arc<UnifiedDocument>, Vec<Value>), String> {
        let (sources, diagnostics) = self.external.lock().unwrap().analyze(&document.text, generation);
        let prepared = PreparedText { text: document.text.clone(), annotations: document.author_ruby.clone(), mapping: document.preparation.clone() };
        let mut updated = kotoclip_nlp::unify::unify_with_sources(&prepared, document.source.clone(), document.routing.clone(), &sources)?;
        updated.ruby_validations = document.ruby_validations.clone();
        updated.grammar = document.grammar.clone();
        updated.expression = document.expression.clone();
        updated.projection = kotoclip_nlp::projection::from_layers(&updated.grammar, &updated.expression);
        updated.elapsed_ms = document.elapsed_ms;
        let updated = Arc::new(updated);
        self.remember(updated.clone());
        Ok((updated, diagnostics))
    }

    pub fn query(&self, analysis_id: Option<String>, token: Option<&MorphemeToken>, forms: &[QueryForm], selected_form: Option<&str>) -> Result<Value, String> {
        let mut dictionary = self.dictionary.lock().unwrap();
        if dictionary.is_none() { *dictionary = Some(DictionaryEngine::prepare(&self.paths.dictionary_sources, &self.paths.dictionaries).map_err(|e| e.to_string())?); }
        serde_json::to_value(output::query(dictionary.as_ref().unwrap(), analysis_id, token, forms, selected_form)).map_err(|e| e.to_string())
    }
}
