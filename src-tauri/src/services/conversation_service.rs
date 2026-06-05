use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::Mutex;
use openman_agents::{Message, MessageRole};

pub struct ConversationService {
    base_dir: PathBuf,
    locks: Mutex<std::collections::HashMap<String, Arc<Mutex<()>>>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversationFile {
    pub session_id: String,
    pub messages: Vec<ConversationMessage>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversationMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

impl ConversationService {
    pub fn new(base_dir: PathBuf) -> Arc<Self> {
        if let Some(parent) = base_dir.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::create_dir_all(&base_dir);
        Arc::new(Self {
            base_dir,
            locks: Mutex::new(std::collections::HashMap::new()),
        })
    }

    fn get_lock(&self, session_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock();
        locks.entry(session_id.to_string()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
    }

    fn ai_file_path(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(format!("{}.json", session_id))
    }

    fn ui_file_path(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(format!("{}.ui.json", session_id))
    }

    fn lock_file_path(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(format!("{}.lock", session_id))
    }

    pub fn append_message(&self, session_id: &str, role: MessageRole, content: String) -> Result<String, String> {
        let _lock = self.get_lock(session_id);
        let lock_path = self.lock_file_path(session_id);
        let _guard = self.acquire_lock(&lock_path)?;

        let mut conv = self.read_ai_conversation(session_id)?;
        let message = ConversationMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: match role {
                MessageRole::User => "user".to_string(),
                MessageRole::Assistant => "assistant".to_string(),
                MessageRole::System => "system".to_string(),
            },
            content,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        conv.messages.push(message);
        let message_id = message.id.clone();
        self.write_ai_conversation(session_id, &conv)?;
        self.write_ui_conversation(session_id, &conv)?;

        Ok(message_id)
    }

    pub fn read_ai_conversation(&self, session_id: &str) -> Result<ConversationFile, String> {
        let path = self.ai_file_path(session_id);
        if !path.exists() {
            return Ok(ConversationFile {
                session_id: session_id.to_string(),
                messages: Vec::new(),
                summary: None,
            });
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read conversation file: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse conversation file: {}", e))
    }

    pub fn read_ui_conversation(&self, session_id: &str) -> Result<ConversationFile, String> {
        let path = self.ui_file_path(session_id);
        if !path.exists() {
            return self.read_ai_conversation(session_id);
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read UI conversation file: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse UI conversation file: {}", e))
    }

    pub fn get_messages(&self, session_id: &str) -> Result<Vec<ConversationMessage>, String> {
        let conv = self.read_ai_conversation(session_id)?;
        Ok(conv.messages)
    }

    pub fn compact_conversation(&self, session_id: &str, summary: String) -> Result<(), String> {
        let _lock = self.get_lock(session_id);
        let lock_path = self.lock_file_path(session_id);
        let _guard = self.acquire_lock(&lock_path)?;

        let mut conv = self.read_ai_conversation(session_id)?;
        conv.summary = Some(summary);
        conv.messages.clear();
        self.write_ai_conversation(session_id, &conv)?;
        self.write_ui_conversation(session_id, &conv)?;

        Ok(())
    }

    pub fn delete_conversation(&self, session_id: &str) -> Result<(), String> {
        let _lock = self.get_lock(session_id);

        for path in [self.ai_file_path(session_id), self.ui_file_path(session_id), self.lock_file_path(session_id)] {
            if path.exists() {
                std::fs::remove_file(&path)
                    .map_err(|e| format!("Failed to delete conversation file: {}", e))?;
            }
        }
        Ok(())
    }

    pub fn get_conversation_path(&self, session_id: &str) -> PathBuf {
        self.ai_file_path(session_id)
    }

    fn write_ai_conversation(&self, session_id: &str, conv: &ConversationFile) -> Result<(), String> {
        let path = self.ai_file_path(session_id);
        let content = serde_json::to_string_pretty(conv)
            .map_err(|e| format!("Failed to serialize conversation: {}", e))?;
        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write AI conversation file: {}", e))?;
        Ok(())
    }

    fn write_ui_conversation(&self, session_id: &str, conv: &ConversationFile) -> Result<(), String> {
        let path = self.ui_file_path(session_id);
        let content = serde_json::to_string_pretty(conv)
            .map_err(|e| format!("Failed to serialize UI conversation: {}", e))?;
        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write UI conversation file: {}", e))?;
        Ok(())
    }

    fn acquire_lock(&self, lock_path: &Path) -> Result<LockGuard, String> {
        LockGuard::acquire(lock_path)
    }
}

struct LockGuard;

impl LockGuard {
    fn acquire(path: &Path) -> Result<Self, String> {
        let mut attempts = 0;
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(file) => {
                    let _ = file;
                    return Ok(LockGuard);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    attempts += 1;
                    if attempts > 100 {
                        return Err("Failed to acquire lock after 100 attempts".to_string());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => return Err(format!("Failed to create lock file: {}", e)),
            }
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
    }
}

pub fn create_conversation_service(base_dir: PathBuf) -> Arc<ConversationService> {
    ConversationService::new(base_dir)
}