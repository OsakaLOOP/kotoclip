//! 来源分析、查询和快照分别加锁，模型推理期间允许前台查词。
use crate::{analysis::ResourcePaths, analysis_cache::AnalysisCache, dictionary::lookup::DictionaryEngine, document_plan::DocumentPlan, output, providers::ProviderManager, rule_store::{RuleSnapshot, RuleStore}};
use kotoclip_nlp::{model::{MorphemeToken, QueryForm, Register, RegisterRouting, SourceAnalysis, SourceRun, StageTiming, UnifiedDocument}, prepare::PreparedText, sources::UniDicProvider, syntax::SyntaxArtifact};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::AtomicU64}, time::{Instant, UNIX_EPOCH}};

pub struct AnalysisEngine {
    pub paths: ResourcePaths,
    providers: Mutex<HashMap<Register, UniDicProvider>>,
    dictionary: Mutex<Option<DictionaryEngine>>,
    documents: Mutex<AnalysisCache<UnifiedDocument>>,
    complete_units: Mutex<AnalysisCache<CompleteUnit>>,
    pub external: Mutex<ProviderManager>,
    pub rules: RuleStore,
    pub cancellation: Arc<AtomicU64>,
}

#[derive(Serialize, Deserialize)]
struct CompleteUnit {
    document: Arc<UnifiedDocument>,
    lookup: Value,
}

impl AnalysisEngine {
    pub fn new(paths: ResourcePaths) -> Self {
        let external = ProviderManager::new(paths.provider_config.clone(), paths.provider_script.clone(), paths.provider_defaults.clone());
        let cancellation = external.cancellation.clone();
        let rules = RuleStore::open(paths.provider_config.with_file_name("language-rules.json"));
        Self { paths, providers: Mutex::new(HashMap::new()), dictionary: Mutex::new(None), documents: Mutex::new(AnalysisCache::new(64 * 1024 * 1024)), complete_units: Mutex::new(AnalysisCache::new(128 * 1024 * 1024)), external: Mutex::new(external), rules, cancellation }
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
        stage_timings: Vec<StageTiming>,
    ) -> Result<Arc<UnifiedDocument>, String> {
        let mut stage_timings = stage_timings;
        let started = Instant::now();
        let unidic_started = Instant::now();
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
        drop(providers);
        let source = self.analyze_source(prepared, &routing)?;
        stage_timings.push(StageTiming::new("unidic", unidic_started));
        let unify_started = Instant::now();
        let mut document = kotoclip_nlp::unify::unify_with_external(prepared, source, routing, artifacts)?;
        if let Some(value) = grammar {
            kotoclip_nlp::grammar::validate(&value, &document.text, &document.morphemes)?;
            document.grammar = kotoclip_nlp::grammar::merge(document.grammar, value);
        }
        if let Some(value) = expression {
            kotoclip_nlp::expression::validate(&value, &document.text, &document.morphemes)?;
            document.expression = kotoclip_nlp::expression::merge(document.expression, value);
        }
        stage_timings.push(StageTiming::new("unify", unify_started));
        // P4 分析只生成来源级证据；规则扫描和词典绑定由后置交互显式触发。
        document.stage_timings = stage_timings;
        document.elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        let document = Arc::new(document);
        self.documents.lock().unwrap().insert(key, document.clone());
        self.remember(document.clone());
        Ok(document)
    }

    fn analyze_source(&self, prepared: &PreparedText, routing: &RegisterRouting) -> Result<SourceAnalysis, String> {
        let mut providers = self.providers.lock().unwrap();
        for run in &routing.runs {
            if !providers.contains_key(&run.selected) {
                let path = if run.selected == Register::Cwj { &self.paths.cwj } else { &self.paths.csj };
                providers.insert(run.selected, UniDicProvider::open(run.selected, path)?);
            }
        }
        let offsets: Vec<_> = prepared.text.char_indices().map(|(index, _)| index).chain(std::iter::once(prepared.text.len())).collect();
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
        Ok(source)
    }

    fn complete_cache_digest(&self, prepared: &PreparedText, routing: &RegisterRouting, range: [usize; 2]) -> Result<String, String> {
        let settings = self.external.lock().unwrap().settings()?;
        let resources: Vec<_> = [&self.paths.cwj, &self.paths.csj, &self.paths.provider_script, &settings.ginza.python, &settings.ginza.dictionary]
            .iter().map(|path| resource_identity(path)).collect();
        let mut dictionaries = fs::read_dir(&self.paths.dictionary_sources).ok().into_iter().flatten()
            .filter_map(Result::ok).map(|entry| resource_identity(&entry.path())).collect::<Vec<_>>();
        dictionaries.sort_by_key(Value::to_string);
        Ok(kotoclip_nlp::external::text_digest(&serde_json::to_string(&(
            "kotoclip.complete-unit.v2", kotoclip_nlp::model::SCHEMA, env!("CARGO_PKG_VERSION"),
            &prepared.mapping.source_sha256, routing, range, self.external.lock().unwrap().cache_identity()?, resources, dictionaries,
        )).map_err(|e| e.to_string())?))
    }

    pub(crate) fn cached_units(&self, plan: &DocumentPlan) -> Vec<bool> {
        plan.units.iter().enumerate().map(|(index, unit)| {
            let (prepared, routing) = plan.unit_input(index);
            let range = [unit.anchor.char_range[0] - unit.context_range[0], unit.anchor.char_range[1] - unit.context_range[0]];
            let Ok(digest) = self.complete_cache_digest(&prepared, &routing, range) else { return false; };
            self.complete_units.lock().unwrap().get(&format!("complete:{digest}")).is_some()
                || self.complete_cache_path(&digest).is_file()
        }).collect()
    }

    pub fn analyze_complete(&self, prepared: &PreparedText, routing: RegisterRouting, range: [usize; 2], generation: u64) -> Result<(Arc<UnifiedDocument>, Value, Vec<Value>, bool, bool), String> {
        let started = Instant::now();
        let timing_enabled = self.external.lock().unwrap().analysis_timing_enabled()?;
        let digest = self.complete_cache_digest(prepared, &routing, range)?;
        let cache_key = format!("complete:{digest}");
        let memory_cached = self.complete_units.lock().unwrap().get(&cache_key);
        let cached = memory_cached.or_else(|| self.read_complete_cache(&digest, prepared));
        if let Some(unit) = cached {
            let providers = unit.document.external_sources.iter().map(|source| json!({
                "id": source.provider.id, "status": "ready", "cache_hit": true,
            })).collect();
            return Ok((self.document_for_timing(unit.document.clone(), timing_enabled), unit.lookup.clone(), providers, timing_enabled, true));
        }
        let unidic_started = Instant::now();
        let source = self.analyze_source(prepared, &routing)?;
        let unidic_timing = StageTiming::new("unidic", unidic_started);
        let ginza_started = Instant::now();
        let (sources, diagnostics) = self.external.lock().unwrap().analyze(&prepared.text, generation);
        if diagnostics.iter().any(|item| item["status"] != "ready") || sources.is_empty() {
            return Err(format!("GiNZA 来源分析未完成：{diagnostics:?}"));
        }
        let ginza_timing = StageTiming::new("ginza", ginza_started);
        let unify_started = Instant::now();
        let mut value = kotoclip_nlp::unify::unify_with_sources(prepared, source, routing, &sources)?;
        let unify_timing = StageTiming::new("unify", unify_started);
        let lookup_started = Instant::now();
        let lookup = self.lookup_targets(&value, range)?;
        let lookup_timing = StageTiming::new("lookup", lookup_started);
        value.stage_timings = vec![unidic_timing, ginza_timing, unify_timing, lookup_timing, StageTiming::new("complete", started)];
        value.elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        let document = Arc::new(value);
        let unit = Arc::new(CompleteUnit { document: document.clone(), lookup: lookup.clone() });
        self.complete_units.lock().unwrap().insert(cache_key, unit.clone());
        self.write_complete_cache(&digest, &unit);
        Ok((self.document_for_timing(document, timing_enabled), lookup, diagnostics, timing_enabled, false))
    }

    fn complete_cache_path(&self, digest: &str) -> PathBuf {
        self.paths.provider_config.with_file_name("analysis-cache").join(format!("{digest}.msgpack"))
    }

    fn read_complete_cache(&self, digest: &str, prepared: &PreparedText) -> Option<Arc<CompleteUnit>> {
        let bytes = fs::read(self.complete_cache_path(digest)).ok()?;
        let unit: CompleteUnit = rmp_serde::from_slice(&bytes).ok()?;
        if unit.document.schema != kotoclip_nlp::model::SCHEMA || unit.document.preparation.source_sha256 != prepared.mapping.source_sha256
            || unit.document.text != prepared.text || unit.document.external_sources.is_empty() { return None; }
        let unit = Arc::new(unit);
        self.complete_units.lock().unwrap().insert(format!("complete:{digest}"), unit.clone());
        Some(unit)
    }

    fn write_complete_cache(&self, digest: &str, unit: &CompleteUnit) {
        let path = self.complete_cache_path(digest);
        let result = (|| -> Result<(), String> {
            fs::create_dir_all(path.parent().unwrap()).map_err(|error| error.to_string())?;
            let bytes = rmp_serde::to_vec_named(unit).map_err(|error| error.to_string())?;
            let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
            fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
            if path.exists() { fs::remove_file(&path).map_err(|error| error.to_string())?; }
            fs::rename(temporary, path).map_err(|error| error.to_string())
        })();
        if let Err(error) = result { log::warn!("完整分析缓存写入失败：{error}"); }
    }

    fn document_for_timing(&self, document: Arc<UnifiedDocument>, enabled: bool) -> Arc<UnifiedDocument> {
        if enabled { return document; }
        let mut value = (*document).clone();
        value.stage_timings.clear();
        value.elapsed_ms = 0.0;
        Arc::new(value)
    }

    pub fn enrich(&self, document: &UnifiedDocument, generation: u64) -> Result<(Arc<UnifiedDocument>, Vec<Value>), String> {
        let mut stage_timings = document.stage_timings.clone();
        let external_started = Instant::now();
        let (sources, diagnostics) = self.external.lock().unwrap().analyze(&document.text, generation);
        stage_timings.push(StageTiming::new("enrich.external", external_started));
        let unify_started = Instant::now();
        let prepared = PreparedText { text: document.text.clone(), annotations: document.author_ruby.clone(), mapping: document.preparation.clone() };
        let mut updated = kotoclip_nlp::unify::unify_with_sources(&prepared, document.source.clone(), document.routing.clone(), &sources)?;
        updated.ruby_validations = document.ruby_validations.clone();
        let grammar_overlay = kotoclip_nlp::grammar::GrammarArtifact { schema: kotoclip_nlp::grammar::SCHEMA.into(), occurrences: document.grammar.occurrences.iter()
            .filter(|item| !matches!(item.provider.as_str(), "unidic" | "compiled-grammar-catalog" | "user-rule")).cloned().collect() };
        let expression_overlay = kotoclip_nlp::expression::ExpressionArtifact { schema: kotoclip_nlp::expression::SCHEMA.into(), occurrences: document.expression.occurrences.iter()
            .filter(|item| !matches!(item.provider.as_str(), "builtin-expression-catalog" | "user-rule")).cloned().collect() };
        updated.grammar = kotoclip_nlp::grammar::merge(updated.grammar, grammar_overlay);
        updated.expression = kotoclip_nlp::expression::merge(updated.expression, expression_overlay);
        stage_timings.push(StageTiming::new("enrich.unify", unify_started));
        updated.stage_timings = stage_timings;
        updated.elapsed_ms = document.elapsed_ms;
        let updated = Arc::new(updated);
        self.remember(updated.clone());
        Ok((updated, diagnostics))
    }

    pub fn refresh_language(&self, document: &UnifiedDocument) -> Result<Arc<UnifiedDocument>, String> {
        let updated = Arc::new(document.clone());
        self.remember(updated.clone());
        Ok(updated)
    }

    pub fn save_rule(&self, rule: kotoclip_nlp::rules::Rule) -> Result<RuleSnapshot, String> { self.rules.save(rule) }
    pub fn set_rule_enabled(&self, id: &str, enabled: bool) -> Result<RuleSnapshot, String> { self.rules.set_enabled(id, enabled) }
    pub fn delete_rule(&self, id: &str) -> Result<RuleSnapshot, String> { self.rules.delete(id) }

    pub fn query(&self, analysis_id: Option<String>, token: Option<&MorphemeToken>, forms: &[QueryForm], selected_form: Option<&str>) -> Result<Value, String> {
        let mut dictionary = self.dictionary.lock().unwrap();
        if dictionary.is_none() { *dictionary = Some(DictionaryEngine::prepare(&self.paths.dictionary_sources, &self.paths.dictionaries).map_err(|e| e.to_string())?); }
        serde_json::to_value(output::query(dictionary.as_ref().unwrap(), analysis_id, token, forms, selected_form)).map_err(|e| e.to_string())
    }

    pub fn lookup_targets(&self, document: &UnifiedDocument, range: [usize; 2]) -> Result<Value, String> {
        let mut dictionary = self.dictionary.lock().unwrap();
        if dictionary.is_none() { *dictionary = Some(DictionaryEngine::prepare(&self.paths.dictionary_sources, &self.paths.dictionaries).map_err(|e| e.to_string())?); }
        let group = crate::dictionary::targets::build(dictionary.as_ref().unwrap(), document, range, &[])?;
        serde_json::to_value(group).map_err(|error| error.to_string())
    }

    pub fn lookup_report(&self, document: &UnifiedDocument) -> Result<Value, String> {
        let mut dictionary = self.dictionary.lock().unwrap();
        if dictionary.is_none() { *dictionary = Some(DictionaryEngine::prepare(&self.paths.dictionary_sources, &self.paths.dictionaries).map_err(|error| error.to_string())?); }
        let dictionary = dictionary.as_ref().unwrap();
        let group = crate::dictionary::targets::build(dictionary, document, [0, document.characters], &[])?;
        crate::dictionary::targets::report(dictionary, &group)
    }

    pub fn query_target(&self, document: &UnifiedDocument, target_id: &str, selected_form: Option<&str>) -> Result<Value, String> {
        let mut dictionary = self.dictionary.lock().unwrap();
        if dictionary.is_none() { *dictionary = Some(DictionaryEngine::prepare(&self.paths.dictionary_sources, &self.paths.dictionaries).map_err(|e| e.to_string())?); }
        let group = crate::dictionary::targets::build(dictionary.as_ref().unwrap(), document, [0, document.characters], &[])?;
        let target = group.outer_targets.iter().chain(group.inner_targets.iter()).chain(group.candidate_targets.iter())
            .find(|target| target.id == target_id).ok_or("查词目标引用无效")?;
        let request = target.matrix_request.as_ref().ok_or("该目标没有词条矩阵")?;
        serde_json::to_value(crate::dictionary::targets::query(dictionary.as_ref().unwrap(), request, selected_form)).map_err(|error| error.to_string())
    }

    pub fn query_group(&self, group: Value, target_id: &str, selected_form: Option<&str>) -> Result<Value, String> {
        let group: crate::dictionary::targets::LookupTargetGroup = serde_json::from_value(group).map_err(|error| error.to_string())?;
        let target = group.outer_targets.iter().chain(group.inner_targets.iter()).chain(group.candidate_targets.iter())
            .find(|target| target.id == target_id).ok_or("查词目标引用无效")?;
        let request = target.matrix_request.as_ref().ok_or("该目标没有词条矩阵")?;
        let mut dictionary = self.dictionary.lock().unwrap();
        if dictionary.is_none() { *dictionary = Some(DictionaryEngine::prepare(&self.paths.dictionary_sources, &self.paths.dictionaries).map_err(|error| error.to_string())?); }
        serde_json::to_value(crate::dictionary::targets::query(dictionary.as_ref().unwrap(), request, selected_form)).map_err(|error| error.to_string())
    }
}

fn resource_identity(path: &Path) -> Value {
    let metadata = fs::metadata(path).ok();
    json!({"path": path, "size": metadata.as_ref().map(|item| item.len()),
        "modified": metadata.and_then(|item| item.modified().ok()).and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map(|time| time.as_nanos())})
}
