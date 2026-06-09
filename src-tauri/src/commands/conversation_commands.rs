use openman_agents::{ConversationFile, ConversationMessage, ConversationService, MessageRole};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn append_message(
    session_id: String,
    role: String,
    content: String,
    conversation_service: State<'_, Arc<ConversationService>>,
) -> Result<String, String> {
    let message_role = match role.to_lowercase().as_str() {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "system" => MessageRole::System,
        _ => return Err(format!("Invalid role: {}", role)),
    };
    conversation_service.append_message(&session_id, message_role, content)
}

#[tauri::command]
pub async fn get_messages(
    session_id: String,
    conversation_service: State<'_, Arc<ConversationService>>,
) -> Result<Vec<ConversationMessage>, String> {
    conversation_service.get_messages(&session_id)
}

#[tauri::command]
pub async fn get_ai_conversation(
    session_id: String,
    conversation_service: State<'_, Arc<ConversationService>>,
) -> Result<ConversationFile, String> {
    conversation_service.read_ai_conversation(&session_id)
}

#[tauri::command]
pub async fn get_ui_conversation(
    session_id: String,
    conversation_service: State<'_, Arc<ConversationService>>,
) -> Result<ConversationFile, String> {
    conversation_service.read_ui_conversation(&session_id)
}

#[tauri::command]
pub async fn compact_conversation(
    session_id: String,
    summary: String,
    conversation_service: State<'_, Arc<ConversationService>>,
) -> Result<(), String> {
    conversation_service.compact_conversation(&session_id, summary)
}

#[tauri::command]
pub async fn delete_conversation(
    session_id: String,
    conversation_service: State<'_, Arc<ConversationService>>,
) -> Result<(), String> {
    conversation_service.delete_conversation(&session_id)
}
