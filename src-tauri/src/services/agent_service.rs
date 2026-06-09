use std::sync::Arc;

use openman_agents::{
    llm::providers::{AnthropicProvider, MiniMaxTokenPlanProvider, OpenAiProvider},
    llm::ContentPart,
    ConversationService, LlmProvider, MessageRole, ProviderProtocol, ProviderRegistry,
    ProviderService, SessionError, SessionRunEvent, SessionRunner, SessionService,
};
use tauri::{AppHandle, Emitter};

const AGENT_EVENT: &str = "agent:event";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AgentUiEvent {
    Started {
        session_id: String,
    },
    LlmEvent {
        session_id: String,
        step: u32,
        event: openman_agents::llm::LlmEvent,
    },
    Finished {
        session_id: String,
        response: String,
    },
    Error {
        session_id: String,
        message: String,
    },
}

pub struct AgentService {
    providers: Arc<ProviderRegistry>,
    session_service: Arc<SessionService>,
    conversation_service: Arc<ConversationService>,
    provider_service: Arc<ProviderService>,
}

impl AgentService {
    pub fn new(
        session_service: Arc<SessionService>,
        conversation_service: Arc<ConversationService>,
        provider_service: Arc<ProviderService>,
    ) -> Arc<Self> {
        let providers = Arc::new(ProviderRegistry::new());
        providers.register_defaults_sync();

        Arc::new(Self {
            providers,
            session_service,
            conversation_service,
            provider_service,
        })
    }

    pub fn providers(&self) -> &Arc<ProviderRegistry> {
        &self.providers
    }

    pub async fn refresh_provider(&self, name: &str) {
        let config = match self.provider_service.get_config(name) {
            Some(c) if c.enabled => c,
            _ => return,
        };

        let provider: Arc<dyn LlmProvider> = match &config.protocol {
            ProviderProtocol::OpenAI if config.name == "minimax" => {
                Arc::new(MiniMaxTokenPlanProvider::from_config(&config))
            }
            ProviderProtocol::OpenAI if config.name == "openai" => Arc::new(OpenAiProvider::new(
                config.api_key.clone(),
                config.base_url.clone(),
            )),
            ProviderProtocol::Anthropic if config.name == "anthropic" => Arc::new(AnthropicProvider::new(
                config.api_key.clone(),
                config.base_url.clone(),
            )),
            _ => return,
        };

        self.providers.register(provider).await;
    }

    pub async fn update_session_provider(
        &self,
        session_id: &str,
        provider: String,
        model: String,
    ) -> Result<(), String> {
        self.session_service
            .update_session_provider(session_id, &provider, &model)
    }

    pub async fn send_message(
        &self,
        session_id: &str,
        message: String,
        app: AppHandle,
    ) -> Result<String, String> {
        emit_agent_event(
            &app,
            AgentUiEvent::Started {
                session_id: session_id.to_string(),
            },
        );

        self.conversation_service
            .append_message(session_id, MessageRole::User, message)?;

        let session = self
            .session_service
            .get_session(session_id)?
            .ok_or_else(|| "Session not found".to_string())?;

        self.refresh_provider(&session.provider).await;

        let session_id_clone = session_id.to_string();
        let session_service = Arc::clone(&self.session_service);
        let conversation_service = Arc::clone(&self.conversation_service);
        let providers = Arc::clone(&self.providers);
        let app_for_events = app.clone();
        let event_sink = Arc::new(move |event: SessionRunEvent| {
            emit_agent_event(
                &app_for_events,
                AgentUiEvent::LlmEvent {
                    session_id: event.session_id,
                    step: event.step,
                    event: event.event,
                },
            );
        });

        let run_result = tokio::task::spawn_blocking(move || {
            let runner = SessionRunner::new(session_service, conversation_service, providers)
                .with_event_sink(event_sink);
            let rt = tokio::runtime::Handle::current();
            rt.block_on(runner.run(&session_id_clone))
        })
        .await
        .map_err(|e| e.to_string())?;

        if let Err(error) = run_result {
            let message = chat_error_message(&error);
            if let Err(append_error) =
                append_assistant_error(&self.conversation_service, session_id, &message)
            {
                tracing::warn!("failed to append LLM error to chat: {}", append_error);
            }
            emit_agent_event(
                &app,
                AgentUiEvent::Error {
                    session_id: session_id.to_string(),
                    message: message.clone(),
                },
            );
            return Err(message);
        }

        let messages = self.conversation_service.get_messages(session_id)?;
        let response = messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .map(|m| m.content.clone())
            .unwrap_or_default();

        emit_agent_event(
            &app,
            AgentUiEvent::Finished {
                session_id: session_id.to_string(),
                response: response.clone(),
            },
        );

        Ok(response)
    }
}

fn emit_agent_event(app: &AppHandle, event: AgentUiEvent) {
    if let Err(error) = app.emit(AGENT_EVENT, event) {
        tracing::warn!("failed to emit agent event: {}", error);
    }
}

fn chat_error_message(error: &SessionError) -> String {
    match error {
        SessionError::Llm(err) => format!("LLM error: {}", err),
        SessionError::Provider(message) => format!("LLM provider error: {}", message),
        SessionError::NoProviderForSession => {
            "LLM error: no provider is configured for this session.".to_string()
        }
        SessionError::StepLimitExceeded { limit, .. } => {
            format!("LLM error: stopped after reaching the {limit}-step limit.")
        }
        _ => error.to_string(),
    }
}

fn append_assistant_error(
    conversation_service: &ConversationService,
    session_id: &str,
    message: &str,
) -> Result<(), String> {
    let content = serde_json::to_string(&vec![ContentPart::text(message)])
        .unwrap_or_else(|_| message.to_string());
    conversation_service
        .append_message(session_id, MessageRole::Assistant, content)
        .map(|_| ())
}
