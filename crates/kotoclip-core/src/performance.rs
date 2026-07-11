use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;

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
        self.entries
            .iter()
            .map(|(phase, elapsed)| TimingEntry {
                phase: phase.clone(),
                duration_ms: elapsed.as_millis(),
            })
            .collect()
    }
}
