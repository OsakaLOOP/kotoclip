//! 文档会话与单工作线程调度。控制操作立即更新代次，迟到产物在提交前失效。
use crate::{analysis_engine::AnalysisEngine, document_plan::DocumentPlan};
use kotoclip_nlp::{model::UnifiedDocument, routing::RegisterPolicy};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, HashMap, VecDeque}, sync::{Arc, Mutex, mpsc::{self, SyncSender}, atomic::Ordering}, thread::JoinHandle};

pub const SCHEMA: &str = "kotoclip.document-update.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnitStage { Pending, Analyzing, Basic, Enriching, Complete, Failed }

#[derive(Debug, Clone, Serialize)]
pub struct UnitUpdate {
    pub unit_id: String,
    pub stage: UnitStage,
    pub artifact_revision: u64,
    pub document: Option<Arc<UnifiedDocument>>,
    pub providers: Vec<Value>,
    pub error: Option<String>,
}

struct Session {
    id: String,
    plan: Arc<DocumentPlan>,
    generation: u64,
    revision: u64,
    paused: bool,
    order: Vec<usize>,
    priority_count: usize,
    units: Vec<UnitUpdate>,
    history: VecDeque<(u64, Vec<UnitUpdate>)>,
    retained: VecDeque<usize>,
}

impl Session {
    fn validate(&self, text_version: &str, generation: u64) -> Result<(), String> {
        if self.plan.text_version != text_version { return Err("文档文本版本已改变，请重新打开".into()); }
        if self.generation != generation { return Err("文档任务代次已改变，请同步会话".into()); }
        Ok(())
    }

    fn publish(&mut self, indices: &[usize]) {
        self.revision += 1;
        self.history.push_back((self.revision, indices.iter().map(|&i| self.units[i].clone()).collect()));
        while self.history.len() > 32 { self.history.pop_front(); }
    }

    fn progress(&self) -> Value {
        json!({"total": self.units.len(), "basic": self.units.iter().filter(|u| u.document.is_some() || u.stage == UnitStage::Complete).count(),
            "complete": self.units.iter().filter(|u| u.stage == UnitStage::Complete).count(), "failed": self.units.iter().filter(|u| u.stage == UnitStage::Failed).count(),
            "pending": if self.paused { 0 } else { self.order.iter().filter(|&&i| matches!(self.units[i].stage, UnitStage::Pending | UnitStage::Analyzing | UnitStage::Basic | UnitStage::Enriching)).count() }})
    }

    fn update(&self, after: Option<u64>) -> Value {
        let incremental = after.filter(|&after| after <= self.revision && self.history.front().is_some_and(|(first, _)| after + 1 >= *first));
        let changes: Vec<_> = if let Some(after) = incremental {
            let mut changed = BTreeMap::new();
            for (_, units) in self.history.iter().filter(|(revision, _)| *revision > after) {
                for unit in units { changed.insert(&unit.unit_id, unit); }
            }
            changed.into_values().collect()
        } else { self.units.iter().collect() };
        json!({"schema": SCHEMA, "session_id": self.id, "document_id": self.plan.id, "text_version": self.plan.text_version,
            "generation": self.generation, "base_revision": incremental, "revision": self.revision, "snapshot": incremental.is_none(),
            "paused": self.paused, "progress": self.progress(), "changes": changes})
    }

    fn advance_generation(&mut self) {
        self.generation += 1;
        self.history.clear();
        for unit in &mut self.units {
            if matches!(unit.stage, UnitStage::Analyzing | UnitStage::Enriching) {
                unit.stage = if unit.document.is_some() { UnitStage::Basic } else { UnitStage::Pending };
            }
        }
    }

    fn request_range(&mut self, range: [usize; 2]) -> Result<(), String> {
        if range[0] > range[1] || range[1] > self.plan.prepared.mapping.origins.len() { return Err("请求范围超出正文".into()); }
        let first = self.plan.units.partition_point(|unit| unit.anchor.char_range[1] <= range[0]).min(self.units.len().saturating_sub(1));
        let last = self.plan.units.partition_point(|unit| unit.anchor.char_range[0] < range[1]).max(first + 1).min(self.units.len());
        self.order = (first..last).collect();
        if first > 0 { self.order.push(first - 1); }
        if last < self.units.len() { self.order.push(last); }
        self.priority_count = self.order.len().min(8);
        for &index in &self.order {
            let unit = &mut self.units[index];
            if unit.document.is_none() { unit.stage = UnitStage::Pending; unit.error = None; }
        }
        self.paused = false;
        Ok(())
    }
}

struct Job {
    session_id: String,
    generation: u64,
    index: usize,
    stage: UnitStage,
    plan: Arc<DocumentPlan>,
    document: Option<Arc<UnifiedDocument>>,
    cancellation: u64,
}

#[derive(Default)]
struct State {
    sessions: HashMap<String, Session>,
    recent: VecDeque<String>,
    sequence: u64,
    active: Option<String>,
    shutdown: bool,
}

impl State {
    fn next(&mut self, cancellation: u64) -> Option<Job> {
        for id in self.recent.iter().rev() {
            let session = self.sessions.get_mut(id).unwrap();
            if session.paused { continue; }
            let primary = &session.order[..session.priority_count];
            let index = primary.iter().copied().find(|&i| session.units[i].stage == UnitStage::Pending)
                .or_else(|| primary.iter().copied().find(|&i| session.units[i].stage == UnitStage::Basic))
                .or_else(|| session.order[session.priority_count..].iter().copied().find(|&i| matches!(session.units[i].stage, UnitStage::Pending | UnitStage::Basic)));
            if let Some(index) = index {
                let stage = if session.units[index].stage == UnitStage::Pending { UnitStage::Analyzing } else { UnitStage::Enriching };
                session.units[index].stage = stage;
                session.publish(&[index]);
                self.active = Some(id.clone());
                return Some(Job { session_id: id.clone(), generation: session.generation, index, stage, plan: session.plan.clone(),
                    document: session.units[index].document.clone(), cancellation });
            }
        }
        self.active = None;
        None
    }
}

pub struct DocumentSessions {
    state: Arc<Mutex<State>>,
    wake: Option<SyncSender<()>>,
    worker: Option<JoinHandle<()>>,
    engine: Arc<AnalysisEngine>,
}

impl DocumentSessions {
    pub fn new(engine: Arc<AnalysisEngine>) -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let (wake, receiver) = mpsc::sync_channel(1);
        let worker_state = state.clone();
        let worker_engine = engine.clone();
        let worker = std::thread::spawn(move || loop {
            let job = {
                let mut state = worker_state.lock().unwrap();
                if state.shutdown { break; }
                state.next(worker_engine.cancellation.load(Ordering::Relaxed))
            };
            let Some(job) = job else { if receiver.recv().is_err() { break; } else { continue; } };
            let result = if job.stage == UnitStage::Analyzing {
                let (prepared, routing) = job.plan.unit_input(job.index);
                worker_engine.analyze(&prepared, routing, &[], None, None).map(|document| (document, Vec::new()))
            } else { worker_engine.enrich(job.document.as_ref().unwrap(), job.cancellation) };
            let mut state = worker_state.lock().unwrap();
            state.active = None;
            let Some(session) = state.sessions.get_mut(&job.session_id) else { continue; };
            if session.generation != job.generation { continue; }
            let unit = &mut session.units[job.index];
            match result {
                Ok((document, providers)) => {
                    let failed = providers.iter().any(|p| p["status"] == "failed" || p["status"] == "cancelled");
                    unit.stage = if job.stage == UnitStage::Analyzing { UnitStage::Basic } else if failed { UnitStage::Failed } else { UnitStage::Complete };
                    unit.artifact_revision += 1;
                    unit.document = Some(document);
                    unit.providers = providers;
                    unit.error = failed.then(|| "部分来源分析未完成，可重试该单元".into());
                }
                Err(error) => { unit.stage = UnitStage::Failed; unit.error = Some(error); },
            }
            session.retained.retain(|&index| index != job.index);
            session.retained.push_back(job.index);
            let mut changed = vec![job.index];
            while session.retained.len() > 24 {
                let position = session.retained.iter().position(|index| !session.order[..session.priority_count].contains(index)).unwrap();
                let index = session.retained.remove(position).unwrap();
                session.units[index].document = None;
                changed.push(index);
            }
            session.publish(&changed);
        });
        Self { state, wake: Some(wake), worker: Some(worker), engine }
    }

    fn wake(&self) { let _ = self.wake.as_ref().unwrap().try_send(()); }

    pub fn open(&self, document_id: Option<String>, text: &str, policy: RegisterPolicy) -> Result<Value, String> {
        if text.trim().is_empty() { return Err("请输入日文正文".into()); }
        let plan = Arc::new(DocumentPlan::new(document_id, text, policy));
        if plan.units.is_empty() { return Err("输入内容没有可分析的正文".into()); }
        let mut state = self.state.lock().unwrap();
        if state.active.is_some() { self.engine.cancellation.fetch_add(1, Ordering::Relaxed); }
        for session in state.sessions.values_mut() { session.advance_generation(); session.paused = true; session.publish(&[]); }
        state.sequence += 1;
        let id = format!("session:{}", state.sequence);
        let units = plan.units.iter().map(|unit| UnitUpdate { unit_id: unit.id.clone(), stage: UnitStage::Pending, artifact_revision: 0, document: None, providers: Vec::new(), error: None }).collect();
        let mut session = Session { id: id.clone(), plan: plan.clone(), generation: 1, revision: 0, paused: false,
            order: Vec::new(), priority_count: 0, units, history: VecDeque::new(), retained: VecDeque::new() };
        session.request_range([0, plan.prepared.mapping.origins.len().min(crate::document_plan::UNIT_CHARACTERS)])?;
        let result = json!({"plan": plan.as_ref(), "update": session.update(None)});
        state.sessions.insert(id.clone(), session);
        state.recent.push_back(id);
        while state.recent.len() > 3 { let old = state.recent.pop_front().unwrap(); state.sessions.remove(&old); }
        drop(state); self.wake();
        Ok(result)
    }

    pub fn poll(&self, id: &str, text_version: &str, generation: u64, after: u64) -> Result<Value, String> {
        let state = self.state.lock().unwrap();
        let session = state.sessions.get(id).ok_or("文档会话已关闭")?;
        session.validate(text_version, generation)?;
        Ok(session.update(Some(after)))
    }

    pub fn snapshot(&self, id: &str) -> Result<Value, String> {
        let state = self.state.lock().unwrap();
        Ok(state.sessions.get(id).ok_or("文档会话已关闭")?.update(None))
    }

    pub fn invalidate_sources(&self) {
        let mut state = self.state.lock().unwrap();
        if state.active.is_some() { self.engine.cancellation.fetch_add(1, Ordering::Relaxed); }
        for session in state.sessions.values_mut() {
            session.advance_generation();
            session.paused = false;
            if session.order.is_empty() {
                session.order.extend(0..session.units.len());
                session.priority_count = session.order.len().min(8);
            }
            for unit in &mut session.units {
                unit.stage = UnitStage::Pending;
                unit.document = None;
                unit.artifact_revision += 1;
                unit.providers.clear(); unit.error = None;
            }
            session.publish(&[]);
        }
        drop(state); self.wake();
    }

    pub fn refresh_language(&self) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        for session in state.sessions.values_mut() {
            let mut changed = Vec::new();
            for (index, unit) in session.units.iter_mut().enumerate() {
                let Some(document) = &unit.document else { continue; };
                unit.document = Some(self.engine.refresh_language(document)?);
                unit.artifact_revision += 1;
                changed.push(index);
            }
            if !changed.is_empty() { session.publish(&changed); }
        }
        Ok(())
    }

    pub fn control(&self, id: &str, text_version: &str, generation: u64, action: &str, range: Option<[usize; 2]>, unit_id: Option<&str>) -> Result<Value, String> {
        let mut state = self.state.lock().unwrap();
        let current = state.sessions.get(id).ok_or("文档会话已关闭")?;
        current.validate(text_version, generation)?;
        if range.is_some_and(|r| r[0] > r[1] || r[1] > current.plan.prepared.mapping.origins.len()) { return Err("请求范围超出正文".into()); }
        if unit_id.is_some_and(|id| !current.units.iter().any(|unit| unit.unit_id == id)) { return Err("正文单元引用无效".into()); }
        if state.active.as_deref() == Some(id) { self.engine.cancellation.fetch_add(1, Ordering::Relaxed); }
        let session = state.sessions.get_mut(id).unwrap();
        session.advance_generation();
        match action {
            "range" => session.request_range(range.unwrap())?,
            "continue" => {
                for index in 0..session.units.len() { if !session.order.contains(&index) { session.order.push(index); } }
                session.paused = false;
            },
            "cancel" => session.paused = true,
            "retry" => {
                for (index, unit) in session.units.iter_mut().enumerate() {
                    if unit_id.map_or(unit.stage == UnitStage::Failed, |id| id == unit.unit_id) {
                        unit.stage = if unit.document.is_some() { UnitStage::Basic } else { UnitStage::Pending };
                        unit.error = None;
                        if !session.order.contains(&index) { session.order.push(index); }
                    }
                }
                session.paused = false;
            },
            _ => return Err("未知文档操作".into()),
        }
        session.publish(&[]);
        let result = session.update(None);
        state.recent.retain(|item| item != id); state.recent.push_back(id.into());
        drop(state); self.wake();
        Ok(result)
    }

    pub fn close(&self, id: &str) -> Result<Value, String> {
        let mut state = self.state.lock().unwrap();
        if state.active.as_deref() == Some(id) { self.engine.cancellation.fetch_add(1, Ordering::Relaxed); }
        state.sessions.remove(id);
        state.recent.retain(|item| item != id);
        Ok(json!({"closed": true, "session_id": id}))
    }

    pub fn document(&self, id: &str, text_version: &str, generation: u64, unit_id: &str, revision: u64) -> Result<Arc<UnifiedDocument>, String> {
        let state = self.state.lock().unwrap();
        let session = state.sessions.get(id).ok_or("文档会话已关闭")?;
        session.validate(text_version, generation)?;
        let unit = session.units.iter().find(|unit| unit.unit_id == unit_id).ok_or("正文单元引用无效")?;
        if unit.artifact_revision != revision { return Err("分析产物已更新，请重新选择词语".into()); }
        unit.document.clone().ok_or_else(|| "正文范围已释放，请重新请求该范围".into())
    }
}

impl Drop for DocumentSessions {
    fn drop(&mut self) {
        self.state.lock().unwrap().shutdown = true;
        self.engine.cancellation.fetch_add(1, Ordering::Relaxed);
        self.wake.take();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}
