use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::database::connection::Database;
use crate::{AgentSession, Message, MessageRole, Project, ProviderConfig, SessionGroup};

pub struct ProjectRepository;

impl ProjectRepository {
    pub fn insert(db: &Database, project: &Project) -> Result<(), String> {
        let tech_stack_json =
            serde_json::to_string(&project.tech_stack).map_err(|e| e.to_string())?;

        db.connection()
            .execute(
                "INSERT OR IGNORE INTO projects (id, path, name, tech_stack, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    project.id,
                    project.path,
                    project.name,
                    tech_stack_json,
                    project.created_at.to_rfc3339()
                ],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn find_by_id(db: &Database, id: &str) -> Result<Option<Project>, String> {
        let mut stmt = db
            .connection()
            .prepare("SELECT id, path, name, tech_stack, created_at FROM projects WHERE id = ?1")
            .map_err(|e| e.to_string())?;

        Ok(stmt
            .query_row(params![id], |row| {
                let tech_stack_json: String = row.get(3)?;
                let created_at: String = row.get(4)?;

                Ok(Project {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    tech_stack: serde_json::from_str(&tech_stack_json).unwrap_or_default(),
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .ok())
    }

    pub fn list(db: &Database) -> Result<Vec<Project>, String> {
        let mut stmt = db
            .connection()
            .prepare("SELECT id, path, name, tech_stack, created_at FROM projects")
            .map_err(|e| e.to_string())?;

        let projects = stmt
            .query_map([], |row| {
                let tech_stack_json: String = row.get(3)?;
                let created_at: String = row.get(4)?;

                Ok(Project {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    tech_stack: serde_json::from_str(&tech_stack_json).unwrap_or_default(),
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|row| row.ok())
            .collect();

        Ok(projects)
    }

    pub fn delete(db: &Database, id: &str) -> Result<(), String> {
        db.connection()
            .execute("DELETE FROM projects WHERE id = ?1", params![id])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

pub struct SessionRepository;

impl SessionRepository {
    pub fn insert(db: &Database, session: &AgentSession) -> Result<(), String> {
        db.connection()
            .execute(
                "INSERT INTO agent_sessions (id, project_id, directory, provider, model, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    session.id,
                    session.project_id,
                    session.directory,
                    session.provider,
                    session.model,
                    session.created_at.to_rfc3339()
                ],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn list(db: &Database) -> Result<Vec<AgentSession>, String> {
        let mut stmt = db
            .connection()
            .prepare(
                "
                SELECT s.id, s.project_id, s.directory, s.provider, s.model, s.created_at,
                       (SELECT group_id FROM session_group_sessions WHERE session_id = s.id LIMIT 1) AS group_id
                FROM agent_sessions s
                ",
            )
            .map_err(|e| e.to_string())?;

        let sessions = stmt
            .query_map([], |row| session_from_row(row))
            .map_err(|e| e.to_string())?
            .filter_map(|row| row.ok())
            .collect();

        Ok(sessions)
    }

    pub fn find_by_id(db: &Database, id: &str) -> Result<Option<AgentSession>, String> {
        let mut stmt = db
            .connection()
            .prepare(
                "
                SELECT s.id, s.project_id, s.directory, s.provider, s.model, s.created_at,
                       (SELECT group_id FROM session_group_sessions WHERE session_id = s.id LIMIT 1) AS group_id
                FROM agent_sessions s
                WHERE s.id = ?1
                ",
            )
            .map_err(|e| e.to_string())?;

        Ok(stmt
            .query_row(params![id], |row| session_from_row(row))
            .ok())
    }

    pub fn find_by_project(db: &Database, project_id: &str) -> Result<Vec<AgentSession>, String> {
        let mut stmt = db
            .connection()
            .prepare(
                "
                SELECT s.id, s.project_id, s.directory, s.provider, s.model, s.created_at,
                       (SELECT group_id FROM session_group_sessions WHERE session_id = s.id LIMIT 1) AS group_id
                FROM agent_sessions s
                WHERE s.project_id = ?1
                ",
            )
            .map_err(|e| e.to_string())?;

        let sessions = stmt
            .query_map(params![project_id], |row| session_from_row(row))
            .map_err(|e| e.to_string())?
            .filter_map(|row| row.ok())
            .collect();

        Ok(sessions)
    }

    pub fn update_provider(
        db: &Database,
        id: &str,
        provider: &str,
        model: &str,
    ) -> Result<(), String> {
        db.connection()
            .execute(
                "UPDATE agent_sessions SET provider = ?1, model = ?2 WHERE id = ?3",
                params![provider, model, id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn delete(db: &Database, id: &str) -> Result<(), String> {
        db.connection()
            .execute(
                "DELETE FROM session_group_sessions WHERE session_id = ?1",
                params![id],
            )
            .map_err(|e| e.to_string())?;

        db.connection()
            .execute("DELETE FROM agent_sessions WHERE id = ?1", params![id])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

pub struct SessionGroupRepository;

impl SessionGroupRepository {
    pub fn insert(db: &Database, group: &SessionGroup) -> Result<(), String> {
        db.connection()
            .execute(
                "INSERT INTO session_groups (id, name, created_at) VALUES (?1, ?2, ?3)",
                params![group.id, group.name, group.created_at.to_rfc3339()],
            )
            .map_err(|e| e.to_string())?;

        for session_id in &group.session_ids {
            Self::add_session(db, &group.id, session_id)?;
        }

        Ok(())
    }

    pub fn list(db: &Database) -> Result<Vec<SessionGroup>, String> {
        let mut stmt = db
            .connection()
            .prepare("SELECT id, name, created_at FROM session_groups")
            .map_err(|e| e.to_string())?;

        let groups = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let created_at: String = row.get(2)?;
                Ok(SessionGroup {
                    session_ids: Self::session_ids(db, &id).unwrap_or_default(),
                    name: row.get(1)?,
                    id,
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|row| row.ok())
            .collect();

        Ok(groups)
    }

    pub fn rename(db: &Database, id: &str, name: Option<String>) -> Result<(), String> {
        db.connection()
            .execute(
                "UPDATE session_groups SET name = ?1 WHERE id = ?2",
                params![name, id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn add_session(db: &Database, group_id: &str, session_id: &str) -> Result<(), String> {
        db.connection()
            .execute(
                "DELETE FROM session_group_sessions WHERE session_id = ?1",
                params![session_id],
            )
            .map_err(|e| e.to_string())?;

        db.connection()
            .execute(
                "INSERT OR IGNORE INTO session_group_sessions (group_id, session_id) VALUES (?1, ?2)",
                params![group_id, session_id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn remove_session(db: &Database, session_id: &str) -> Result<(), String> {
        db.connection()
            .execute(
                "DELETE FROM session_group_sessions WHERE session_id = ?1",
                params![session_id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn delete(db: &Database, id: &str) -> Result<(), String> {
        db.connection()
            .execute(
                "DELETE FROM session_group_sessions WHERE group_id = ?1",
                params![id],
            )
            .map_err(|e| e.to_string())?;

        db.connection()
            .execute("DELETE FROM session_groups WHERE id = ?1", params![id])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn session_ids(db: &Database, group_id: &str) -> Result<Vec<String>, String> {
        let mut stmt = db
            .connection()
            .prepare("SELECT session_id FROM session_group_sessions WHERE group_id = ?1")
            .map_err(|e| e.to_string())?;

        let session_ids = stmt
            .query_map(params![group_id], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|row| row.ok())
            .collect();

        Ok(session_ids)
    }
}

pub struct MessageRepository;

impl MessageRepository {
    pub fn insert(db: &Database, message: &Message) -> Result<(), String> {
        db.connection()
            .execute(
                "INSERT INTO messages (id, session_id, role, content, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    message.id,
                    message.session_id,
                    role_to_str(&message.role),
                    message.content,
                    message.timestamp.to_rfc3339()
                ],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn find_by_session(db: &Database, session_id: &str) -> Result<Vec<Message>, String> {
        let mut stmt = db
            .connection()
            .prepare("SELECT id, session_id, role, content, timestamp FROM messages WHERE session_id = ?1 ORDER BY timestamp ASC")
            .map_err(|e| e.to_string())?;

        let messages = stmt
            .query_map(params![session_id], |row| {
                let role: String = row.get(2)?;
                let timestamp: String = row.get(4)?;

                Ok(Message {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: role_from_str(&role),
                    content: row.get(3)?,
                    timestamp: DateTime::parse_from_rfc3339(&timestamp)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|row| row.ok())
            .collect();

        Ok(messages)
    }

    pub fn delete_by_session(db: &Database, session_id: &str) -> Result<(), String> {
        db.connection()
            .execute(
                "DELETE FROM messages WHERE session_id = ?1",
                params![session_id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

fn session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentSession> {
    let created_at: String = row.get(5)?;
    Ok(AgentSession {
        id: row.get(0)?,
        project_id: row.get(1)?,
        directory: row.get(2)?,
        provider: row.get(3)?,
        model: row.get(4)?,
        group_id: row.get(6)?,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

fn role_to_str(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::System => "system",
    }
}

fn role_from_str(role: &str) -> MessageRole {
    match role {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        _ => MessageRole::System,
    }
}

pub struct ProviderConfigRepository;

impl ProviderConfigRepository {
    pub fn upsert(db: &Database, config: &ProviderConfig) -> Result<(), String> {
        db.connection()
            .execute(
                "INSERT OR REPLACE INTO provider_configs (name, model, api_key, base_url, enabled) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    config.name,
                    config.model,
                    config.api_key,
                    config.base_url,
                    config.enabled as i32
                ],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn find_by_name(db: &Database, name: &str) -> Result<Option<ProviderConfig>, String> {
        let mut stmt = db
            .connection()
            .prepare("SELECT name, model, api_key, base_url, enabled FROM provider_configs WHERE name = ?1")
            .map_err(|e| e.to_string())?;

        let result = stmt
            .query_row(params![name], |row| {
                Ok(ProviderConfig {
                    name: row.get(0)?,
                    model: row.get(1)?,
                    api_key: row.get(2)?,
                    base_url: row.get(3)?,
                    enabled: row.get::<_, i32>(4)? != 0,
                })
            })
            .ok();

        Ok(result)
    }

    pub fn list(db: &Database) -> Result<Vec<ProviderConfig>, String> {
        let mut stmt = db
            .connection()
            .prepare("SELECT name, model, api_key, base_url, enabled FROM provider_configs")
            .map_err(|e| e.to_string())?;

        let configs = stmt
            .query_map([], |row| {
                Ok(ProviderConfig {
                    name: row.get(0)?,
                    model: row.get(1)?,
                    api_key: row.get(2)?,
                    base_url: row.get(3)?,
                    enabled: row.get::<_, i32>(4)? != 0,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|row| row.ok())
            .collect();

        Ok(configs)
    }

    pub fn delete(db: &Database, name: &str) -> Result<(), String> {
        db.connection()
            .execute(
                "DELETE FROM provider_configs WHERE name = ?1",
                params![name],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
