use std::sync::Arc;
use tauri::{AppHandle, State};

use crate::services::agent_service::AgentService;

#[tauri::command]
pub async fn send_message(
    session_id: String,
    message: String,
    app: AppHandle,
    agent_service: State<'_, Arc<AgentService>>,
) -> Result<String, String> {
    agent_service.send_message(&session_id, message, app).await
}

#[tauri::command]
pub async fn update_session_provider(
    session_id: String,
    provider: String,
    model: String,
    agent_service: State<'_, Arc<AgentService>>,
) -> Result<(), String> {
    agent_service
        .update_session_provider(&session_id, provider, model)
        .await
}
