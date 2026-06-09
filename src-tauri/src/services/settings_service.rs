use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: String,
    pub editor_font_size: u32,
    pub editor_tab_size: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            editor_font_size: 14,
            editor_tab_size: 2,
        }
    }
}

pub struct SettingsService {
    settings: RwLock<AppSettings>,
    config_path: PathBuf,
}

impl SettingsService {
    pub fn new(config_dir: PathBuf) -> Arc<Self> {
        let config_path = config_dir.join("settings.json");
        Arc::new(Self {
            settings: RwLock::new(AppSettings::default()),
            config_path,
        })
    }

    pub fn load(&self) -> Result<(), String> {
        if !self.config_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.config_path)
            .map_err(|e| format!("Failed to read settings: {}", e))?;

        let settings: AppSettings = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse settings: {}", e))?;

        *self.settings.write() = settings;
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        let settings = self.settings.read().clone();
        let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;

        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        std::fs::write(&self.config_path, content)
            .map_err(|e| format!("Failed to write settings: {}", e))
    }

    pub fn get_settings(&self) -> AppSettings {
        self.settings.read().clone()
    }

    pub fn update_settings(&self, updates: AppSettings) {
        *self.settings.write() = updates;
    }
}

impl Default for SettingsService {
    fn default() -> Self {
        Self {
            settings: RwLock::new(AppSettings::default()),
            config_path: PathBuf::new(),
        }
    }
}
