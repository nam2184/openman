use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::Stream;

use super::events::LlmEvent;
use super::request::{LlmError, LlmMessage, LlmRequest, LlmResponse};

pub mod anthropic;
pub mod minimax_token_plan;
pub mod openai;
mod openai_compatible_chat;

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

/// Helper used by provider implementations to extract the JSON
/// payload from an SSE `data: ...` line. Returns `None` if the
/// line is not a `data:` line.
pub fn parse_sse_line(line: &str) -> Option<String> {
    const PREFIX: &str = "data: ";
    line.strip_prefix(PREFIX).map(|s| s.to_string())
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
