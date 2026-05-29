//! Generic HTTP adapter for AI providers.
//!
//! The adapter is entirely driven by [`ProviderConfig`]: request bodies are
//! rendered from `request_body_template`, authentication headers are assembled
//! from the configured auth fields, and response content is extracted via the
//! configured `response_path`.

use crate::config::ProviderConfig;
use crate::encryption::{EncryptionError, Encryptor};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stream_utils::{Channel, ChannelError};
use thiserror::Error;

/// One chat message sent to a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Classification of a provider test connection failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestConnectionErrorKind {
    /// Authentication failed (HTTP 401/403).
    Auth,
    /// Network/transport failure (DNS, TLS, timeout, refused).
    Network,
    /// Provider configuration is incomplete or invalid (missing key, bad template).
    InvalidConfig,
    /// Provider returned an unexpected HTTP status.
    HttpStatus,
    /// Provider response could not be parsed or the configured response path is wrong.
    MalformedResponse,
    /// Catch-all for unclassified errors.
    Unknown,
}

/// Error detail returned by `provider_test_connection`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTestError {
    pub kind: TestConnectionErrorKind,
    pub message: String,
}

/// Result returned by `provider_test_connection`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTestResult {
    pub success: bool,
    pub error: Option<ProviderTestError>,
}

/// Classify an adapter error string into a [`TestConnectionErrorKind`].
///
/// The classification is based on the human-readable error messages produced
/// by [`AiProviderError`]'s `Display` implementation.
pub fn classify_test_error(message: &str) -> TestConnectionErrorKind {
    let lower = message.to_lowercase();

    if lower.contains("missing an api key")
        || lower.contains("encryption error")
        || lower.contains("invalid auth header")
    {
        TestConnectionErrorKind::InvalidConfig
    } else if lower.contains("http error: ")
        && !lower.contains("returned http ")
    {
        TestConnectionErrorKind::Network
    } else if lower.contains("returned http 401") || lower.contains("returned http 403") {
        TestConnectionErrorKind::Auth
    } else if lower.contains("returned http ") {
        TestConnectionErrorKind::HttpStatus
    } else if lower.contains("json error")
        || lower.contains("response path")
        || lower.contains("did not resolve to a string")
    {
        TestConnectionErrorKind::MalformedResponse
    } else {
        TestConnectionErrorKind::Unknown
    }
}

const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Errors returned by the generic provider adapter.
#[derive(Debug, Error)]
pub enum AiProviderError {
    #[error("provider '{provider}' is missing an API key")]
    MissingApiKey { provider: String },

    #[error("encryption error: {0}")]
    Encryption(#[from] EncryptionError),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("invalid auth header name '{name}': {reason}")]
    InvalidHeaderName { name: String, reason: String },

    #[error("invalid auth header value for '{name}': {reason}")]
    InvalidHeaderValue { name: String, reason: String },

    #[error("provider returned HTTP {status}: {body}")]
    HttpStatus {
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("response path '{path}' did not resolve to a string")]
    ResponsePathNotString { path: String },

    #[error("response path error at '{segment}' in '{path}'")]
    ResponsePath { path: String, segment: String },

    #[error("stream channel error: {0}")]
    Channel(#[from] ChannelError),
}

/// Generic HTTP AI provider driven by [`ProviderConfig`].
#[derive(Clone)]
pub struct AiProvider {
    config: ProviderConfig,
    api_key: String,
    client: reqwest::Client,
}

impl AiProvider {
    /// Build an adapter from persisted config, decrypting the API key once.
    pub fn new(config: ProviderConfig, encryptor: &Encryptor) -> Result<Self, AiProviderError> {
        let api_key = config.decrypt_api_key(encryptor)?.unwrap_or_default();
        Ok(Self::with_api_key(config, api_key))
    }

    /// Build an adapter with an already-decrypted API key.
    pub fn with_api_key(config: ProviderConfig, api_key: String) -> Self {
        Self {
            config,
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Expose the config used by this adapter.
    pub fn config(&self) -> &ProviderConfig {
        &self.config
    }

    /// Render `request_body_template` by substituting supported placeholders.
    pub fn render_request_body(
        &self,
        model: &str,
        messages: &[ChatMessage],
        stream: bool,
        temperature: f64,
    ) -> Result<Value, AiProviderError> {
        render_request_body(
            &self.config.request_body_template,
            model,
            messages,
            stream,
            temperature,
        )
    }

    /// Send a non-streaming chat request and extract the configured content string.
    pub async fn chat_once(
        &self,
        model: &str,
        messages: &[ChatMessage],
        temperature: f64,
    ) -> Result<String, AiProviderError> {
        let body = self.render_request_body(model, messages, false, temperature)?;
        let response_body = self.post_json(body).await?;
        extract_response_content(&response_body, &self.config.response_path).map(ToOwned::to_owned)
    }

    /// Send a minimal chat request. Intended for `provider_test_connection`.
    ///
    /// On failure, the error is classified into a [`ProviderTestError`] with
    /// a structured [`TestConnectionErrorKind`] so the UI can display an
    /// actionable message.
    pub async fn test_connection(&self) -> ProviderTestResult {
        let model = self
            .config
            .models
            .first()
            .map(|model| model.id.as_str())
            .unwrap_or("test");
        let messages = [ChatMessage {
            role: "user".to_string(),
            content: "Reply with OK".to_string(),
        }];

        match self.chat_once(model, &messages, 0.0).await {
            Ok(_) => ProviderTestResult {
                success: true,
                error: None,
            },
            Err(error) => {
                let message = error.to_string();
                let kind = classify_test_error(&message);
                ProviderTestResult {
                    success: false,
                    error: Some(ProviderTestError { kind, message }),
                }
            }
        }
    }

    /// Stream provider tokens and push each extracted token through a Channel.
    pub async fn chat_stream(
        &self,
        model: &str,
        messages: &[ChatMessage],
        temperature: f64,
        channel: &Channel<String>,
    ) -> Result<(), AiProviderError> {
        let body = self.render_request_body(model, messages, true, temperature)?;
        let mut response = self
            .client
            .post(&self.config.base_url)
            .headers(self.auth_headers()?)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AiProviderError::HttpStatus { status, body });
        }

        let mut pending = String::new();
        while let Some(chunk) = response.chunk().await? {
            pending.push_str(&String::from_utf8_lossy(&chunk));
            let consumed = drain_complete_sse_events(&mut pending, |event| {
                if let Some(token) = self.extract_sse_token(event)? {
                    channel.send(token)?;
                }
                Ok::<_, AiProviderError>(())
            })?;

            if consumed {
                break;
            }
        }

        // Process a final event that may not have been followed by a blank line.
        if !pending.trim().is_empty() {
            let event = std::mem::take(&mut pending);
            for data in data_lines(&event) {
                if data == "[DONE]" {
                    break;
                }
                if let Some(token) = self.extract_sse_token(data)? {
                    channel.send(token)?;
                }
            }
        }

        Ok(())
    }

    async fn post_json(&self, body: Value) -> Result<Value, AiProviderError> {
        let response = self
            .client
            .post(&self.config.base_url)
            .headers(self.auth_headers()?)
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(AiProviderError::HttpStatus { status, body });
        }

        Ok(serde_json::from_str(&body)?)
    }

    fn auth_headers(&self) -> Result<HeaderMap, AiProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(AiProviderError::MissingApiKey {
                provider: self.config.name.clone(),
            });
        }

        let name =
            HeaderName::from_bytes(self.config.auth_header_name.as_bytes()).map_err(|e| {
                AiProviderError::InvalidHeaderName {
                    name: self.config.auth_header_name.clone(),
                    reason: e.to_string(),
                }
            })?;
        let value = format!("{}{}", self.config.auth_header_value_prefix, self.api_key);
        let value =
            HeaderValue::from_str(&value).map_err(|e| AiProviderError::InvalidHeaderValue {
                name: self.config.auth_header_name.clone(),
                reason: e.to_string(),
            })?;

        let mut headers = HeaderMap::new();
        headers.insert(name, value);
        if requires_anthropic_version_header(&self.config.base_url) {
            headers.insert(
                HeaderName::from_static("anthropic-version"),
                HeaderValue::from_static(ANTHROPIC_VERSION),
            );
        }
        Ok(headers)
    }

    fn extract_sse_token(&self, event_data: &str) -> Result<Option<String>, AiProviderError> {
        let json: Value = serde_json::from_str(event_data)?;
        match extract_response_content(&json, &self.config.response_path) {
            Ok(token) if !token.is_empty() => Ok(Some(token.to_string())),
            Ok(_) => Ok(None),
            Err(AiProviderError::ResponsePath { .. })
            | Err(AiProviderError::ResponsePathNotString { .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

/// Render a JSON request template with the supported placeholders.
pub fn render_request_body(
    template: &Value,
    model: &str,
    messages: &[ChatMessage],
    stream: bool,
    temperature: f64,
) -> Result<Value, AiProviderError> {
    let messages = serde_json::to_value(messages)?;
    Ok(render_value(
        template,
        model,
        &messages,
        stream,
        temperature,
    ))
}

fn render_value(
    value: &Value,
    model: &str,
    messages: &Value,
    stream: bool,
    temperature: f64,
) -> Value {
    match value {
        Value::String(s) => render_string(s, model, messages, stream, temperature),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| render_value(item, model, messages, stream, temperature))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        render_value(value, model, messages, stream, temperature),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn requires_anthropic_version_header(base_url: &str) -> bool {
    base_url.contains("api.anthropic.com")
}

fn render_string(
    value: &str,
    model: &str,
    messages: &Value,
    stream: bool,
    temperature: f64,
) -> Value {
    match value {
        "{model}" => Value::String(model.to_string()),
        "{messages}" => messages.clone(),
        "{stream}" => Value::Bool(stream),
        "{temperature}" => serde_json::Number::from_f64(temperature)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        _ => Value::String(
            value
                .replace("{model}", model)
                .replace("{messages}", &messages.to_string())
                .replace("{stream}", if stream { "true" } else { "false" })
                .replace("{temperature}", &temperature.to_string()),
        ),
    }
}

/// Extract a string from a JSON response using a simple JSONPath-like path.
///
/// Supported syntax: `choices[0].message.content`, `.choices[0].text`, and
/// `$.choices[0].message.content`.
pub fn extract_response_content<'a>(
    response: &'a Value,
    response_path: &str,
) -> Result<&'a str, AiProviderError> {
    let mut current = response;
    let trimmed = response_path
        .trim()
        .trim_start_matches('$')
        .trim_start_matches('.');

    if trimmed.is_empty() {
        return current
            .as_str()
            .ok_or_else(|| AiProviderError::ResponsePathNotString {
                path: response_path.to_string(),
            });
    }

    for segment in trimmed.split('.') {
        current = apply_path_segment(current, response_path, segment)?;
    }

    current
        .as_str()
        .ok_or_else(|| AiProviderError::ResponsePathNotString {
            path: response_path.to_string(),
        })
}

fn apply_path_segment<'a>(
    mut current: &'a Value,
    full_path: &str,
    segment: &str,
) -> Result<&'a Value, AiProviderError> {
    let mut rest = segment;
    if let Some(field_end) = rest.find('[') {
        let field = &rest[..field_end];
        if !field.is_empty() {
            current = current
                .get(field)
                .ok_or_else(|| path_error(full_path, segment))?;
        }
        rest = &rest[field_end..];
    } else {
        return current
            .get(rest)
            .ok_or_else(|| path_error(full_path, segment));
    }

    while !rest.is_empty() {
        if !rest.starts_with('[') {
            return Err(path_error(full_path, segment));
        }
        let Some(end) = rest.find(']') else {
            return Err(path_error(full_path, segment));
        };
        let index: usize = rest[1..end]
            .parse()
            .map_err(|_| path_error(full_path, segment))?;
        current = current
            .get(index)
            .ok_or_else(|| path_error(full_path, segment))?;
        rest = &rest[end + 1..];
    }

    Ok(current)
}

fn path_error(path: &str, segment: &str) -> AiProviderError {
    AiProviderError::ResponsePath {
        path: path.to_string(),
        segment: segment.to_string(),
    }
}

fn drain_complete_sse_events<F>(
    pending: &mut String,
    mut on_event: F,
) -> Result<bool, AiProviderError>
where
    F: FnMut(&str) -> Result<(), AiProviderError>,
{
    let mut done = false;

    while let Some(index) = pending.find("\n\n") {
        let event = pending[..index].to_string();
        pending.drain(..index + 2);

        for data in data_lines(&event) {
            if data == "[DONE]" {
                done = true;
                break;
            }
            on_event(data)?;
        }

        if done {
            break;
        }
    }

    Ok(done)
}

fn data_lines(event: &str) -> Vec<&str> {
    event
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("data:"))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelInfo, ProviderConfig};
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn config(response_path: &str) -> ProviderConfig {
        ProviderConfig {
            id: 1,
            name: "Test".to_string(),
            base_url: "http://127.0.0.1/chat".to_string(),
            api_key_encrypted: Some(vec![1, 2, 3]),
            auth_header_name: "Authorization".to_string(),
            auth_header_value_prefix: "Bearer ".to_string(),
            models: vec![ModelInfo {
                id: "gpt-test".to_string(),
                name: "GPT Test".to_string(),
                context_window: 1024,
            }],
            request_body_template: serde_json::json!({
                "model": "{model}",
                "messages": "{messages}",
                "stream": "{stream}",
                "temperature": "{temperature}",
                "metadata": {"label": "model={model};stream={stream}"}
            }),
            response_path: response_path.to_string(),
        }
    }

    #[test]
    fn template_rendering_substitutes_placeholders_with_json_types() {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];

        let rendered = render_request_body(
            &config("choices[0].message.content").request_body_template,
            "gpt-4o",
            &messages,
            true,
            0.25,
        )
        .unwrap();

        assert_eq!(rendered["model"], "gpt-4o");
        assert_eq!(rendered["messages"][0]["role"], "user");
        assert_eq!(rendered["stream"], true);
        assert_eq!(rendered["temperature"], 0.25);
        assert_eq!(rendered["metadata"]["label"], "model=gpt-4o;stream=true");
    }

    #[test]
    fn response_extraction_supports_dot_and_array_segments() {
        let response = serde_json::json!({
            "choices": [{"message": {"content": "hello world"}}]
        });

        let content = extract_response_content(&response, "choices[0].message.content").unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn response_extraction_supports_dollar_prefix() {
        let response = serde_json::json!({
            "content": [{"text": "hello"}]
        });

        let content = extract_response_content(&response, "$.content[0].text").unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn response_extraction_rejects_missing_or_non_string_path() {
        let response = serde_json::json!({"choices": [{"message": {"content": 42}}]});

        assert!(matches!(
            extract_response_content(&response, "choices[0].message.missing"),
            Err(AiProviderError::ResponsePath { .. })
        ));
        assert!(matches!(
            extract_response_content(&response, "choices[0].message.content"),
            Err(AiProviderError::ResponsePathNotString { .. })
        ));
    }

    #[test]
    fn sse_parser_handles_complete_events_and_done() {
        let mut pending = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n",
            "data: [DONE]\n\n",
            "data: ignored\n\n"
        )
        .to_string();
        let mut events = Vec::new();

        let done = drain_complete_sse_events(&mut pending, |event| {
            events.push(event.to_string());
            Ok(())
        })
        .unwrap();

        assert!(done);
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn openai_compatible_request_response_cycle_uses_configured_auth_and_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "mocked response"}}]
            })))
            .mount(&server)
            .await;

        let mut provider_config = config("choices[0].message.content");
        provider_config.base_url = format!("{}/chat/completions", server.uri());
        provider_config.auth_header_name = "X-API-Key".to_string();
        provider_config.auth_header_value_prefix = "Token ".to_string();
        let provider = AiProvider::with_api_key(provider_config, "secret-key".to_string());

        let content = provider
            .chat_once(
                "gpt-4o-mini",
                &[ChatMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                }],
                0.7,
            )
            .await
            .unwrap();

        assert_eq!(content, "mocked response");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(
            request.headers.get("x-api-key").unwrap().to_str().unwrap(),
            "Token secret-key"
        );
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body["model"], "gpt-4o-mini");
        assert_eq!(body["messages"][0]["content"], "hello");
        assert_eq!(body["stream"], false);
        assert_eq!(body["temperature"], 0.7);
    }

    #[tokio::test]
    async fn missing_api_key_fails_before_http_request() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "should not be called"}}]
            })))
            .mount(&server)
            .await;

        let mut provider_config = config("choices[0].message.content");
        provider_config.name = "OpenAI".to_string();
        provider_config.base_url = format!("{}/chat/completions", server.uri());
        let provider = AiProvider::with_api_key(provider_config, String::new());

        let error = provider
            .chat_once(
                "gpt-4o-mini",
                &[ChatMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                }],
                0.7,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(error, AiProviderError::MissingApiKey { provider } if provider == "OpenAI")
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn anthropic_requests_include_required_version_header() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"text": "mocked anthropic response"}]
            })))
            .mount(&server)
            .await;

        let mut provider_config = config("content[0].text");
        provider_config.base_url =
            format!("{}/v1/messages?upstream=api.anthropic.com", server.uri());
        provider_config.auth_header_name = "x-api-key".to_string();
        provider_config.auth_header_value_prefix = String::new();
        provider_config.request_body_template = serde_json::json!({
            "model": "{model}",
            "messages": "{messages}",
            "max_tokens": 1024,
        });
        let provider = AiProvider::with_api_key(provider_config, "anthropic-secret".to_string());

        let content = provider
            .chat_once(
                "claude-sonnet-4-20250514",
                &[ChatMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                }],
                0.7,
            )
            .await
            .unwrap();

        assert_eq!(content, "mocked anthropic response");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(
            request.headers.get("x-api-key").unwrap().to_str().unwrap(),
            "anthropic-secret"
        );
        assert_eq!(
            request
                .headers
                .get("anthropic-version")
                .unwrap()
                .to_str()
                .unwrap(),
            "2023-06-01"
        );
    }

    #[tokio::test]
    async fn streaming_response_pushes_tokens_through_channel() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(concat!(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}\n\n",
                        "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n",
                        "data: [DONE]\n\n"
                    )),
            )
            .mount(&server)
            .await;

        let mut provider_config = config("choices[0].delta.content");
        provider_config.base_url = format!("{}/chat/completions", server.uri());
        let provider = AiProvider::with_api_key(provider_config, "secret-key".to_string());
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let tauri_channel = tauri::ipc::Channel::<String>::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let item: String = serde_json::from_str(&s).unwrap();
                received_clone.lock().unwrap().push(item);
            }
            Ok(())
        });
        let channel = Channel::from_tauri(tauri_channel);

        provider
            .chat_stream(
                "gpt-4o-mini",
                &[ChatMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                }],
                0.7,
                &channel,
            )
            .await
            .unwrap();

        assert_eq!(
            *received.lock().unwrap(),
            vec!["hel".to_string(), "lo".to_string()]
        );
        let requests = server.received_requests().await.unwrap();
        let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["stream"], true);
    }

    // ── test_connection structured error tests ────────────────────

    #[tokio::test]
    async fn test_connection_returns_success_for_valid_provider() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "OK"}}]
            })))
            .mount(&server)
            .await;

        let mut provider_config = config("choices[0].message.content");
        provider_config.base_url = format!("{}/chat/completions", server.uri());
        let provider = AiProvider::with_api_key(provider_config, "secret-key".to_string());

        let result = provider.test_connection().await;
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_connection_classifies_missing_api_key_as_invalid_config() {
        let provider = AiProvider::with_api_key(config("choices[0].message.content"), String::new());

        let result = provider.test_connection().await;
        assert!(!result.success);
        let error = result.error.unwrap();
        assert_eq!(error.kind, TestConnectionErrorKind::InvalidConfig);
        assert!(error.message.contains("missing an API key"));
    }

    #[tokio::test]
    async fn test_connection_classifies_auth_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(401).set_body_string("Unauthorized"),
            )
            .mount(&server)
            .await;

        let mut provider_config = config("choices[0].message.content");
        provider_config.base_url = format!("{}/chat/completions", server.uri());
        let provider = AiProvider::with_api_key(provider_config, "bad-key".to_string());

        let result = provider.test_connection().await;
        assert!(!result.success);
        let error = result.error.unwrap();
        assert_eq!(error.kind, TestConnectionErrorKind::Auth);
    }

    #[tokio::test]
    async fn test_connection_classifies_403_as_auth_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(403).set_body_string("Forbidden"),
            )
            .mount(&server)
            .await;

        let mut provider_config = config("choices[0].message.content");
        provider_config.base_url = format!("{}/chat/completions", server.uri());
        let provider = AiProvider::with_api_key(provider_config, "forbidden-key".to_string());

        let result = provider.test_connection().await;
        assert!(!result.success);
        let error = result.error.unwrap();
        assert_eq!(error.kind, TestConnectionErrorKind::Auth);
    }

    #[tokio::test]
    async fn test_connection_classifies_server_error_as_http_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(500).set_body_string("Internal Server Error"),
            )
            .mount(&server)
            .await;

        let mut provider_config = config("choices[0].message.content");
        provider_config.base_url = format!("{}/chat/completions", server.uri());
        let provider = AiProvider::with_api_key(provider_config, "key".to_string());

        let result = provider.test_connection().await;
        assert!(!result.success);
        let error = result.error.unwrap();
        assert_eq!(error.kind, TestConnectionErrorKind::HttpStatus);
    }

    #[tokio::test]
    async fn test_connection_classifies_bad_response_path_as_malformed_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "OK"}}]
            })))
            .mount(&server)
            .await;

        let mut provider_config = config("nonexistent.path[0].value");
        provider_config.base_url = format!("{}/chat/completions", server.uri());
        let provider = AiProvider::with_api_key(provider_config, "key".to_string());

        let result = provider.test_connection().await;
        assert!(!result.success);
        let error = result.error.unwrap();
        assert_eq!(error.kind, TestConnectionErrorKind::MalformedResponse);
    }

    #[tokio::test]
    async fn test_connection_classifies_network_failure() {
        // Use an unreachable URL to simulate a network failure.
        let provider_config = config("choices[0].message.content");
        // base_url points to a non-routable address.
        let provider =
            AiProvider::with_api_key(provider_config, "key".to_string());

        let result = provider.test_connection().await;
        assert!(!result.success);
        let error = result.error.unwrap();
        assert_eq!(error.kind, TestConnectionErrorKind::Network);
    }

    // ── classify_test_error unit tests ────────────────────────────

    #[test]
    fn classify_missing_api_key() {
        assert_eq!(
            classify_test_error("provider 'OpenAI' is missing an API key"),
            TestConnectionErrorKind::InvalidConfig,
        );
    }

    #[test]
    fn classify_encryption_error() {
        assert_eq!(
            classify_test_error("encryption error: decryption failed"),
            TestConnectionErrorKind::InvalidConfig,
        );
    }

    #[test]
    fn classify_invalid_auth_header() {
        assert_eq!(
            classify_test_error("invalid auth header name 'bad': invalid"),
            TestConnectionErrorKind::InvalidConfig,
        );
    }

    #[test]
    fn classify_http_401() {
        assert_eq!(
            classify_test_error("provider returned HTTP 401 Unauthorized: body"),
            TestConnectionErrorKind::Auth,
        );
    }

    #[test]
    fn classify_http_403() {
        assert_eq!(
            classify_test_error("provider returned HTTP 403 Forbidden: body"),
            TestConnectionErrorKind::Auth,
        );
    }

    #[test]
    fn classify_http_500() {
        assert_eq!(
            classify_test_error("provider returned HTTP 500 Internal Server Error: body"),
            TestConnectionErrorKind::HttpStatus,
        );
    }

    #[test]
    fn classify_http_429() {
        assert_eq!(
            classify_test_error("provider returned HTTP 429 Too Many Requests: body"),
            TestConnectionErrorKind::HttpStatus,
        );
    }

    #[test]
    fn classify_json_error() {
        assert_eq!(
            classify_test_error("json error: expected value at line 1 column 1"),
            TestConnectionErrorKind::MalformedResponse,
        );
    }

    #[test]
    fn classify_response_path_error() {
        assert_eq!(
            classify_test_error("response path 'bad.path' did not resolve to a string"),
            TestConnectionErrorKind::MalformedResponse,
        );
    }

    #[test]
    fn classify_unknown_error() {
        assert_eq!(
            classify_test_error("something unexpected happened"),
            TestConnectionErrorKind::Unknown,
        );
    }
}
