use std::path::{Path, PathBuf};
use std::sync::Arc;
use openman_agents::Project;
use crate::db::connection::Database;
use crate::db::repositories::ProjectRepository;
use crate::services::stack_detector::StackDetector;

pub struct ProjectService {
    db_path: PathBuf,
    projects: parking_lot::RwLock<std::collections::HashMap<String, Project>>,
    stack_detector: Arc<StackDetector>,
}

impl ProjectService {
    pub fn new(db_path: PathBuf, stack_detector: Arc<StackDetector>) -> Arc<Self> {
        let service = Arc::new(Self {
            db_path,
            projects: parking_lot::RwLock::new(std::collections::HashMap::new()),
            stack_detector,
        });

        if let Err(e) = service.db() {
            tracing::warn!("Failed to initialize project database: {}", e);
        }

        service
    }

    pub fn open_project(&self, path: &Path) -> Result<Project, String> {
        if !path.exists() {
            return Err(format!("Path does not exist: {}", path.display()));
        }
        if !path.is_dir() {
            return Err(format!("Path is not a directory: {}", path.display()));
        }

        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let tech_stack = self.stack_detector.detect(path);

        let mut project = Project::new(path.to_string_lossy().to_string(), name);
        project.tech_stack = tech_stack.languages;

        let id = project.id.clone();
        ProjectRepository::insert(&self.db()?, &project)?;
        self.projects.write().insert(id.clone(), project.clone());

        Ok(project)
    }

    pub fn create_project(&self, name: String) -> Result<Project, String> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err("Project name is required".to_string());
        }

        let project = Project::container(trimmed_name.to_string());
        let id = project.id.clone();
        ProjectRepository::insert(&self.db()?, &project)?;
        self.projects.write().insert(id, project.clone());

        Ok(project)
    }


    pub fn get_project(&self, id: &str) -> Option<Project> {
        if let Ok(db) = self.db() {
            if let Ok(Some(project)) = ProjectRepository::find_by_id(&db, id) {
                return Some(project);
            }
        }

        self.projects.read().get(id).cloned()
    }

    pub fn list_projects(&self) -> Vec<Project> {
        if let Ok(db) = self.db() {
            if let Ok(projects) = ProjectRepository::list(&db) {
                return projects;
            }
        }

        self.projects.read().values().cloned().collect()
    }

    pub fn close_project(&self, id: &str) -> bool {
        self.projects.write().remove(id).is_some()
    }

    pub fn refresh_stack(&self, id: &str) -> Result<Vec<String>, String> {
        let mut projects = self.projects.write();
        if let Some(project) = projects.get_mut(id) {
            let path = PathBuf::from(&project.path);
            if path.exists() {
                let stack = self.stack_detector.detect(&path);
                project.tech_stack = stack.languages;
                return Ok(project.tech_stack.clone());
            }
        }
        Err("Project not found".to_string())
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
