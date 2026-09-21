use crate::analysis_engine::AnalysisEngine;
use kotoclip_nlp::{
    model::{QueryForm, Register},
    prepare::prepare_text,
    routing::{route_text, RegisterPolicy},
    syntax::SyntaxArtifact,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
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
    AnalyzeRouted { text: String, policy: RegisterPolicy },
    OpenDocument { document_id: Option<String>, text: String, #[serde(default)] policy: RegisterPolicy },
    RequestRange { session_id: String, text_version: String, generation: u64, range: [usize; 2] },
    ContinueDocument { session_id: String, text_version: String, generation: u64 },
    CancelDocument { session_id: String, text_version: String, generation: u64 },
    RetryDocument { session_id: String, text_version: String, generation: u64, unit_id: Option<String> },
    CloseDocument { session_id: String },
    SyncDocument { session_id: String },
    PollDocument { session_id: String, text_version: String, generation: u64, after_revision: u64 },
    QueryDocument { session_id: String, text_version: String, generation: u64, unit_id: String, artifact_revision: u64, token_id: String, #[serde(default)] selected_form: Option<String> },
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
        #[serde(default)]
        selected_form: Option<String>,
    },
    QueryCandidate {
        analysis_id: String,
        candidate_id: String,
        #[serde(default)]
        selected_form: Option<String>,
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
    engine: Arc<AnalysisEngine>,
    sessions: crate::document_session::DocumentSessions,
}

impl AnalysisService {
    pub fn new(paths: ResourcePaths) -> Self {
        let engine = Arc::new(AnalysisEngine::new(paths));
        Self { sessions: crate::document_session::DocumentSessions::new(engine.clone()), engine }
    }
    pub fn cancellation(&self) -> std::sync::Arc<std::sync::atomic::AtomicU64> {
        self.engine.cancellation.clone()
    }
    pub fn dispatch(&self, request: Request) -> Response {
        if matches!(request, Request::CancelExternal) { self.engine.cancellation.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
        let generation = self.engine.cancellation.load(std::sync::atomic::Ordering::Relaxed);
        self.dispatch_at(request, generation)
    }
    pub fn dispatch_at(&self, request: Request, generation: u64) -> Response {
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
    fn execute(&self, request: Request, generation: u64) -> Result<Value, String> {
        match request {
            Request::CancelExternal => {
                Ok(json!({"cancelled": true}))
            },
            Request::ProviderStatus => self.engine.external.lock().unwrap().status(),
            Request::CheckProviders => {
                let result = self.engine.external.lock().unwrap().check(generation);
                self.sessions.invalidate_sources();
                result
            },
            Request::ConfigureProviders { settings } => {
                let result = self.engine.external.lock().unwrap().configure(settings);
                if result.is_ok() { self.sessions.invalidate_sources(); }
                result
            },
            Request::OpenDocument { document_id, text, policy } => self.sessions.open(document_id, &text, policy),
            Request::RequestRange { session_id, text_version, generation, range } => self.sessions.control(&session_id, &text_version, generation, "range", Some(range), None),
            Request::ContinueDocument { session_id, text_version, generation } => self.sessions.control(&session_id, &text_version, generation, "continue", None, None),
            Request::CancelDocument { session_id, text_version, generation } => self.sessions.control(&session_id, &text_version, generation, "cancel", None, None),
            Request::RetryDocument { session_id, text_version, generation, unit_id } => self.sessions.control(&session_id, &text_version, generation, "retry", None, unit_id.as_deref()),
            Request::CloseDocument { session_id } => self.sessions.close(&session_id),
            Request::SyncDocument { session_id } => self.sessions.snapshot(&session_id),
            Request::PollDocument { session_id, text_version, generation, after_revision } => self.sessions.poll(&session_id, &text_version, generation, after_revision),
            Request::QueryDocument { session_id, text_version, generation, unit_id, artifact_revision, token_id, selected_form } => {
                let document = self.sessions.document(&session_id, &text_version, generation, &unit_id, artifact_revision)?;
                let token = document.morphemes.iter().find(|token| token.id == token_id).ok_or("词引用无效")?;
                let mut result = self.engine.query(Some(document.id.clone()), Some(token), &token.query_forms, selected_form.as_deref())?;
                result["target"] = json!({"session_id": session_id, "text_version": text_version, "generation": generation, "unit_id": unit_id, "artifact_revision": artifact_revision, "token_id": token_id});
                Ok(result)
            },
            Request::Enrich { analysis_id } => {
                let document = self.engine.document(&analysis_id)?;
                let (document, providers) = self.engine.enrich(&document, generation)?;
                Ok(json!({"document": document.as_ref(), "providers": providers}))
            },
            Request::Status => Ok(self.engine.status()),
            Request::Analyze { text, register } => self.analyze_document(text, register.into(), &[], None, None),
            Request::AnalyzeRouted { text, policy } => self.analyze_document(text, policy, &[], None, None),
            Request::AnalyzeWithArtifacts { text, register, artifacts, grammar, expression } => self.analyze_document(text, register.into(), &artifacts, grammar, expression),
            Request::Query {
                analysis_id,
                token_id,
                selected_form,
            } => {
                let document = self.engine.document(&analysis_id)?;
                let token = document
                    .morphemes
                    .iter()
                    .find(|t| t.id == token_id)
                    .cloned()
                    .ok_or("词引用无效")?;
                self.engine.query(Some(analysis_id), Some(&token), &token.query_forms, selected_form.as_deref())
            }
            Request::QueryCandidate { analysis_id, candidate_id, selected_form } => {
                let candidate = self.engine.document(&analysis_id)?
                    .dictionary_candidates.candidates.iter()
                    .find(|candidate| candidate.id == candidate_id)
                    .cloned()
                    .ok_or("词典候选引用无效")?;
                self.engine.query(Some(analysis_id), None, &candidate.query_forms, selected_form.as_deref())
            }
            Request::Search { word } => {
                let word = word.trim();
                if word.is_empty() || word.chars().count() > 100 {
                    return Err("查询词应为 1 至 100 字符".into());
                }
                let forms = [QueryForm {
                    kind: "search".into(),
                    form: word.into(),
                    reading: None,
                    reading_field: None,
                }];
                self.engine.query(None, None, &forms, None)
            }
        }
    }

    fn analyze_document(
        &self,
        text: String,
        policy: RegisterPolicy,
        artifacts: &[SyntaxArtifact],
        grammar: Option<kotoclip_nlp::grammar::GrammarArtifact>,
        expression: Option<kotoclip_nlp::expression::ExpressionArtifact>,
    ) -> Result<Value, String> {
        if text.chars().count() > 20000 { return Err("单次分析支持 20,000 字符，长文档请使用文档会话".into()); }
        if text.trim().is_empty() { return Err("请输入日文正文".into()); }
        let prepared = prepare_text(&text);
        let routing = route_text(&prepared.text, policy);
        let document = self.engine.analyze(&prepared, routing, artifacts, grammar, expression)?;
        serde_json::to_value(document.as_ref()).map_err(|e| e.to_string())
    }
}
