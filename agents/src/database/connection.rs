use rusqlite::{Connection, Result};
use std::path::PathBuf;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(path: PathBuf) -> Result<Self> {
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    pub fn init(&self) -> Result<(), String> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                name TEXT NOT NULL,
                tech_stack TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS agent_sessions (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                directory TEXT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id)
            );

            CREATE TABLE IF NOT EXISTS session_groups (
                id TEXT PRIMARY KEY,
                name TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS session_group_sessions (
                group_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                PRIMARY KEY (group_id, session_id),
                FOREIGN KEY (group_id) REFERENCES session_groups(id),
                FOREIGN KEY (session_id) REFERENCES agent_sessions(id)
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES agent_sessions(id)
            );

            CREATE TABLE IF NOT EXISTS memory (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                fact TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id)
            );

            CREATE TABLE IF NOT EXISTS provider_configs (
                name TEXT PRIMARY KEY,
                model TEXT NOT NULL,
                api_key TEXT,
                base_url TEXT,
                enabled INTEGER NOT NULL DEFAULT 1
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_project ON agent_sessions(project_id);
            CREATE INDEX IF NOT EXISTS idx_session_groups_session ON session_group_sessions(session_id);
            CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
            CREATE INDEX IF NOT EXISTS idx_memory_project ON memory(project_id);
            "
        ).map_err(|e| e.to_string())?;

        let _ = self
            .conn
            .execute("ALTER TABLE session_groups ADD COLUMN name TEXT", []);
        let _ = self.conn.execute(
            "ALTER TABLE agent_sessions ADD COLUMN directory TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE agent_sessions ADD COLUMN provider TEXT NOT NULL DEFAULT 'anthropic'",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE agent_sessions ADD COLUMN model TEXT NOT NULL DEFAULT 'claude-3-5-sonnet-20241022'",
            [],
        );

        Ok(())
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}
