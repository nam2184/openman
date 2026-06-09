use openman_agents::{ProviderConfig, ProviderService};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_provider_configs(
    provider_service: State<'_, Arc<ProviderService>>,
) -> Result<Vec<ProviderConfig>, String> {
    Ok(provider_service.get_configs())
}

#[tauri::command]
pub async fn get_provider_config(
    name: String,
    provider_service: State<'_, Arc<ProviderService>>,
) -> Result<Option<ProviderConfig>, String> {
    Ok(provider_service.get_config(&name))
}

#[tauri::command]
pub async fn upsert_provider_config(
    config: ProviderConfig,
    provider_service: State<'_, Arc<ProviderService>>,
) -> Result<(), String> {
    provider_service.upsert_config(config)
}

#[tauri::command]
pub async fn delete_provider_config(
    name: String,
    provider_service: State<'_, Arc<ProviderService>>,
) -> Result<(), String> {
    provider_service.delete_config(&name)
}

#[tauri::command]
pub async fn set_active_provider(
    name: String,
    provider_service: State<'_, Arc<ProviderService>>,
) -> Result<(), String> {
    let configs = provider_service.get_configs();
    for config in configs {
        let enabled = config.name == name;
        provider_service.set_enabled(&config.name, enabled)?;
    }
    Ok(())
}
