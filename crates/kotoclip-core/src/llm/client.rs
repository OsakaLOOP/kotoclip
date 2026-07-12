use super::transport::{
    HttpHeader, HttpJsonRequest, HttpJsonResponse, JsonHttpTransport, NetworkPolicy, TransportError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

#[derive(Clone)]
pub struct ApiCredential(String);

impl ApiCredential {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiCredential([REDACTED])")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredCompletionRequest {
    pub task_id: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub response_schema_name: String,
    pub response_schema: Value,
    pub max_output_tokens: u32,
}

pub trait StructuredCompletionProvider: Send + Sync {
    fn endpoint(&self) -> &str;
    fn build_request(
        &self,
        request: &StructuredCompletionRequest,
        policy: &NetworkPolicy,
    ) -> Result<HttpJsonRequest, TransportError>;
    fn parse_response(&self, response: HttpJsonResponse) -> Result<Value, TransportError>;
}

/// 仅负责 OpenAI-compatible 的 HTTP 形态；不绑定任何具体服务或模型。
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    pub endpoint: String,
    pub model: String,
    pub credential: ApiCredential,
}

impl StructuredCompletionProvider for OpenAiCompatibleProvider {
    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn build_request(
        &self,
        request: &StructuredCompletionRequest,
        policy: &NetworkPolicy,
    ) -> Result<HttpJsonRequest, TransportError> {
        policy.validate_endpoint(&self.endpoint)?;
        Ok(HttpJsonRequest {
            method: "POST".to_string(),
            url: self.endpoint.clone(),
            headers: vec![
                HttpHeader {
                    name: "content-type".to_string(),
                    value: "application/json".to_string(),
                    sensitive: false,
                },
                HttpHeader {
                    name: "authorization".to_string(),
                    value: format!("Bearer {}", self.credential.expose()),
                    sensitive: true,
                },
            ],
            json_body: json!({
                "model": self.model,
                "temperature": 0,
                "messages": [
                    { "role": "system", "content": request.system_prompt },
                    { "role": "user", "content": request.user_prompt }
                ],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": request.response_schema_name,
                        "strict": true,
                        "schema": request.response_schema
                    }
                },
                "max_tokens": request.max_output_tokens
            }),
            timeout_ms: policy.timeout_ms,
            max_response_bytes: policy.max_response_bytes,
        })
    }

    fn parse_response(&self, response: HttpJsonResponse) -> Result<Value, TransportError> {
        if !(200..300).contains(&response.status) {
            return Err(TransportError::HttpStatus {
                status: response.status,
            });
        }
        let content = response
            .json_body
            .pointer("/choices/0/message/content")
            .ok_or_else(|| {
                TransportError::InvalidJson("缺少 choices[0].message.content".to_string())
            })?;
        match content {
            Value::String(text) => serde_json::from_str(text)
                .map_err(|error| TransportError::InvalidJson(error.to_string())),
            Value::Object(_) => Ok(content.clone()),
            _ => Err(TransportError::InvalidJson(
                "message.content 既不是 JSON 字符串也不是对象".to_string(),
            )),
        }
    }
}

pub struct StructuredLlmClient<T, P> {
    pub transport: T,
    pub provider: P,
    pub network_policy: NetworkPolicy,
}

impl<T, P> StructuredLlmClient<T, P>
where
    T: JsonHttpTransport,
    P: StructuredCompletionProvider,
{
    pub fn complete_json(
        &self,
        request: &StructuredCompletionRequest,
    ) -> Result<Value, TransportError> {
        self.network_policy
            .validate_endpoint(self.provider.endpoint())?;
        let http_request = self.provider.build_request(request, &self.network_policy)?;
        let response = self.transport.execute(&http_request)?;
        if response.response_bytes > self.network_policy.max_response_bytes {
            return Err(TransportError::ResponseTooLarge {
                actual: response.response_bytes,
                limit: self.network_policy.max_response_bytes,
            });
        }
        self.provider.parse_response(response)
    }
}
