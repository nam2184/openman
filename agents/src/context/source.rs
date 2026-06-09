use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextSnapshot {
    pub key: String,
    pub value: Value,
    pub rendered: String,
}

#[derive(Debug, Clone)]
pub struct LoadedContext {
    pub key: String,
    pub value: Value,
    pub rendered: String,
}

pub trait ContextSource: Send + Sync {
    fn key(&self) -> &str;
    fn load(&self) -> Result<LoadedContext, String>;
}

impl LoadedContext {
    pub fn snapshot(&self) -> ContextSnapshot {
        ContextSnapshot {
            key: self.key.clone(),
            value: self.value.clone(),
            rendered: self.rendered.clone(),
        }
    }
}
