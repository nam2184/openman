use async_trait::async_trait;
use futures_util::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use super::{parse_anthropic_event, LlmError, LlmProvider, LlmStream, ToolResultInject};
use crate::llm::events::{LlmEvent, Usage};
use crate::llm::request::{LlmMessage, LlmRequest};

pub struct AnthropicProvider {
    api_key: Option<String>,
    base_url: String,
    http_client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: Option<String>, base_url: Option<String>) -> Self {
        let base_url = base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string());
        Self {
            api_key: api_key.or_else(|| std::env::var("ANTHROPIC_API_KEY").ok()),
            base_url,
            http_client: reqwest::Client::builder()
                .default_headers({
                    let mut headers = reqwest::header::HeaderMap::new();
                    headers.insert(
                        "anthropic-version",
                        reqwest::header::HeaderValue::from_static("2023-06-01"),
                    );
                    headers
                })
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn provider_name(&self) -> &str {
        "anthropic"
    }

    fn supported_models(&self) -> Vec<String> {
        vec![
            "claude-sonnet-4-20250514".to_string(),
            "claude-3-5-sonnet-latest".to_string(),
            "claude-3-5-haiku-latest".to_string(),
            "claude-3-opus-latest".to_string(),
            "claude-3-haiku-latest".to_string(),
        ]
    }

    fn model_base_url(&self) -> Option<&str> {
        Some(&self.base_url)
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    async fn stream(&self, request: LlmRequest) -> Result<LlmStream, LlmError> {
        let api_key = self.api_key.as_ref().ok_or_else(|| {
            LlmError::new("auth", "ANTHROPIC_API_KEY not set").provider("anthropic")
        })?;

        let url = format!("{}/v1/messages", self.base_url);

        let (chat_messages, system) = {
            let mut system_parts: Vec<serde_json::Value> = Vec::new();
            let mut chat_messages: Vec<serde_json::Value> = Vec::new();

            for msg in &request.messages {
                match msg.role.as_str() {
                    "system" => {
                        for part in &msg.content {
                            match part {
                                crate::llm::request::ContentPart::Text { text } => {
                                    system_parts.push(serde_json::json!({
                                        "type": "text",
                                        "text": text
                                    }));
                                }
                                _ => {}
                            }
                        }
                    }
                    "user" | "assistant" | "tool" => {
                        let role = if msg.role == "tool" {
                            "user"
                        } else {
                            &msg.role
                        };
                        let mut content_parts: Vec<serde_json::Value> = Vec::new();

                        for part in &msg.content {
                            match part {
                                crate::llm::request::ContentPart::Text { text } => {
                                    content_parts.push(serde_json::json!({
                                        "type": "text",
                                        "text": text
                                    }));
                                }
                                crate::llm::request::ContentPart::ToolCall { id, name, input } => {
                                    content_parts.push(serde_json::json!({
                                        "type": "tool_use",
                                        "id": id,
                                        "name": name,
                                        "input": input
                                    }));
                                }
                                crate::llm::request::ContentPart::ToolResult {
                                    id,
                                    name,
                                    result,
                                } => {
                                    content_parts.push(serde_json::json!({
                                        "type": "tool_result",
                                        "tool_use_id": id,
                                        "content": serde_json::to_string(result).unwrap_or_default()
                                    }));
                                }
                                crate::llm::request::ContentPart::Reasoning { .. } => {}
                            }
                        }

                        chat_messages.push(serde_json::json!({
                            "role": role,
                            "content": content_parts
                        }));
                    }
                    _ => {}
                }
            }

            let system = if system_parts.len() == 1
                && system_parts[0].get("type").and_then(|t| t.as_str()) == Some("text")
            {
                system_parts[0]
                    .get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| serde_json::json!(s))
            } else if !system_parts.is_empty() {
                Some(serde_json::json!(system_parts))
            } else {
                None
            };

            (chat_messages, system)
        };

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": chat_messages,
            "stream": true,
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tok) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tok);
        } else {
            body["max_tokens"] = serde_json::json!(8192);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }

        if let Some(system_val) = system {
            body["system"] = system_val;
        }

        if !request.tools.is_empty() {
            let tools = request
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters
                    })
                })
                .collect::<Vec<_>>();
            body["tools"] = serde_json::json!(tools);
        }

        let (abort_tx, mut abort_rx) = oneshot::channel();
        let abort_tx = Arc::new(abort_tx);
        let (tool_result_tx, mut tool_result_rx) = mpsc::channel::<ToolResultInject>(32);

        let response = self
            .http_client
            .post(&url)
            .header("x-api-key", api_key.as_str())
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                LlmError::from(e)
                    .provider("anthropic")
                    .model(&request.model)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(LlmError::new(&format!("http_{}", status.as_u16()), &text)
                .provider("anthropic")
                .model(&request.model));
        }

        let stream = async_stream::stream! {
            let mut event_stream = response.bytes_stream();
            let mut line_buffer = String::new();

            while let Some(chunk) = event_stream.next().await {
                if matches!(abort_rx.try_recv(), Err(tokio::sync::oneshot::error::TryRecvError::Closed)) {
                    break;
                }

                let Ok(bytes) = chunk else { continue };
                let text = String::from_utf8_lossy(&bytes);

                for ch in text.chars() {
                    if ch == '\n' {
                        let line = line_buffer.trim();
                        let line_owned = line.to_string();
                        line_buffer.clear();

                        if line_owned.is_empty() {
                            continue;
                        }

                        if let Some(event) = parse_anthropic_event(&line_owned) {
                            yield event;
                        }
                    } else {
                        line_buffer.push(ch);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_creation() {
        let provider = AnthropicProvider::new(Some("test-key".to_string()), None);
        assert_eq!(provider.provider_name(), "anthropic");
        assert!(provider.api_key().is_some());
    }

    #[test]
    fn provider_from_env() {
        std::env::set_var("ANTHROPIC_API_KEY", "env-key");
        let provider = AnthropicProvider::new(None, None);
        assert_eq!(provider.api_key(), Some("env-key"));
        std::env::remove_var("ANTHROPIC_API_KEY");
    }
}
