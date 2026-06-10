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
    settings: AppSettings,
) -> Result<(), String> {
    settings_service.update_settings(settings);
    settings_service.save()
}
