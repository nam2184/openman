use async_trait::async_trait;

use super::openai_compatible_chat::OpenAiCompatibleChatProvider;
use super::{LlmError, LlmProvider, LlmStream};
use crate::llm::request::LlmRequest;

pub struct OpenAiProvider {
    inner: OpenAiCompatibleChatProvider,
}

impl OpenAiProvider {
    pub fn new(api_key: Option<String>, base_url: Option<String>) -> Self {
        Self {
            inner: OpenAiCompatibleChatProvider::new(
                "openai",
                api_key,
                base_url,
                "https://api.openai.com/v1",
                "OPENAI_API_KEY",
                &[
                    "gpt-4.1",
                    "gpt-4.1-mini",
                    "gpt-4o",
                    "gpt-4o-mini",
                    "o3",
                    "o4-mini",
                ],
            ),
        }
    }

    pub fn with_base_url(mut self, url: &str) -> Self {
        self.inner = self.inner.with_base_url(url);
        self
    }

    pub fn chat_completions_url(&self) -> String {
        self.inner.chat_completions_url()
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn provider_name(&self) -> &str {
        self.inner.provider_name()
    }

    fn supported_models(&self) -> Vec<String> {
        self.inner.supported_models()
    }

    async fn stream(&self, request: LlmRequest) -> Result<LlmStream, LlmError> {
        self.inner.stream(request).await
    }

    fn model_base_url(&self) -> Option<&str> {
        self.inner.model_base_url()
    }

    fn api_key(&self) -> Option<&str> {
        self.inner.api_key()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::events::LlmEvent;
    use crate::llm::request::LlmMessage;
    use futures_util::StreamExt;

    #[test]
    fn provider_creation() {
        let provider = OpenAiProvider::new(Some("test-key".to_string()), None);
        assert_eq!(provider.provider_name(), "openai");
        assert_eq!(provider.model_base_url(), Some("https://api.openai.com/v1"));
        assert_eq!(
            provider.chat_completions_url(),
            "https://api.openai.com/v1/chat/completions"
        );
        assert!(provider.api_key().is_some());
    }

    #[tokio::test]
    async fn stream_produces_events_or_http_error() {
        let provider = OpenAiProvider::new(Some("invalid-openai-key".to_string()), None);
        let request =
            LlmRequest::new("gpt-4o-mini", "openai").with_message(LlmMessage::user("say hello"));

        let result = provider.stream(request).await;

        if let Ok(stream) = result {
            let events: Vec<_> = stream.events.collect().await;
            assert!(
                !events.is_empty(),
                "stream should produce at least one event"
            );
            let has_text_or_error = events.iter().any(|e| {
                matches!(e, LlmEvent::TextDelta { .. })
                    || matches!(e, LlmEvent::ProviderError { .. })
                    || matches!(e, LlmEvent::Finish { .. })
            });
            assert!(
                has_text_or_error,
                "stream should contain text, error, or finish event: {events:?}"
            );
        } else {
            let err = match result {
                Err(e) => e,
                _ => unreachable!("expected error"),
            };
            assert!(
                err.code.starts_with("http_"),
                "should be an HTTP error, got: {} - {}",
                err.code,
                err.message
            );
        }
    }
}
