use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalysisCancelled;

impl fmt::Display for AnalysisCancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("分析已取消")
    }
}

impl std::error::Error for AnalysisCancelled {}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisPhase {
    Preparing,
    Tokenizing,
    Chunking,
    GrammarMatching,
    DictionaryMatching,
    ProfileScoring,
    ExpressionMatching,
    RecordingExposure,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgress {
    pub phase: AnalysisPhase,
    pub completed: usize,
    pub total: usize,
    pub percent: u8,
    pub message: String,
}

impl AnalysisProgress {
    pub fn stage(phase: AnalysisPhase, percent: u8, message: impl Into<String>) -> Self {
        Self {
            phase,
            completed: 0,
            total: 0,
            percent: percent.min(100),
            message: message.into(),
        }
    }

    pub fn counted(
        phase: AnalysisPhase,
        completed: usize,
        total: usize,
        percent: u8,
        message: impl Into<String>,
    ) -> Self {
        Self {
            phase,
            completed,
            total,
            percent: percent.min(100),
            message: message.into(),
        }
    }
}
