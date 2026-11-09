use openbot_core::error::{Error, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// Handle to the local-first SQLite database.
///
/// All access goes through a single `Mutex<Connection>`; SQLite handles
/// durability and crash recovery for us (WAL journal, synchronous=NORMAL).
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Storage(format!("create dir: {e}")))?;
        }
        let conn = Connection::open(path).map_err(|e| Error::Storage(format!("open: {e}")))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| Error::Storage(format!("journal_mode: {e}")))?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|e| Error::Storage(format!("synchronous: {e}")))?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| Error::Storage(format!("foreign_keys: {e}")))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| Error::Storage(format!("open: {e}")))?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| Error::Storage(format!("foreign_keys: {e}")))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| Error::Storage(format!("lock: {e}")))?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| Error::Storage(format!("migrate: {e}")))?;
        Ok(())
    }

    pub(crate) fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("storage lock poisoned")
    }
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    avatar TEXT NOT NULL DEFAULT '🤖',
    avatar_color TEXT NOT NULL DEFAULT '#7aa2f7',
    instructions TEXT NOT NULL DEFAULT '',
    provider TEXT NOT NULL DEFAULT 'simulated',
    model TEXT NOT NULL DEFAULT '',
    temperature REAL NOT NULL DEFAULT 0.7,
    max_tool_iterations INTEGER NOT NULL DEFAULT 8,
    compaction_messages INTEGER NOT NULL DEFAULT 60,
    busy_mode TEXT NOT NULL DEFAULT 'queue',
    phase TEXT NOT NULL DEFAULT 'idle',
    workspace_root TEXT NOT NULL DEFAULT '',
    capabilities TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    sender TEXT NOT NULL,
    text TEXT NOT NULL DEFAULT '',
    reply_to TEXT,
    attachments TEXT NOT NULL DEFAULT '[]',
    reactions TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    turn_id TEXT
);
CREATE INDEX IF NOT EXISTS idx_messages_agent ON messages(agent_id, created_at);

CREATE TABLE IF NOT EXISTS queue_items (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    sender TEXT NOT NULL,
    text TEXT NOT NULL,
    reply_to TEXT,
    attachments TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'queued',
    position INTEGER,
    turn_id TEXT,
    error TEXT,
    held INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_queue_agent ON queue_items(agent_id, status, position);

CREATE TABLE IF NOT EXISTS channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    task_owner TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS channel_members (
    channel_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    PRIMARY KEY (channel_id, agent_id)
);

CREATE TABLE IF NOT EXISTS channel_messages (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL,
    sender TEXT NOT NULL,
    text TEXT NOT NULL DEFAULT '',
    delegation_of TEXT,
    reply_to TEXT,
    attachments TEXT NOT NULL DEFAULT '[]',
    reactions TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_channel_messages ON channel_messages(channel_id, created_at);

CREATE TABLE IF NOT EXISTS turns (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'running',
    prompt TEXT NOT NULL DEFAULT '',
    started_at TEXT NOT NULL,
    ended_at TEXT,
    error TEXT
);
CREATE INDEX IF NOT EXISTS idx_turns_agent ON turns(agent_id, started_at);

CREATE TABLE IF NOT EXISTS routines (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    interval_minutes INTEGER NOT NULL DEFAULT 60,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_run_at TEXT,
    next_run_at TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS quick_actions (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    prompt TEXT NOT NULL,
    icon TEXT NOT NULL DEFAULT '⚡',
    color TEXT NOT NULL DEFAULT '#7aa2f7',
    "order" INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    instructions TEXT NOT NULL DEFAULT '',
    avatar TEXT NOT NULL DEFAULT '🤖',
    avatar_color TEXT NOT NULL DEFAULT '#7aa2f7',
    provider TEXT NOT NULL DEFAULT 'simulated',
    model TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]',
    builtin INTEGER NOT NULL DEFAULT 0
);
"#;
