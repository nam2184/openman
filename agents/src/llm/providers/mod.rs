use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::Stream;

use super::events::{FinishReason, LlmEvent, Usage};
use super::request::{LlmError, LlmMessage, LlmRequest, LlmResponse};

pub mod anthropic;
pub mod openai;
mod openai_compatible_chat;
pub mod minimax_token_plan;

pub use anthropic::AnthropicProvider;
pub use minimax_token_plan::MiniMaxTokenPlanProvider;
pub use openai::OpenAiProvider;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn provider_name(&self) -> &str;
    fn supported_models(&self) -> Vec<String>;

    async fn stream(&self, request: LlmRequest) -> Result<LlmStream, LlmError>;

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>
    where
        Self: Sized,
    {
        let stream = self.stream(request).await?;
        let events: Vec<LlmEvent> = stream.events.collect().await;
        Ok(LlmResponse::from_events(events))
    }

    fn model_base_url(&self) -> Option<&str>;
    fn api_key(&self) -> Option<&str>;
}

pub struct LlmStream {
    pub events: Pin<Box<dyn Stream<Item = LlmEvent> + Send>>,
    pub tool_result_tx: mpsc::Sender<ToolResultInject>,
    pub abort_tx: Option<Arc<oneshot::Sender<()>>>,
}

impl LlmStream {
    pub async fn inject_tool_result(
        &self,
        id: &str,
        name: &str,
        result: Value,
    ) -> Result<(), LlmError> {
        self.tool_result_tx
            .send(ToolResultInject {
                id: id.to_string(),
                name: name.to_string(),
                result,
            })
            .await
            .map_err(|_| LlmError::new("stream_closed", "tool result channel closed"))
    }

    pub fn abort(&self) {
        if let Some(tx) = self.abort_tx.as_ref() {
            if let Ok(sender) = Arc::try_unwrap(tx.clone()) {
                let _ = sender.send(());
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolResultInject {
    pub id: String,
    pub name: String,
    pub result: Value,
}

pub fn parse_sse_line(line: &str) -> Option<String> {
    if line.starts_with("data: ") {
        Some(line[7..].to_string())
    } else {
        None
    }
}

pub fn parse_openai_chunk(text: &str) -> Option<LlmEvent> {
    let json: Value = serde_json::from_str(text).ok()?;
    let choices = json.get("choices")?.as_array()?;
    let choice = choices.first()?;
    let delta = choice.get("delta")?;

    if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            return Some(LlmEvent::TextDelta {
                id: "text".to_string(),
                text: text.to_string(),
            });
        }
    }

    if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
        let usage = json.get("usage").and_then(|u| parse_openai_usage(u));
        return Some(LlmEvent::Finish {
            reason: FinishReason::from(reason),
            usage,
        });
    }

    None
}

pub fn parse_openai_tool_call(delta: &Value) -> Option<LlmEvent> {
    let tool_calls = delta.get("tool_calls")?.as_array()?;
    let call = tool_calls.first()?;
    let id = call.get("id")?.as_str()?.to_string();
    let function = call.get("function")?;
    let name = function.get("name")?.as_str()?.to_string();
    let args = function
        .get("arguments")
        .and_then(|a| a.as_str())
        .unwrap_or("{}");
    let input: Value = serde_json::from_str(args).unwrap_or(Value::Null);

    Some(LlmEvent::ToolCall {
        id,
        name,
        input,
        provider_executed: Some(false),
    })
}

fn parse_openai_usage(value: &Value) -> Option<Usage> {
    Some(Usage {
        input_tokens: value.get("prompt_tokens").and_then(|v| v.as_u64()),
        output_tokens: value.get("completion_tokens").and_then(|v| v.as_u64()),
        total_tokens: value.get("total_tokens").and_then(|v| v.as_u64()),
        reasoning_tokens: None,
        cache_read_input_tokens: None,
        cache_write_input_tokens: None,
    })
}

pub fn parse_anthropic_event(line: &str) -> Option<LlmEvent> {
    let data = parse_sse_line(line)?;
    let event_type = data
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("event: "))?;
    let content = data
        .lines()
        .find(|l| l.starts_with("data: "))
        .map(|l| &l[7..])?;

    match event_type.trim() {
        "message_start" => Some(LlmEvent::StepStart { index: 0 }),
        "content_block_start" => {
            let json: Value = serde_json::from_str(content).ok()?;
            let block_type = json.get("type")?.as_str()?;
            if block_type == "text" {
                Some(LlmEvent::TextStart {
                    id: "text".to_string(),
                })
            } else if block_type == "thinking" {
                Some(LlmEvent::ReasoningStart {
                    id: "reasoning".to_string(),
                })
            } else {
                None
            }
        }
        "content_block_delta" => {
            let json: Value = serde_json::from_str(content).ok()?;
            let delta_type = json.get("type")?.as_str()?;
            if delta_type == "text_delta" {
                let text = json.get("text")?.as_str()?.to_string();
                if !text.is_empty() {
                    return Some(LlmEvent::TextDelta {
                        id: "text".to_string(),
                        text,
                    });
                }
            } else if delta_type == "thinking_delta" {
                let text = json
                    .get("thinking")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                if !text.is_empty() {
                    return Some(LlmEvent::ReasoningDelta {
                        id: "reasoning".to_string(),
                        text,
                    });
                }
            }
            None
        }
        "content_block_stop" => {
            let json: Value = serde_json::from_str(content).ok()?;
            let index = json.get("index")?.as_u64()? as u32;
            Some(LlmEvent::TextEnd {
                id: format!("block-{index}"),
            })
        }
        "message_delta" => {
            let json: Value = serde_json::from_str(content).ok()?;
            let delta = json.get("delta")?;
            let stop_sequence = delta.get("stop_sequence")?.as_null();
            let usage = json.get("usage").and_then(|u| parse_anthropic_usage(u));

            if stop_sequence.is_some() {
                Some(LlmEvent::Finish {
                    reason: FinishReason::Stop,
                    usage,
                })
            } else {
                None
            }
        }
        "message_stop" => Some(LlmEvent::Finish {
            reason: FinishReason::Stop,
            usage: None,
        }),
        "input_json" => None,
        _ => None,
    }
}

fn parse_anthropic_usage(value: &Value) -> Option<Usage> {
    Some(Usage {
        input_tokens: value.get("input_tokens").and_then(|v| v.as_u64()),
        output_tokens: value.get("output_tokens").and_then(|v| v.as_u64()),
        total_tokens: value.get("total_tokens").and_then(|v| v.as_u64()),
        reasoning_tokens: value.get("thinking_tokens").and_then(|v| v.as_u64()),
        cache_read_input_tokens: value
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64()),
        cache_write_input_tokens: value
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64()),
    })
}

pub fn to_llm_messages(history: &[(String, String)]) -> Vec<LlmMessage> {
    let mut messages = Vec::new();
    for (role, content) in history {
        match role.as_str() {
            "user" => messages.push(LlmMessage::user(content)),
            "assistant" => messages.push(LlmMessage::assistant(content)),
            "system" => messages.push(LlmMessage::system(content)),
            "tool" => {} // tool results handled separately
            _ => messages.push(LlmMessage::user(content)),
        }
    }
    messages
}

pub fn system_prompt(agent_name: &str, languages: &[String]) -> String {
    format!(
        "You are {}, an AI coding assistant. Languages detected in this project: {}.",
        agent_name,
        languages.join(", ")
    )
}
