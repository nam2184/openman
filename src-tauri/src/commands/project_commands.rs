use crate::services::project_service::ProjectService;
use arachne_agents::Project;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn create_project(
    name: String,
    project_service: State<'_, Arc<ProjectService>>,
) -> Result<Project, String> {
    project_service.create_project(name)
}

#[tauri::command]
pub async fn open_project(
    path: String,
    project_service: State<'_, Arc<ProjectService>>,
) -> Result<Project, String> {
    let path = Path::new(&path);
    project_service.open_project(path)
}

#[tauri::command]
pub async fn get_project(
    id: String,
    project_service: State<'_, Arc<ProjectService>>,
) -> Result<Option<Project>, String> {
    Ok(project_service.get_project(&id))
}

#[tauri::command]
pub async fn list_projects(
    project_service: State<'_, Arc<ProjectService>>,
) -> Result<Vec<Project>, String> {
    Ok(project_service.list_projects())
}

#[tauri::command]
pub async fn close_project(
    id: String,
    project_service: State<'_, Arc<ProjectService>>,
) -> Result<bool, String> {
    Ok(project_service.close_project(&id))
}

#[tauri::command]
pub async fn refresh_project_stack(
    id: String,
    project_service: State<'_, Arc<ProjectService>>,
) -> Result<Vec<String>, String> {
    project_service.refresh_stack(&id)
}
