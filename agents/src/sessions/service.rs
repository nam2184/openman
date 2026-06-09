use std::path::PathBuf;
use std::sync::Arc;

use crate::database::{Database, ProjectRepository, SessionGroupRepository, SessionRepository};
use crate::{AgentSession, SessionGroup};

pub struct SessionService {
    db_path: PathBuf,
}

impl SessionService {
    pub fn new(db_path: PathBuf) -> Arc<Self> {
        let service = Arc::new(Self { db_path });
        if let Err(error) = service.db() {
            tracing::warn!("Failed to initialize session database: {}", error);
        }
        service
    }

    pub fn create_session(
        &self,
        project_id: String,
        directory: String,
        provider: String,
        model: String,
    ) -> Result<String, String> {
        let db = self.db()?;
        if ProjectRepository::find_by_id(&db, &project_id)?.is_none() {
            return Err("Project must exist before creating sessions".to_string());
        }

        let directory_path = PathBuf::from(&directory);
        if !directory_path.exists() || !directory_path.is_dir() {
            return Err(format!("Session directory does not exist: {directory}"));
        }

        let session = AgentSession::new(project_id, directory, provider, model);
        let id = session.id.clone();
        SessionRepository::insert(&db, &session)?;
        Ok(id)
    }

    pub fn get_session(&self, id: &str) -> Result<Option<AgentSession>, String> {
        SessionRepository::find_by_id(&self.db()?, id)
    }

    pub fn get_all_sessions(&self) -> Result<Vec<AgentSession>, String> {
        SessionRepository::list(&self.db()?)
    }

    pub fn delete_session(&self, id: &str) -> Result<(), String> {
        SessionRepository::delete(&self.db()?, id)
    }

    pub fn create_group(&self, session_ids: Vec<String>) -> Result<String, String> {
        let db = self.db()?;
        let group = SessionGroup::new(session_ids);
        let id = group.id.clone();
        SessionGroupRepository::insert(&db, &group)?;
        Ok(id)
    }

    pub fn get_all_groups(&self) -> Result<Vec<SessionGroup>, String> {
        SessionGroupRepository::list(&self.db()?)
    }

    pub fn delete_group(&self, id: &str) -> Result<(), String> {
        SessionGroupRepository::delete(&self.db()?, id)
    }

    pub fn rename_group(&self, id: &str, name: Option<String>) -> Result<(), String> {
        SessionGroupRepository::rename(&self.db()?, id, name)
    }

    pub fn add_session_to_group(&self, session_id: &str, group_id: &str) -> Result<(), String> {
        SessionGroupRepository::add_session(&self.db()?, group_id, session_id)
    }

    pub fn remove_session_from_group(&self, session_id: &str) -> Result<(), String> {
        SessionGroupRepository::remove_session(&self.db()?, session_id)
    }

    pub fn update_session_provider(
        &self,
        session_id: &str,
        provider: &str,
        model: &str,
    ) -> Result<(), String> {
        SessionRepository::update_provider(&self.db()?, session_id, provider, model)
    }

    fn db(&self) -> Result<Database, String> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let db = Database::new(self.db_path.clone()).map_err(|e| e.to_string())?;
        db.init()?;
        Ok(db)
    }
}
