use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod commands;
mod db;
mod error;
mod services;

use services::agent_runtime::{create_agent_runtime, AgentRuntime};
use services::memory_service::MemoryService;
use services::conversation_service::create_conversation_service;
use services::conversation_service::ConversationService;
use services::project_service::ProjectService;
use services::session_service::SessionService;
use services::settings_service::SettingsService;
use services::stack_detector::StackDetector;
use services::tree_sitter::TreeSitterService;
use services::watcher_service::WatcherService;
use services::context_indexer::ContextIndexer;

pub struct AppState {
    pub project_service: Arc<ProjectService>,
    pub agent_runtime: Arc<AgentRuntime>,
    pub session_service: Arc<SessionService>,
    pub conversation_service: Arc<ConversationService>,
    pub settings_service: Arc<SettingsService>,
    pub memory_service: Arc<MemoryService>,
    pub stack_detector: Arc<StackDetector>,
    pub tree_sitter: Arc<TreeSitterService>,
    pub watcher_service: Arc<WatcherService>,
    pub context_indexer: Arc<ContextIndexer>,
}

fn setup_logging() {
    let default_filter = default_log_filter();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn default_log_filter() -> &'static str {
    if cfg!(feature = "dev-logs") || cfg!(debug_assertions) {
        "openman=debug,tauri=info"
    } else if cfg!(feature = "prod-logs") {
        "openman=info"
    } else {
        "warn"
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    setup_logging();

    let tree_sitter = TreeSitterService::new();
    let stack_detector = StackDetector::new();
    let memory_service = MemoryService::new();
    let watcher_service = WatcherService::new();
    let context_indexer = ContextIndexer::new(Arc::clone(&tree_sitter));
    let agent_runtime = create_agent_runtime(Arc::clone(&tree_sitter));

    let app_dirs = directories::ProjectDirs::from("ai", "openman", "openman");
    let app_data_dir = app_dirs
        .as_ref()
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap().join("data"));
    let app_config_dir = app_dirs
        .as_ref()
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap().join("config"));

    let db_path = app_data_dir.join("openman.sqlite");
    let project_service = ProjectService::new(db_path.clone(), Arc::clone(&stack_detector));
    let session_service = SessionService::new(db_path);
    let conversation_service = create_conversation_service(app_data_dir.join("conversations"));

    let settings_service = SettingsService::new(app_config_dir);

    if let Err(e) = settings_service.load() {
        tracing::warn!("Failed to load settings: {}", e);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            project_service: Arc::clone(&project_service),
            agent_runtime: Arc::clone(&agent_runtime),
            session_service: Arc::clone(&session_service),
            conversation_service: Arc::clone(&conversation_service),
            settings_service: Arc::clone(&settings_service),
            memory_service: Arc::clone(&memory_service),
            stack_detector: Arc::clone(&stack_detector),
            tree_sitter: Arc::clone(&tree_sitter),
            watcher_service: Arc::clone(&watcher_service),
            context_indexer: Arc::clone(&context_indexer),
        })
        .manage(conversation_service)
        .manage(project_service)
        .manage(agent_runtime)
        .manage(session_service)
        .manage(settings_service)
        .invoke_handler(tauri::generate_handler![
            commands::project_commands::create_project,
            commands::project_commands::open_project,
            commands::project_commands::get_project,
            commands::project_commands::list_projects,
            commands::project_commands::close_project,
            commands::project_commands::refresh_project_stack,
            commands::file_commands::read_file,
            commands::file_commands::write_file,
            commands::file_commands::list_directory,
            commands::file_commands::search_files,
            commands::file_commands::get_file_tree,
            commands::agent_commands::create_agent_session,
            commands::agent_commands::send_message,
            commands::agent_commands::update_agent_context,
            commands::agent_commands::add_memory_fact,
            commands::agent_commands::parse_code_context,
            commands::session_commands::init_sessions,
            commands::session_commands::create_session,
            commands::session_commands::get_session,
            commands::session_commands::get_all_sessions,
            commands::session_commands::delete_session,
            commands::session_commands::create_session_group,
            commands::session_commands::get_all_session_groups,
            commands::session_commands::delete_session_group,
            commands::session_commands::rename_session_group,
            commands::session_commands::add_session_to_group,
            commands::session_commands::remove_session_from_group,
            commands::conversation_commands::append_message,
            commands::conversation_commands::get_messages,
            commands::conversation_commands::get_ai_conversation,
            commands::conversation_commands::get_ui_conversation,
            commands::conversation_commands::compact_conversation,
            commands::conversation_commands::delete_conversation,
            commands::settings_commands::get_settings,
            commands::settings_commands::update_provider,
            commands::settings_commands::set_active_provider,
            commands::settings_commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
