use async_trait::async_trait;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

use super::{parse_openai_chunk, LlmError, LlmProvider, LlmStream, ToolResultInject};
use crate::llm::events::LlmEvent;
use crate::llm::request::{ContentPart, LlmRequest};

pub struct OpenAiCompatibleChatProvider {
    provider_name: String,
    api_key_env: String,
    api_key: Option<String>,
    base_url: String,
    supported_models: Vec<String>,
    http_client: reqwest::Client,
}

impl OpenAiCompatibleChatProvider {
    pub fn new(
        provider_name: &str,
        api_key: Option<String>,
        base_url: Option<String>,
        default_base_url: &str,
        api_key_env: &str,
        supported_models: &[&str],
    ) -> Self {
        Self {
            provider_name: provider_name.to_string(),
            api_key_env: api_key_env.to_string(),
            api_key: api_key.or_else(|| std::env::var(api_key_env).ok()),
            base_url: base_url.unwrap_or_else(|| default_base_url.to_string()),
            supported_models: supported_models
                .iter()
                .map(|model| model.to_string())
                .collect(),
            http_client: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }

    pub fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    pub async fn endpoint_status(&self, model: &str) -> Result<reqwest::StatusCode, LlmError> {
        let body = serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": "ping" }],
            "stream": false,
            "max_tokens": 1,
        });

        let mut request = self
            .http_client
            .post(self.chat_completions_url())
            .timeout(Duration::from_secs(15))
            .header("Content-Type", "application/json")
            .json(&body);

        if let Some(api_key) = self.api_key.as_deref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }

        request
            .send()
            .await
            .map(|response| response.status())
            .map_err(|error| {
                LlmError::from(error)
                    .provider(&self.provider_name)
                    .model(model)
            })
    }

    fn auth_error(&self) -> LlmError {
        LlmError::new("auth", &format!("{} not set", self.api_key_env))
            .provider(&self.provider_name)
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleChatProvider {
    fn provider_name(&self) -> &str {
        &self.provider_name
    }

    fn supported_models(&self) -> Vec<String> {
        self.supported_models.clone()
    }

    fn model_base_url(&self) -> Option<&str> {
        Some(&self.base_url)
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    async fn stream(&self, request: LlmRequest) -> Result<LlmStream, LlmError> {
        let api_key = self.api_key.as_ref().ok_or_else(|| self.auth_error())?;
        let body = build_request_body(&request, lower_messages);

        tracing::debug!(
            "llm request: provider={} url={} model={} key={:?}",
            self.provider_name,
            self.chat_completions_url(),
            request.model,
            api_key,
        );

        let (abort_tx, mut abort_rx) = oneshot::channel();
        let abort_tx = Arc::new(abort_tx);
        let (tool_result_tx, mut tool_result_rx) = mpsc::channel::<ToolResultInject>(32);

        let auth_header = format!("Bearer {api_key}");
        tracing::debug!(
            "llm request header: provider={} url={} model={} Authorization={:?}",
            self.provider_name,
            self.chat_completions_url(),
            request.model,
            auth_header,
        );

        let response = self
            .http_client
            .post(self.chat_completions_url())
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                LlmError::from(error)
                    .provider(&self.provider_name)
                    .model(&request.model)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            tracing::warn!(
                "llm http error: provider={} url={} model={} status={} body={}",
                self.provider_name,
                self.chat_completions_url(),
                request.model,
                status.as_u16(),
                &text.chars().take(500).collect::<String>(),
            );
            return Err(LlmError::new(&format!("http_{}", status.as_u16()), &text)
                .provider(&self.provider_name)
                .model(&request.model));
        }

        let stream = async_stream::stream! {
            let mut event_stream = response.bytes_stream();
            let mut tool_result_rx = tool_result_rx;

            // The HTTP stream is the single source of truth for the LLM's
            // turn. End the stream as soon as the HTTP response is done.
            // Holding the stream open after the LLM has finished (waiting
            // for `tool_result_rx.recv()` to return) deadlocks the runner,
            // because the runner needs the stream to end in order to
            // persist the assistant message.
            loop {
                tokio::select! {
                    biased;

                    chunk = event_stream.next() => {
                        let Some(chunk) = chunk else { break };
                        if matches!(abort_rx.try_recv(), Err(tokio::sync::oneshot::error::TryRecvError::Closed)) {
                            break;
                        }

                        let Ok(bytes) = chunk else { continue };
                        let text = String::from_utf8_lossy(&bytes);

                        let mut saw_done = false;
                        for line in text.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }

                            if line == "data: [DONE]" || line == "[DONE]" {
                                saw_done = true;
                                break;
                            }

                            if let Some(data) = line.strip_prefix("data: ") {
                                let data = data.trim();
                                if data.is_empty() || data == "[DONE]" {
                                    continue;
                                }

                                // Note: structured `delta.tool_calls` is
                                // intentionally ignored. The LLM is
                                // expected to emit tool calls as XML
                                // blocks in the `delta.content` field;
                                // the runner parses those out of the
                                // accumulated text.
                                if let Some(event) = parse_openai_chunk(data) {
                                    yield event;
                                }
                            }
                        }

                        if saw_done {
                            break;
                        }
                    }

                    inject = tool_result_rx.recv() => {
                        let Some(inject) = inject else { break };
                        yield LlmEvent::ToolResult {
                            id: inject.id,
                            name: inject.name,
                            result: crate::llm::events::ToolResultValue::Json { value: inject.result },
                            output: None,
                        };
                    }
                }
            }

            drop(tool_result_rx);
        };

        Ok(LlmStream {
            events: Box::pin(stream),
            tool_result_tx,
            abort_tx: Some(abort_tx),
        })
    }
}

fn build_request_body(
    request: &LlmRequest,
    lower_messages: impl Fn(&LlmRequest) -> Vec<serde_json::Value>,
) -> serde_json::Value {
    let messages = lower_messages(request);

    let mut body = serde_json::json!({
        "model": request.model,
        "messages": messages,
        "stream": true,
        "stream_options": { "include_usage": true },
    });

    if let Some(temp) = request.temperature {
        body["temperature"] = serde_json::json!(temp);
    }
    if let Some(max_tok) = request.max_tokens {
        body["max_tokens"] = serde_json::json!(max_tok);
    }
    if let Some(top_p) = request.top_p {
        body["top_p"] = serde_json::json!(top_p);
    }
    if let Some(stop) = &request.stop {
        body["stop"] = serde_json::json!(stop);
    }
    // Note: tool definitions are NOT sent in the request body. The
    // LLM is told about tools via the system prompt (in XML block
    // format) and emits tool calls as `<tool_name>...</tool_name>`
    // blocks in its `text_delta` events. The runner parses those
    // out of the accumulated text.
    body
}

#[cfg(test)]
mod body_tests {
    use super::*;
    use crate::llm::events::ToolDefinition;
    use crate::llm::request::LlmMessage;

    fn sample_request() -> LlmRequest {
        LlmRequest::new("gpt-4o-mini", "openai")
            .with_message(LlmMessage::user("hi"))
            .with_tools(std::iter::once(ToolDefinition::new(
                "read",
                "Read a file",
                serde_json::json!({"type": "object", "properties": {"path": {"type":"string"}}}),
            )))
    }

    #[test]
    fn build_request_body_omits_tools_field() {
        // Even if the caller attaches a `tools` list to the request,
        // the wire body must NOT include it.
        let body = build_request_body(&sample_request(), |req| {
            vec![serde_json::json!({"role": "user", "content": "hi"})]
        });
        assert!(
            body.get("tools").is_none(),
            "tools must not be sent on the wire; body was: {body}"
        );
    }

    #[test]
    fn build_request_body_includes_messages_model_stream() {
        let body = build_request_body(&sample_request(), |req| {
            vec![serde_json::json!({"role": "user", "content": "hi"})]
        });
        assert_eq!(body["model"], "gpt-4o-mini");
        assert_eq!(body["stream"], true);
        assert!(body["stream_options"]["include_usage"]
            .as_bool()
            .unwrap_or(false));
        assert!(body["messages"]
            .as_array()
            .map(|a| a.len() == 1)
            .unwrap_or(false));
    }

    #[test]
    fn build_request_body_passes_through_optional_params() {
        let mut req = sample_request();
        req.temperature = Some(0.5);
        req.max_tokens = Some(256);
        req.top_p = Some(0.9);
        req.stop = Some(vec!["STOP".to_string()]);
        let body = build_request_body(&req, |_| vec![]);
        assert_eq!(body["temperature"].as_f64().unwrap(), 0.5);
        assert_eq!(body["max_tokens"], 256);
        // f32 -> f64 round-tripping is approximate; compare with a
        // small tolerance rather than exact equality.
        let top_p = body["top_p"].as_f64().unwrap();
        assert!((top_p - 0.9).abs() < 1e-5, "top_p was {top_p}");
        assert_eq!(body["stop"][0], "STOP");
    }
}

fn lower_messages(request: &LlmRequest) -> Vec<serde_json::Value> {
    request
        .messages
        .iter()
        .map(|msg| {
            let role = match msg.role.as_str() {
                "user" => "user",
                "assistant" => "assistant",
                "system" => "system",
                "tool" => "tool",
                _ => "user",
            };
            serde_json::json!({ "role": role, "content": lower_content(&msg.content) })
        })
        .collect()
}

fn lower_content(content: &[ContentPart]) -> serde_json::Value {
    if content.is_empty() {
        return serde_json::Value::String(String::new());
    }

    let parts = content
        .iter()
        .filter_map(|part| {
            part.as_prompt_text()
                .map(|text| serde_json::json!({ "type": "text", "text": text }))
        })
        .collect::<Vec<_>>();

    if parts.len() == 1 && parts[0].get("type").and_then(|value| value.as_str()) == Some("text") {
        return serde_json::Value::String(
            parts[0]
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string(),
        );
    }

    serde_json::Value::Array(parts)
}
