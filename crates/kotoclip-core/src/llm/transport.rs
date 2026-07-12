use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use thiserror::Error;

#[derive(Clone, PartialEq, Eq)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
    pub sensitive: bool,
}

impl fmt::Debug for HttpHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpHeader")
            .field("name", &self.name)
            .field(
                "value",
                &if self.sensitive {
                    "[REDACTED]"
                } else {
                    self.value.as_str()
                },
            )
            .field("sensitive", &self.sensitive)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct HttpJsonRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<HttpHeader>,
    pub json_body: Value,
    pub timeout_ms: u64,
    pub max_response_bytes: usize,
}

impl HttpJsonRequest {
    /// 供日志使用；敏感 header 值永远不会进入调试输出。
    pub fn redacted_headers(&self) -> Vec<HttpHeader> {
        self.headers
            .iter()
            .map(|header| HttpHeader {
                name: header.name.clone(),
                value: if header.sensitive {
                    "[REDACTED]".to_string()
                } else {
                    header.value.clone()
                },
                sensitive: header.sensitive,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpJsonResponse {
    pub status: u16,
    pub json_body: Value,
    pub response_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    pub https_only: bool,
    pub allowed_origins: Vec<String>,
    pub timeout_ms: u64,
    pub max_response_bytes: usize,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            https_only: true,
            allowed_origins: Vec::new(),
            timeout_ms: 20_000,
            max_response_bytes: 256 * 1024,
        }
    }
}

impl NetworkPolicy {
    pub fn validate_endpoint(&self, endpoint: &str) -> Result<(), TransportError> {
        if self.https_only && !endpoint.starts_with("https://") {
            return Err(TransportError::Policy(
                "LLM endpoint 必须使用 HTTPS".to_string(),
            ));
        }
        let origin = endpoint_origin(endpoint).ok_or_else(|| {
            TransportError::Policy("LLM endpoint 不是可识别的绝对 HTTP(S) URL".to_string())
        })?;
        if !self.allowed_origins.is_empty()
            && !self
                .allowed_origins
                .iter()
                .any(|allowed| allowed.trim_end_matches('/') == origin)
        {
            return Err(TransportError::Policy(
                "LLM endpoint 不在显式授权的 origin 列表中".to_string(),
            ));
        }
        Ok(())
    }
}

fn endpoint_origin(endpoint: &str) -> Option<&str> {
    let scheme_end = endpoint.find("://")?;
    let authority_start = scheme_end + 3;
    let authority_end = endpoint[authority_start..]
        .find(|character| matches!(character, '/' | '?' | '#'))
        .map(|offset| authority_start + offset)
        .unwrap_or(endpoint.len());
    let authority = &endpoint[authority_start..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    Some(&endpoint[..authority_end])
}

/// 网络实现边界。未来可由 Tauri/reqwest 实现；核心 crate 不直接联网。
pub trait JsonHttpTransport: Send + Sync {
    fn execute(&self, request: &HttpJsonRequest) -> Result<HttpJsonResponse, TransportError>;
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("网络策略拒绝请求：{0}")]
    Policy(String),
    #[error("网络请求失败：{0}")]
    Request(String),
    #[error("响应超过大小限制：{actual} > {limit}")]
    ResponseTooLarge { actual: usize, limit: usize },
    #[error("LLM API 返回 HTTP {status}")]
    HttpStatus { status: u16 },
    #[error("响应不是预期 JSON：{0}")]
    InvalidJson(String),
}
