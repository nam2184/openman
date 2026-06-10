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
            supported_models: supported_models.iter().map(|model| model.to_string()).collect(),
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
            .map_err(|error| LlmError::from(error).provider(&self.provider_name).model(model))
    }

    fn auth_error(&self) -> LlmError {
        LlmError::new("auth", &format!("{} not set", self.api_key_env)).provider(&self.provider_name)
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
        let messages = lower_messages(&request);
        let tools = lower_tools(&request);

        tracing::debug!(
            "llm request: provider={} url={} model={} key={:?}",
            self.provider_name,
            self.chat_completions_url(),
            request.model,
            api_key,
        );

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
        if tools != serde_json::Value::Null {
            body["tools"] = tools;
        }

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
            .map_err(|error| LlmError::from(error).provider(&self.provider_name).model(&request.model))?;

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
            let mut in_tool_call = false;
            let mut tool_call_id = String::new();
            let mut tool_call_name = String::new();
            let mut tool_call_buffer = String::new();

            while let Some(chunk) = event_stream.next().await {
                if matches!(abort_rx.try_recv(), Err(tokio::sync::oneshot::error::TryRecvError::Closed)) {
                    break;
                }

                let Ok(bytes) = chunk else { continue };
                let text = String::from_utf8_lossy(&bytes);

                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    if line == "data: [DONE]" || line == "[DONE]" {
                        break;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        let data = data.trim();
                        if data.is_empty() || data == "[DONE]" {
                            continue;
                        }

                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
                                if let Some(choice) = choices.first() {
                                    if let Some(delta) = choice.get("delta").and_then(|d| d.as_object()) {
                                        if delta.contains_key("tool_calls") {
                                            in_tool_call = true;
                                            if let Some(calls) = delta.get("tool_calls").and_then(|c| c.as_array()) {
                                                if let Some(call) = calls.first() {
                                                    if let Some(id) = call.get("id").and_then(|i| i.as_str()) {
                                                        tool_call_id = id.to_string();
                                                    }
                                                    if let Some(func) = call.get("function").and_then(|f| f.as_object()) {
                                                        if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                                                            tool_call_name = name.to_string();
                                                        }
                                                        if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                                                            tool_call_buffer = args.to_string();
                                                        }
                                                    }
                                                }
                                            }
                                            continue;
                                        }
                                    }
                                }
                            }
                        }

                        if in_tool_call && !tool_call_id.is_empty() {
                            in_tool_call = false;
                            let input: serde_json::Value = serde_json::from_str(&tool_call_buffer)
                                .unwrap_or(serde_json::Value::Object(Default::default()));
                            yield LlmEvent::ToolCall {
                                id: tool_call_id.clone(),
                                name: tool_call_name.clone(),
                                input,
                                provider_executed: Some(false),
                            };
                            tool_call_id.clear();
                            tool_call_name.clear();
                            tool_call_buffer.clear();
                        }

                        if let Some(event) = parse_openai_chunk(data) {
                            yield event;
                        }
                    }
                }
            }

            while let Some(inject) = tool_result_rx.recv().await {
                yield LlmEvent::ToolResult {
                    id: inject.id,
                    name: inject.name,
                    result: crate::llm::events::ToolResultValue::Json { value: inject.result },
                    output: None,
                };
            }
        };

        Ok(LlmStream {
            events: Box::pin(stream),
            tool_result_tx,
            abort_tx: Some(abort_tx),
        })
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
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(serde_json::json!({ "type": "text", "text": text })),
            ContentPart::ToolCall { id, name, input } => Some(serde_json::json!({
                "type": "function",
                "id": id,
                "function": {
                    "name": name,
                    "arguments": serde_json::to_string(input).unwrap_or_default()
                }
            })),
            ContentPart::ToolResult { id, name, result } => Some(serde_json::json!({
                "type": "function",
                "id": id,
                "name": name,
                "content": serde_json::to_string(result).unwrap_or_default()
            })),
            ContentPart::Reasoning { .. } => None,
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

fn lower_tools(request: &LlmRequest) -> serde_json::Value {
    if request.tools.is_empty() {
        return serde_json::Value::Null;
    }

    serde_json::Value::Array(
        request
            .tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters
                    }
                })
            })
            .collect::<Vec<_>>(),
    )
}
