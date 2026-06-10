use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;

use crate::domain::ToolCall;
use crate::llm::events::{FinishReason, LlmEvent, ToolResultValue};
use crate::llm::providers::LlmProvider;
use crate::llm::request::{ContentPart, LlmMessage, LlmRequest};
use crate::llm::subagent_registry::SubagentRegistry;
use crate::permission::{PermissionMode, PermissionService};
use crate::sessions::conversation::{ConversationMessage, ConversationService};
use crate::sessions::service::SessionService;
use crate::tools::{run_tool_async, run_tool_with_context, ToolContext, ToolRuntime};

const MAX_STEPS: u32 = 25;

pub type SessionEventSink = Arc<dyn Fn(SessionRunEvent) + Send + Sync>;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionRunEvent {
    pub session_id: String,
    pub step: u32,
    pub event: LlmEvent,
}

pub struct SessionRunner {
    session_service: Arc<SessionService>,
    conversation_service: Arc<ConversationService>,
    providers: Arc<ProviderRegistry>,
    subagent_registry: Option<Arc<SubagentRegistry>>,
    max_steps: u32,
    readonly_tools: bool,
    event_sink: Option<SessionEventSink>,
}

impl SessionRunner {
    pub fn new(
        session_service: Arc<SessionService>,
        conversation_service: Arc<ConversationService>,
        providers: Arc<ProviderRegistry>,
    ) -> Self {
        Self {
            session_service,
            conversation_service,
            providers,
            subagent_registry: None,
            max_steps: MAX_STEPS,
            readonly_tools: false,
            event_sink: None,
        }
    }

    pub fn with_event_sink(mut self, event_sink: SessionEventSink) -> Self {
        self.event_sink = Some(event_sink);
        self
    }

    /// Cap the number of LLM turns a single `run` invocation may take.
    /// Used by the `task` and `ask_peer` tools to bound child sessions.
    pub fn with_max_turns(mut self, max_turns: u32) -> Self {
        self.max_steps = max_turns;
        self
    }

    /// Restrict this runner's tools to the read-only subset used by
    /// `ask_peer` children. The runner drops `write`, `edit`,
    /// `apply_patch`, `shell`, `task`, and `ask_peer` from the tool
    /// definitions it advertises to the LLM.
    pub fn with_readonly_tools(mut self, readonly: bool) -> Self {
        self.readonly_tools = readonly;
        self
    }

    /// Wire the SubagentRegistry into the runner so the `task` and
    /// `ask_peer` tools have access. The agent service calls this on
    /// its top-level runner; the `task`/`ask_peer` tools build their
    /// own runners without it.
    pub fn with_subagent_registry(mut self, registry: Arc<SubagentRegistry>) -> Self {
        self.subagent_registry = Some(registry);
        self
    }

    fn emit_event(&self, session_id: &str, step: u32, event: &LlmEvent) {
        if let Some(sink) = &self.event_sink {
            sink(SessionRunEvent {
                session_id: session_id.to_string(),
                step,
                event: event.clone(),
            });
        }
    }

    pub async fn run(&self, session_id: &str) -> Result<RunResult, SessionError> {
        let mut step = 0u32;
        let mut needs_continuation = true;

        while needs_continuation && step < self.max_steps {
            match self.run_turn(session_id, step).await {
                Ok(needs_continue) => {
                    needs_continuation = needs_continue;
                    step += 1;
                }
                Err(e) => {
                    tracing::warn!("turn error for session {} step {}: {}", session_id, step, e);
                    return Err(e);
                }
            }
        }

        if needs_continuation {
            return Err(SessionError::StepLimitExceeded {
                session_id: session_id.to_string(),
                limit: self.max_steps,
            });
        }

        Ok(RunResult { steps: step })
    }

    async fn run_turn(&self, session_id: &str, step: u32) -> Result<bool, SessionError> {
        let session = self
            .session_service
            .get_session(session_id)
            .map_err(|e| SessionError::Conversation(e))?
            .ok_or_else(|| SessionError::SessionNotFound(session_id.to_string()))?;

        // Drain completed sub-agent results into the conversation so the
        // LLM sees them as ordinary context on this turn.
        if let Some(registry) = &self.subagent_registry {
            let completions = registry.take_completions(session_id);
            for c in completions {
                let prefix = match c.kind {
                    crate::llm::subagent_registry::ChildKind::Task => "task_result",
                    crate::llm::subagent_registry::ChildKind::AskPeer => "peer_answer",
                };
                let block = format!(
                    "<{prefix} id=\"{cid}\" state=\"{st}\">\n{text}\n</{prefix}>",
                    prefix = prefix,
                    cid = c.child_session_id,
                    st = if c.success { "completed" } else { "error" },
                    text = c.text,
                );
                if let Err(e) = self.conversation_service.append_message(
                    session_id,
                    crate::MessageRole::User,
                    block,
                ) {
                    tracing::warn!("failed to append child completion: {e}");
                }
            }
        }

        let history = self
            .conversation_service
            .get_messages(session_id)
            .map_err(|e| SessionError::Conversation(e))?;

        let messages = history_to_llm_messages(&history);

        let system_prompt = system_prompt_for_session(&session.provider, &[]);

        let tools = if self.readonly_tools {
            readonly_tool_definitions()
        } else {
            default_tool_definitions()
        };

        let request = LlmRequest::new(&session.model, &session.provider)
            .with_system(system_prompt)
            .with_messages(messages)
            .with_tools(tools);

        let provider = self
            .providers
            .get(&session.provider)
            .await
            .ok_or(SessionError::NoProviderForSession)?;

        let stream = provider
            .stream(request)
            .await
            .map_err(|e| SessionError::Llm(e))?;

        let mut pending_tool_calls: Vec<(String, String, String)> = Vec::new();
        let mut accumulated_text = String::new();
        let mut assistant_parts: Vec<ContentPart> = Vec::new();
        let mut needs_continuation = false;
        let mut finish_reason = FinishReason::Unknown;

        tokio::pin!(stream);

        while let Some(event) = stream.events.as_mut().next().await {
            self.emit_event(session_id, step, &event);

            match event {
                LlmEvent::TextDelta { text, .. } => {
                    accumulated_text.push_str(&text);
                }
                LlmEvent::TextEnd { .. } => {
                    if !accumulated_text.is_empty() {
                        assistant_parts.push(ContentPart::text(accumulated_text.clone()));
                        accumulated_text.clear();
                    }
                }
                LlmEvent::ReasoningDelta { text, .. } => {
                    if !text.is_empty() {
                        assistant_parts.push(ContentPart::reasoning(&text));
                    }
                }
                LlmEvent::ToolInputDelta { id, name, text } => {
                    if let Some((_, _, buf)) =
                        pending_tool_calls.iter_mut().find(|(cid, _, _)| cid == &id)
                    {
                        buf.push_str(&text);
                    }
                }
                LlmEvent::ToolCall {
                    id,
                    name,
                    input,
                    provider_executed: _,
                } => {
                    needs_continuation = true;
                    let input_str = serde_json::to_string(&input).unwrap_or_default();
                    pending_tool_calls.push((id, name, input_str));
                }
                LlmEvent::ToolResult {
                    id, name, result, ..
                } => {
                    let result_json = match result {
                        ToolResultValue::Text { value } => serde_json::json!({ "text": value }),
                        ToolResultValue::Error { value } => serde_json::json!({ "error": value }),
                        ToolResultValue::Json { value } => value,
                        ToolResultValue::Content { value } => {
                            serde_json::json!({ "content": value })
                        }
                    };
                    assistant_parts.push(ContentPart::tool_result(&id, &name, result_json));
                }
                LlmEvent::StepFinish { reason, .. } => {
                    finish_reason = reason;
                }
                LlmEvent::Finish { reason, .. } => {
                    finish_reason = reason;
                }
                LlmEvent::ToolError { id, name, message } => {
                    let result = serde_json::json!({ "error": message });
                    assistant_parts.push(ContentPart::tool_result(&id, &name, result));
                }
                LlmEvent::ProviderError { message } => {
                    tracing::error!("provider error during step {}: {}", step, message);
                    return Err(SessionError::Provider(message));
                }
                _ => {}
            }
        }

        if !accumulated_text.is_empty() {
            assistant_parts.push(ContentPart::text(accumulated_text));
        }

        if !pending_tool_calls.is_empty() {
            let permission = PermissionService::new(PermissionMode::Build);
            let ctx = ToolContext::new(PermissionMode::Build);

            for (id, name, input_str) in pending_tool_calls {
                let input: serde_json::Value = serde_json::from_str(&input_str).unwrap_or_default();
                let arguments = if let Some(obj) = input.as_object() {
                    obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                } else {
                    std::collections::HashMap::new()
                };
                let tool_call = ToolCall {
                    name: name.clone(),
                    arguments,
                };

                if let Err(err) = permission.assert_tool_call(&tool_call) {
                    let event = LlmEvent::ToolError {
                        id: id.clone(),
                        name: name.clone(),
                        message: err.to_string(),
                    };
                    self.emit_event(session_id, step, &event);

                    let _ = stream
                        .inject_tool_result(
                            &id,
                            &name,
                            serde_json::json!({ "error": err.to_string() }),
                        )
                        .await;
                    continue;
                }

                // Dispatch through the async path so the `task` and
                // `ask_peer` tools can drive child sessions. For
                // read-only runs (e.g. an `ask_peer` child), the runner
                // has no ToolRuntime; it falls back to the sync path
                // inside `run_tool_async` for non-task tools, but task
                // / ask_peer are unreachable because the tool set is
                // already filtered.
                let result = if let Some(registry) = &self.subagent_registry {
                    let runtime = ToolRuntime {
                        caller_session_id: session_id.to_string(),
                        session_service: Arc::clone(&self.session_service),
                        conversation_service: Arc::clone(&self.conversation_service),
                        subagent_registry: Arc::clone(registry),
                    };
                    run_tool_async(&tool_call, &runtime).await
                } else {
                    run_tool_with_context(&tool_call, &ctx)
                };

                let (result_value, output) = if result.success {
                    let output = result.output.clone();
                    (serde_json::json!({ "text": output }), Some(result.output))
                } else {
                    let error = result.error.unwrap_or_default();
                    (serde_json::json!({ "error": error }), None)
                };

                let event = LlmEvent::ToolResult {
                    id: id.clone(),
                    name: name.clone(),
                    result: ToolResultValue::Json {
                        value: result_value.clone(),
                    },
                    output,
                };
                self.emit_event(session_id, step, &event);

                let _ = stream.inject_tool_result(&id, &name, result_value).await;
            }
        }

        let assistant_content =
            serde_json::to_string(&assistant_parts).unwrap_or_else(|_| "[]".to_string());

        self.conversation_service
            .append_message(
                session_id,
                crate::domain::MessageRole::Assistant,
                assistant_content,
            )
            .map_err(|e| SessionError::Conversation(e))?;

        let should_continue = matches!(
            finish_reason,
            FinishReason::ToolCalls | FinishReason::Length
        ) && needs_continuation;

        Ok(should_continue)
    }
}

fn history_to_llm_messages(history: &[ConversationMessage]) -> Vec<LlmMessage> {
    history
        .iter()
        .filter(|msg| !msg.content.trim().is_empty())
        .map(|msg| match msg.role.as_str() {
            "user" => LlmMessage::user(&msg.content),
            "assistant" => {
                if let Ok(parts) = serde_json::from_str::<Vec<ContentPart>>(&msg.content) {
                    LlmMessage {
                        role: "assistant".to_string(),
                        content: parts,
                    }
                } else {
                    LlmMessage::assistant(&msg.content)
                }
            }
            "system" => LlmMessage::system(&msg.content),
            _ => LlmMessage::user(&msg.content),
        })
        .collect()
}

fn system_prompt_for_session(provider: &str, _extra: &[String]) -> String {
    let agent_name = match provider {
        "anthropic" => "Claude",
        "openai" => "GPT",
        "minimax" => "MiniMax",
        _ => "AI Assistant",
    };
    format!("You are {}, a helpful AI coding assistant.", agent_name)
}

fn default_tool_definitions() -> Vec<crate::llm::events::ToolDefinition> {
    vec![
        crate::llm::events::ToolDefinition::new(
            "read",
            "Read a file from disk",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the file to read" },
                    "offset": { "type": "integer", "description": "Line offset to start reading from", "minimum": 1 },
                    "limit": { "type": "integer", "description": "Maximum number of lines to read" }
                },
                "required": ["path"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "write",
            "Write content to a file",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the file to write" },
                    "content": { "type": "string", "description": "Content to write" }
                },
                "required": ["path", "content"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "edit",
            "Replace text in an existing file",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the file to edit" },
                    "old_string": { "type": "string", "description": "Text to find and replace" },
                    "new_string": { "type": "string", "description": "Replacement text" }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "glob",
            "Find files by glob pattern",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Root directory to search from" },
                    "pattern": { "type": "string", "description": "Glob pattern to match files against" }
                },
                "required": ["path"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "grep",
            "Search file contents",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Root directory to search from" },
                    "pattern": { "type": "string", "description": "Text pattern to search for" },
                    "include": { "type": "string", "description": "File name pattern to filter by" }
                },
                "required": ["path", "pattern"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "shell",
            "Run a shell command",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to execute" },
                    "workdir": { "type": "string", "description": "Working directory for the command" }
                },
                "required": ["command"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "apply_patch",
            "Apply a file-oriented patch",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "patchText": { "type": "string", "description": "Full patch text describing file operations" }
                },
                "required": ["patchText"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "todo",
            "Update the session todo list",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "Todo content to set" }
                },
                "required": ["content"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "plan",
            "Enter or exit planning mode",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "enum": ["plan", "build"], "description": "Mode to switch to" }
                }
            }),
        ),
    ]
}

/// Read-only tool subset used by `ask_peer` child sessions. Excludes
/// `write`, `edit`, `apply_patch`, `shell`, `task`, and `ask_peer`
/// itself. The peer can read files, search, and fetch the web.
pub fn readonly_tool_definitions() -> Vec<crate::llm::events::ToolDefinition> {
    vec![
        crate::llm::events::ToolDefinition::new(
            "read",
            "Read a file from disk",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to a file to read" },
                    "offset": { "type": "integer", "minimum": 1 },
                    "limit": { "type": "integer", "minimum": 1 }
                },
                "required": ["path"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "glob",
            "Find files by glob pattern",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Root directory" },
                    "pattern": { "type": "string", "description": "Glob pattern" }
                },
                "required": ["path"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "grep",
            "Search file contents",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "pattern": { "type": "string" },
                    "include": { "type": "string" }
                },
                "required": ["path", "pattern"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "webfetch",
            "Fetch a web URL",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string" }
                },
                "required": ["url"]
            }),
        ),
        crate::llm::events::ToolDefinition::new(
            "websearch",
            "Search the web",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
        ),
    ]
}

#[derive(Debug)]
pub struct RunResult {
    pub steps: u32,
}

#[derive(Debug)]
pub enum SessionError {
    SessionNotFound(String),
    NoProviderForSession,
    Conversation(String),
    Llm(crate::llm::request::LlmError),
    Provider(String),
    StepLimitExceeded { session_id: String, limit: u32 },
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::SessionNotFound(id) => write!(f, "session not found: {id}"),
            SessionError::NoProviderForSession => {
                write!(f, "no provider configured for session")
            }
            SessionError::Conversation(msg) => write!(f, "conversation error: {msg}"),
            SessionError::Llm(err) => write!(f, "LLM error: {err}"),
            SessionError::Provider(msg) => write!(f, "provider error: {msg}"),
            SessionError::StepLimitExceeded { session_id, limit } => {
                write!(f, "session {session_id} exceeded step limit {limit}")
            }
        }
    }
}

impl std::error::Error for SessionError {}

pub struct ProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn LlmProvider>>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register(&self, provider: Arc<dyn LlmProvider>) {
        let name = provider.provider_name().to_string();
        self.providers.write().await.insert(name, provider);
    }

    pub async fn get(&self, name: &str) -> Option<Arc<dyn LlmProvider>> {
        self.providers.read().await.get(name).cloned()
    }

    pub fn register_defaults_sync(&self) {
        let openai = Arc::new(crate::llm::providers::OpenAiProvider::new(None, None))
            as Arc<dyn LlmProvider>;
        let anthropic = Arc::new(crate::llm::providers::AnthropicProvider::new(None, None))
            as Arc<dyn LlmProvider>;
        let minimax = Arc::new(crate::llm::providers::MiniMaxTokenPlanProvider::new(None, None))
            as Arc<dyn LlmProvider>;

        // Note: using blocking insert since we're in a sync context
        // In async context use register() which does .write().await
        let mut guard = self.providers.blocking_write();
        guard.insert("openai".to_string(), openai);
        guard.insert("anthropic".to_string(), anthropic);
        guard.insert("minimax".to_string(), minimax);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_to_llm_messages_empty() {
        let history = vec![];
        let messages = history_to_llm_messages(&history);
        assert!(messages.is_empty());
    }

    #[test]
    fn history_to_llm_messages_user() {
        let history = vec![ConversationMessage {
            id: "1".to_string(),
            role: "user".to_string(),
            content: "hello".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        }];
        let messages = history_to_llm_messages(&history);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
    }

    #[test]
    fn system_prompt_for_session_names() {
        assert!(system_prompt_for_session("anthropic", &[]).contains("Claude"));
        assert!(system_prompt_for_session("openai", &[]).contains("GPT"));
        assert!(system_prompt_for_session("minimax", &[]).contains("MiniMax"));
    }
}
