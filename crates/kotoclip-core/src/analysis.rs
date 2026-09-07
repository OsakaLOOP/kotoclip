use crate::{dictionary::lookup::DictionaryEngine, output};
use kotoclip_nlp::{
    model::{QueryForm, Register, UnifiedDocument},
    prepare::prepare_text,
    routing::select_register,
    sources::UniDicProvider,
    unify::unify,
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
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status,
    Analyze {
        text: String,
        register: Register,
    },
    Query {
        analysis_id: String,
        token_id: String,
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
}

impl AnalysisService {
    pub fn new(paths: ResourcePaths) -> Self {
        Self {
            paths,
            providers: HashMap::new(),
            dictionary: None,
            documents: VecDeque::new(),
        }
    }
    pub fn dispatch(&mut self, request: Request) -> Response {
        match self.execute(request) {
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
    fn execute(&mut self, request: Request) -> Result<Value, String> {
        match request {
            Request::Status => Ok(
                json!({ "schema": kotoclip_nlp::model::SCHEMA, "providers": [
                    {"register": "cwj", "available": self.paths.cwj.is_file(), "loaded": self.providers.contains_key(&Register::Cwj)},
                    {"register": "csj", "available": self.paths.csj.is_file(), "loaded": self.providers.contains_key(&Register::Csj)}
                ]}),
            ),
            Request::Analyze { text, register } => {
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
                let mut document = unify(&prepared, source, routing)?;
                document.elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
                let value = serde_json::to_value(&document).map_err(|e| e.to_string())?;
                self.documents.retain(|d| d.id != document.id);
                self.documents.push_back(document);
                while self.documents.len() > 4 {
                    self.documents.pop_front();
                }
                Ok(value)
            }
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
