use crate::{dictionary::lookup::DictionaryEngine, output};
use kotoclip_nlp::{
    model::{QueryForm, Register, UnifiedDocument},
    prepare::prepare_text,
    routing::select_register,
    sources::UniDicProvider,
    syntax::SyntaxArtifact,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Debug, Clone)]
pub struct ResourcePaths {
    pub cwj: PathBuf,
    pub csj: PathBuf,
    pub dictionary_sources: PathBuf,
    pub dictionaries: PathBuf,
    pub provider_config: PathBuf,
    pub provider_script: PathBuf,
    pub provider_defaults: crate::providers::ProviderSettings,
}

impl ResourcePaths {
    pub fn development(root: &Path) -> Self {
        let data = std::env::var_os("KOTOCLIP_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("data"));
        Self {
            cwj: std::env::var_os("KOTOCLIP_UNIDIC_CWJ")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    root.join("experiments/unidic-source/unidic-cwj-202512.vibrato.dic")
                }),
            csj: std::env::var_os("KOTOCLIP_UNIDIC_CSJ")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    root.join("experiments/unidic-source/unidic-csj-202512.vibrato.dic")
                }),
            dictionary_sources: data.join("dict-sources"),
            dictionaries: data.join("dicts"),
            provider_config: data.join("providers.local.json"),
            provider_script: root.join("scripts/nlp_provider.py"),
            provider_defaults: crate::providers::ProviderSettings::development(root),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status,
    ProviderStatus,
    CheckProviders,
    ConfigureProviders { settings: crate::providers::ProviderSettings },
    Enrich { analysis_id: String },
    CancelExternal,
    Analyze {
        text: String,
        register: Register,
    },
    AnalyzeWithArtifacts {
        text: String,
        register: Register,
        artifacts: Vec<SyntaxArtifact>,
        #[serde(default)]
        grammar: Option<kotoclip_nlp::grammar::GrammarArtifact>,
        #[serde(default)]
        expression: Option<kotoclip_nlp::expression::ExpressionArtifact>,
    },
    Query {
        analysis_id: String,
        token_id: String,
    },
    QueryCandidate {
        analysis_id: String,
        candidate_id: String,
    },
    Search {
        word: String,
    },
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub result: Option<Value>,
    pub error: Option<String>,
}

pub struct AnalysisService {
    paths: ResourcePaths,
    providers: HashMap<Register, UniDicProvider>,
    dictionary: Option<DictionaryEngine>,
    documents: VecDeque<UnifiedDocument>,
    external: crate::providers::ProviderManager,
}

impl AnalysisService {
    pub fn new(paths: ResourcePaths) -> Self {
        let external = crate::providers::ProviderManager::new(paths.provider_config.clone(), paths.provider_script.clone(), paths.provider_defaults.clone());
        Self {
            paths,
            providers: HashMap::new(),
            dictionary: None,
            documents: VecDeque::new(),
            external,
        }
    }
    pub fn cancellation(&self) -> std::sync::Arc<std::sync::atomic::AtomicU64> {
        self.external.cancellation.clone()
    }
    pub fn dispatch(&mut self, request: Request) -> Response {
        let generation = self.external.cancellation.load(std::sync::atomic::Ordering::Relaxed);
        self.dispatch_at(request, generation)
    }
    pub fn dispatch_at(&mut self, request: Request, generation: u64) -> Response {
        match self.execute(request, generation) {
            Ok(value) => Response {
                result: Some(value),
                error: None,
            },
            Err(error) => Response {
                result: None,
                error: Some(error),
            },
        }
    }
    fn execute(&mut self, request: Request, generation: u64) -> Result<Value, String> {
        match request {
            Request::CancelExternal => {
                Ok(json!({"cancelled": true}))
            },
            Request::ProviderStatus => self.external.status(),
            Request::CheckProviders => self.external.check(generation),
            Request::ConfigureProviders { settings } => self.external.configure(settings),
            Request::Enrich { analysis_id } => self.enrich(&analysis_id, generation),
            Request::Status => Ok(
                json!({ "schema": kotoclip_nlp::model::SCHEMA, "providers": [
                    {"register": "cwj", "available": self.paths.cwj.is_file(), "loaded": self.providers.contains_key(&Register::Cwj)},
                    {"register": "csj", "available": self.paths.csj.is_file(), "loaded": self.providers.contains_key(&Register::Csj)}
                ]}),
            ),
            Request::Analyze { text, register } => self.analyze_document(text, register, &[], None, None),
            Request::AnalyzeWithArtifacts { text, register, artifacts, grammar, expression } => self.analyze_document(text, register, &artifacts, grammar, expression),
            Request::Query {
                analysis_id,
                token_id,
            } => {
                let document = self
                    .documents
                    .iter()
                    .find(|d| d.id == analysis_id)
                    .ok_or("分析结果已释放，请重新分析")?;
                let token = document
                    .morphemes
                    .iter()
                    .find(|t| t.id == token_id)
                    .cloned()
                    .ok_or("词引用无效")?;
                self.prepare_dictionary()?;
                serde_json::to_value(output::query(
                    self.dictionary.as_ref().unwrap(),
                    Some(analysis_id),
                    Some(&token),
                    &token.query_forms,
                ))
                .map_err(|e| e.to_string())
            }
            Request::QueryCandidate { analysis_id, candidate_id } => {
                let candidate = self.documents.iter()
                    .find(|document| document.id == analysis_id)
                    .ok_or("分析结果已释放，请重新分析")?
                    .dictionary_candidates.candidates.iter()
                    .find(|candidate| candidate.id == candidate_id)
                    .cloned()
                    .ok_or("词典候选引用无效")?;
                self.prepare_dictionary()?;
                serde_json::to_value(output::query(
                    self.dictionary.as_ref().unwrap(),
                    Some(analysis_id),
                    None,
                    &candidate.query_forms,
                )).map_err(|e| e.to_string())
            }
            Request::Search { word } => {
                let word = word.trim();
                if word.is_empty() || word.chars().count() > 100 {
                    return Err("查询词应为 1 至 100 字符".into());
                }
                self.prepare_dictionary()?;
                let forms = [QueryForm {
                    kind: "search".into(),
                    form: word.into(),
                    reading: None,
                    reading_field: None,
                }];
                serde_json::to_value(output::query(
                    self.dictionary.as_ref().unwrap(),
                    None,
                    None,
                    &forms,
                ))
                .map_err(|e| e.to_string())
            }
        }
    }

    fn enrich(&mut self, analysis_id: &str, generation: u64) -> Result<Value, String> {
        let document = self.documents.iter().find(|d| d.id == analysis_id).cloned().ok_or("分析结果已释放，请重新分析")?;
        let (sources, diagnostics) = self.external.analyze(&document.text, generation);
        let prepared = kotoclip_nlp::prepare::PreparedText { text: document.text.clone(), annotations: document.author_ruby.clone(), mapping: document.preparation.clone() };
        let mut updated = kotoclip_nlp::unify::unify_with_sources(&prepared, document.source, document.routing, &sources)?;
        updated.ruby_validations = document.ruby_validations;
        updated.grammar = document.grammar;
        updated.expression = document.expression;
        updated.projection = kotoclip_nlp::projection::from_layers(&updated.grammar, &updated.expression);
        updated.elapsed_ms = document.elapsed_ms;
        let value = json!({"document": updated, "providers": diagnostics});
        self.documents.retain(|d| d.id != analysis_id);
        self.documents.push_back(updated);
        Ok(value)
    }

    fn analyze_document(
        &mut self,
        text: String,
        register: Register,
        artifacts: &[SyntaxArtifact],
        grammar: Option<kotoclip_nlp::grammar::GrammarArtifact>,
        expression: Option<kotoclip_nlp::expression::ExpressionArtifact>,
    ) -> Result<Value, String> {
                if text.chars().count() > 20000 {
                    return Err("首版单次支持 20,000 字符，请选择较短的正文范围".into());
                }
                if text.trim().is_empty() {
                    return Err("请输入日文正文".into());
                }
                let started = Instant::now();
                let prepared = prepare_text(&text);
                let routing = select_register(&prepared.text, register);
                let selected_register = routing.selected;
                if !self.providers.contains_key(&selected_register) {
                    let path = if selected_register == Register::Cwj {
                        &self.paths.cwj
                    } else {
                        &self.paths.csj
                    };
                    self.providers.insert(
                        selected_register,
                        UniDicProvider::open(selected_register, path)?,
                    );
                }
                let source = self.providers[&selected_register].analyze(&prepared.text)?;
                let mut document = kotoclip_nlp::unify::unify_with_external(&prepared, source, routing, artifacts)?;
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
                let value = serde_json::to_value(&document).map_err(|e| e.to_string())?;
                self.documents.retain(|d| d.id != document.id);
                self.documents.push_back(document);
                while self.documents.len() > 4 {
                    self.documents.pop_front();
                }
                Ok(value)
    }
    fn prepare_dictionary(&mut self) -> Result<(), String> {
        if self.dictionary.is_none() {
            self.dictionary = Some(
                DictionaryEngine::prepare(&self.paths.dictionary_sources, &self.paths.dictionaries)
                    .map_err(|e| e.to_string())?,
            );
        }
        Ok(())
    }
}
