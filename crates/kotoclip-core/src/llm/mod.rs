//! LLM 辅助能力的可选框架。
//!
//! 当前模块不接入 Engine、Tauri 命令或 UI。调用方必须显式提供网络传输、
//! 凭据和用户授权，并在采用任何结果前执行本地证据校验。

pub mod client;
pub mod dictionary;
pub mod transport;

pub use client::{
    ApiCredential, OpenAiCompatibleProvider, StructuredCompletionProvider,
    StructuredCompletionRequest, StructuredLlmClient,
};
pub use dictionary::{
    build_disambiguation_prompt, build_disambiguation_request, validate_disambiguation_decision,
    DictionaryDisambiguationDecision, DictionaryDisambiguationRequest, DisambiguationBudget,
    DisambiguationValidationError,
};
pub use transport::{
    HttpHeader, HttpJsonRequest, HttpJsonResponse, JsonHttpTransport, NetworkPolicy, TransportError,
};
