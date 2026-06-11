use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::database::{Database, ProviderConfigRepository};
use crate::{ProviderConfig, ProviderProtocol};

pub struct ProviderService {
    db_path: PathBuf,
    configs: RwLock<Vec<ProviderConfig>>,
}

impl ProviderService {
    pub fn new(db_path: PathBuf) -> Arc<Self> {
        let service = Arc::new(Self {
            db_path,
            configs: RwLock::new(Vec::new()),
        });
        if let Err(e) = service.load() {
            tracing::warn!("Failed to load provider configs: {}", e);
        }
        service
    }

    pub fn with_defaults() -> Arc<Self> {
        let service = Arc::new(Self {
            db_path: PathBuf::new(),
            configs: RwLock::new(Self::default_configs()),
        });
        service
    }

    fn default_configs() -> Vec<ProviderConfig> {
        vec![
            ProviderConfig::new(
                "anthropic".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
                ProviderProtocol::Anthropic,
            ),
            ProviderConfig::new(
                "openai".to_string(),
                "gpt-4o".to_string(),
                ProviderProtocol::OpenAI,
            ),
            ProviderConfig::new(
                "minimax".to_string(),
                "MiniMax-M3".to_string(),
                ProviderProtocol::OpenAI,
            ),
        ]
    }

    fn db(&self) -> Result<Database, String> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let db = Database::new(self.db_path.clone()).map_err(|e| e.to_string())?;
        db.init()?;
        Ok(db)
    }

    pub fn load(&self) -> Result<(), String> {
        if self.db_path.as_os_str().is_empty() {
            return Ok(());
        }
        let db = self.db()?;
        let mut configs = ProviderConfigRepository::list(&db)?;
        if configs.is_empty() {
            configs = Self::default_configs();
            for config in &configs {
                ProviderConfigRepository::upsert(&db, config)?;
            }
        }
        *self.configs.write() = configs;
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        if self.db_path.as_os_str().is_empty() {
            return Ok(());
        }
        let configs = self.configs.read().clone();
        let db = self.db()?;
        for config in configs {
            ProviderConfigRepository::upsert(&db, &config)?;
        }
        Ok(())
    }

    pub fn get_configs(&self) -> Vec<ProviderConfig> {
        self.configs.read().clone()
    }

    pub fn get_config(&self, name: &str) -> Option<ProviderConfig> {
        self.configs.read().iter().find(|c| c.name == name).cloned()
    }

    pub fn upsert_config(&self, config: ProviderConfig) -> Result<(), String> {
        {
            let mut configs = self.configs.write();
            if let Some(existing) = configs.iter_mut().find(|c| c.name == config.name) {
                *existing = config;
            } else {
                configs.push(config);
            }
        }
        self.save()
    }

    pub fn delete_config(&self, name: &str) -> Result<(), String> {
        {
            let mut configs = self.configs.write();
            configs.retain(|c| c.name != name);
        }
        if !self.db_path.as_os_str().is_empty() {
            let db = self.db()?;
            ProviderConfigRepository::delete(&db, name)?;
        }
        Ok(())
    }

    pub fn get_enabled(&self) -> Option<ProviderConfig> {
        self.configs.read().iter().find(|c| c.enabled).cloned()
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<(), String> {
        {
            let mut configs = self.configs.write();
            if let Some(config) = configs.iter_mut().find(|c| c.name == name) {
                config.enabled = enabled;
            }
        }
        self.save()
    }
}

impl Default for ProviderService {
    fn default() -> Self {
        Self {
            db_path: PathBuf::new(),
            configs: RwLock::new(Vec::new()),
        }
    }
}
