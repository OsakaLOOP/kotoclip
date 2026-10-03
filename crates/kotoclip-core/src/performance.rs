use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::{Arc, Mutex}, time::{Duration, SystemTime, UNIX_EPOCH}};

/// 仅用于诊断命令的实际墙钟时间累加器。
/// 同一阶段可在多个文本分段中执行，最终按真实调用耗时累加。
#[derive(Debug, Default, Clone)]
pub struct TimingCollector {
    entries: BTreeMap<String, Duration>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimingEntry {
    pub phase: String,
    pub duration_ms: u128,
}

impl TimingCollector {
    pub fn add(&mut self, phase: impl Into<String>, elapsed: Duration) {
        *self.entries.entry(phase.into()).or_default() += elapsed;
    }

    pub fn entries(&self) -> Vec<TimingEntry> {
        self.entries.iter().map(|(phase, elapsed)| TimingEntry {
            phase: phase.clone(),
            duration_ms: elapsed.as_millis(),
        }).collect()
    }
}

pub const RETAINED_EVENTS: usize = 512;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceEvent {
    pub sequence: u64,
    pub recorded_at_ms: u128,
    pub operation: String,
    pub actor: String,
    pub status: String,
    pub elapsed_ms: f64,
    pub execution_ms: f64,
    pub dictionary_lock_wait_ms: f64,
    pub dictionary_pool_wait_ms: f64,
    pub dictionary_lock_hold_ms: f64,
    pub dictionary_prepare_ms: f64,
    pub target_build_ms: f64,
    pub serialization_ms: f64,
    pub database_ms: f64,
    pub sqlite_ms: f64,
    pub transaction_ms: f64,
    pub background_task_ms: f64,
    pub details: BTreeMap<String, Value>,
}

impl PerformanceEvent {
    pub fn new(operation: impl Into<String>, actor: impl Into<String>) -> Self {
        Self {
            sequence: 0,
            recorded_at_ms: 0,
            operation: operation.into(),
            actor: actor.into(),
            status: "ok".into(),
            elapsed_ms: 0.0,
            execution_ms: 0.0,
            dictionary_lock_wait_ms: 0.0,
            dictionary_pool_wait_ms: 0.0,
            dictionary_lock_hold_ms: 0.0,
            dictionary_prepare_ms: 0.0,
            target_build_ms: 0.0,
            serialization_ms: 0.0,
            database_ms: 0.0,
            sqlite_ms: 0.0,
            transaction_ms: 0.0,
            background_task_ms: 0.0,
            details: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSnapshot {
    pub schema: String,
    pub retained_limit: usize,
    pub events: Vec<PerformanceEvent>,
}

#[derive(Default)]
struct State {
    next_sequence: u64,
    events: Vec<PerformanceEvent>,
}

#[derive(Clone, Default)]
pub struct PerformanceRecorder {
    state: Arc<Mutex<State>>,
}

impl PerformanceRecorder {
    pub fn record(&self, mut event: PerformanceEvent) {
        let mut state = self.state.lock().unwrap();
        state.next_sequence += 1;
        event.sequence = state.next_sequence;
        event.recorded_at_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |value| value.as_millis());
        state.events.push(event);
        if state.events.len() > RETAINED_EVENTS {
            let expired = state.events.len() - RETAINED_EVENTS;
            state.events.drain(..expired);
        }
    }

    pub fn snapshot(&self, clear: bool) -> PerformanceSnapshot {
        let mut state = self.state.lock().unwrap();
        let events = state.events.clone();
        if clear { state.events.clear(); }
        PerformanceSnapshot { schema: "kotoclip.performance.v1".into(), retained_limit: RETAINED_EVENTS, events }
    }
}
