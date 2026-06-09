use async_trait::async_trait;
use futures_util::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use super::{parse_openai_chunk, LlmError, LlmProvider, LlmStream, ToolResultInject};
use crate::llm::events::LlmEvent;
use crate::llm::request::{LlmMessage, LlmRequest};

pub struct MiniMaxProvider {
    api_key: Option<String>,
    group_id: Option<String>,
    base_url: String,
    http_client: reqwest::Client,
}

impl MiniMaxProvider {
    pub fn new(
        api_key: Option<String>,
        group_id: Option<String>,
        base_url: Option<String>,
    ) -> Self {
        let base_url = base_url.unwrap_or_else(|| "https://api.minimax.chat".to_string());
        Self {
            api_key: api_key.or_else(|| std::env::var("MINIMAX_API_KEY").ok()),
            group_id: group_id.or_else(|| std::env::var("MINIMAX_GROUP_ID").ok()),
            base_url,
            http_client: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }

    pub fn with_group_id(mut self, group_id: &str) -> Self {
        self.group_id = Some(group_id.to_string());
        self
    }
}

#[async_trait]
impl LlmProvider for MiniMaxProvider {
    fn provider_name(&self) -> &str {
        "minimax"
    }

    fn supported_models(&self) -> Vec<String> {
        vec![
            "MiniMax-Text-01".to_string(),
            "abab6-chat".to_string(),
            "abab5.5-chat".to_string(),
            "abab5.5s-chat".to_string(),
        ]
    }

    fn model_base_url(&self) -> Option<&str> {
        Some(&self.base_url)
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    async fn stream(&self, request: LlmRequest) -> Result<LlmStream, LlmError> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| LlmError::new("auth", "MINIMAX_API_KEY not set").provider("minimax"))?;

        let group_id = self
            .group_id
            .as_ref()
            .ok_or_else(|| LlmError::new("auth", "MINIMAX_GROUP_ID not set").provider("minimax"))?;

        let url = format!("{}/v1/chat/completions", self.base_url);

        let messages = request
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
                let content = if msg.content.is_empty() {
                    serde_json::Value::String("".to_string())
                } else {
                    let mut parts = Vec::new();
                    for part in &msg.content {
                        match part {
                            crate::llm::request::ContentPart::Text { text } => {
                                parts.push(serde_json::json!({
                                    "type": "text",
                                    "text": text
                                }));
                            }
                            crate::llm::request::ContentPart::ToolCall { id, name, input } => {
                                parts.push(serde_json::json!({
                                    "type": "function",
                                    "id": id,
                                    "function": {
                                        "name": name,
                                        "arguments": serde_json::to_string(input).unwrap_or_default()
                                    }
                                }));
                            }
                            crate::llm::request::ContentPart::ToolResult { id, name, result } => {
                                parts.push(serde_json::json!({
                                    "type": "function",
                                    "id": id,
                                    "name": name,
                                    "content": serde_json::to_string(result).unwrap_or_default()
                                }));
                            }
                            crate::llm::request::ContentPart::Reasoning { .. } => {}
                        }
                    }
                    if parts.len() == 1
                        && parts[0].get("type").and_then(|t| t.as_str()) == Some("text")
                    {
                        serde_json::Value::String(
                            parts[0]
                                .get("text")
                                .unwrap()
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                        )
                    } else {
                        serde_json::Value::Array(parts)
                    }
                };
                serde_json::json!({ "role": role, "content": content })
            })
            .collect::<Vec<_>>();

        let tools = if request.tools.is_empty() {
            serde_json::Value::Null
        } else {
            let tools = request
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect::<Vec<_>>();
            serde_json::Value::Array(tools)
        };

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "stream": true,
            "stream_precision": "0.001",
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

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .header("GroupId", group_id.as_str())
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::from(e).provider("minimax").model(&request.model))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(LlmError::new(&format!("http_{}", status.as_u16()), &text)
                .provider("minimax")
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
                            let input_str = tool_call_buffer.clone();
                            let input: serde_json::Value = serde_json::from_str(&input_str)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_creation() {
        let provider = MiniMaxProvider::new(
            Some("test-key".to_string()),
            Some("test-group".to_string()),
            None,
        );
        assert_eq!(provider.provider_name(), "minimax");
        assert!(provider.api_key().is_some());
    }

    #[test]
    fn provider_from_env() {
        std::env::set_var("MINIMAX_API_KEY", "env-key");
        std::env::set_var("MINIMAX_GROUP_ID", "env-group");
        let provider = MiniMaxProvider::new(None, None, None);
        assert_eq!(provider.api_key(), Some("env-key"));
        std::env::remove_var("MINIMAX_API_KEY");
        std::env::remove_var("MINIMAX_GROUP_ID");
    }
}
