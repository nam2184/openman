use crate::services::settings_service::{AppSettings, SettingsService};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_settings(
    settings_service: State<'_, Arc<SettingsService>>,
) -> Result<AppSettings, String> {
    Ok(settings_service.get_settings())
}

#[tauri::command]
pub async fn save_settings(
    settings_service: State<'_, Arc<SettingsService>>,
) -> Result<(), String> {
    settings_service.save()
}
