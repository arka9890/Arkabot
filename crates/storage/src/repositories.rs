//! Repositories over the SQLite database.

use crate::db::Db;
use chrono::{DateTime, Utc};
use openbot_core::error::{Error, Result};
use openbot_core::ids::*;
use openbot_core::models::*;
use rusqlite::{params, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;

fn now() -> DateTime<Utc> {
    Utc::now()
}

fn to_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".to_string())
}

fn from_json<T: DeserializeOwned + Default>(s: &str) -> T {
    serde_json::from_str(s).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AgentRepository<'a> {
    db: &'a Db,
}

impl<'a> AgentRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    fn row_to_agent(row: &rusqlite::Row<'_>) -> rusqlite::Result<Agent> {
        let capabilities: String = row.get(14)?;
        Ok(Agent {
            id: AgentId::from_str(row.get::<_, String>(0)?.as_str()).unwrap_or_default(),
            name: row.get(1)?,
            description: row.get(2)?,
            avatar: row.get(3)?,
            avatar_color: row.get(4)?,
            instructions: row.get(5)?,
            provider: row
                .get::<_, String>(6)?
                .parse::<ProviderKind>()
                .unwrap_or_default(),
            model: row.get(7)?,
            temperature: row.get(8)?,
            max_tool_iterations: row.get::<_, i64>(9)? as u32,
            compaction_messages: row.get::<_, i64>(10)? as u32,
            busy_mode: row.get::<_, String>(11)?.parse().unwrap_or_default(),
            phase: row
                .get::<_, String>(12)?
                .parse::<AgentPhase>()
                .unwrap_or_default(),
            workspace_root: row.get(13)?,
            capabilities: from_json(&capabilities),
            created_at: row.get(15)?,
            updated_at: row.get(16)?,
        })
    }

    const COLS: &'static str = "id, name, description, avatar, avatar_color, instructions,
        provider, model, temperature, max_tool_iterations, compaction_messages, busy_mode,
        phase, workspace_root, capabilities, created_at, updated_at";

    pub fn create(&self, agent: &Agent) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO agents (id, name, description, avatar, avatar_color, instructions,
                provider, model, temperature, max_tool_iterations, compaction_messages, busy_mode,
                phase, workspace_root, capabilities, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![
                agent.id.as_str(),
                agent.name,
                agent.description,
                agent.avatar,
                agent.avatar_color,
                agent.instructions,
                agent.provider.as_str(),
                agent.model,
                agent.temperature,
                agent.max_tool_iterations as i64,
                agent.compaction_messages as i64,
                agent.busy_mode.as_str(),
                agent.phase.as_str(),
                agent.workspace_root,
                to_json(&agent.capabilities),
                agent.created_at,
                agent.updated_at,
            ],
        )
        .map_err(|e| Error::Storage(format!("agent insert: {e}")))?;
        Ok(())
    }

    pub fn update(&self, agent: &Agent) -> Result<()> {
        let conn = self.db.conn();
        let count = conn
            .execute(
                "UPDATE agents SET name=?2, description=?3, avatar=?4, avatar_color=?5,
                instructions=?6, provider=?7, model=?8, temperature=?9, max_tool_iterations=?10,
                compaction_messages=?11, busy_mode=?12, phase=?13, workspace_root=?14,
                capabilities=?15, updated_at=?16 WHERE id=?1",
                params![
                    agent.id.as_str(),
                    agent.name,
                    agent.description,
                    agent.avatar,
                    agent.avatar_color,
                    agent.instructions,
                    agent.provider.as_str(),
                    agent.model,
                    agent.temperature,
                    agent.max_tool_iterations as i64,
                    agent.compaction_messages as i64,
                    agent.busy_mode.as_str(),
                    agent.phase.as_str(),
                    agent.workspace_root,
                    to_json(&agent.capabilities),
                    agent.updated_at,
                ],
            )
            .map_err(|e| Error::Storage(format!("agent update: {e}")))?;
        if count == 0 {
            return Err(Error::NotFound(format!("agent {}", agent.id)));
        }
        Ok(())
    }

    pub fn set_phase(&self, id: &AgentId, phase: AgentPhase) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE agents SET phase=?2, updated_at=?3 WHERE id=?1",
            params![id.as_str(), phase.as_str(), now()],
        )
        .map_err(|e| Error::Storage(format!("set phase: {e}")))?;
        Ok(())
    }

    pub fn get(&self, id: &AgentId) -> Result<Option<Agent>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(&format!("SELECT {} FROM agents WHERE id=?1", Self::COLS))
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let row = stmt
            .query_row(params![id.as_str()], |r| Self::row_to_agent(r))
            .optional()
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        Ok(row)
    }

    pub fn list(&self) -> Result<Vec<Agent>> {
        let conn = self.db.conn();
        let sql = format!("SELECT {} FROM agents ORDER BY created_at ASC", Self::COLS);
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map([], |r| Self::row_to_agent(r))
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))
    }

    pub fn delete(&self, id: &AgentId) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM agents WHERE id=?1", params![id.as_str()])
            .map_err(|e| Error::Storage(format!("delete: {e}")))?;
        Ok(())
    }

    pub fn count(&self) -> Result<u64> {
        let conn = self.db.conn();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM agents", [], |r| r.get(0))
            .map_err(|e| Error::Storage(format!("count: {e}")))?;
        Ok(n as u64)
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MessageRepository<'a> {
    db: &'a Db,
}

impl<'a> MessageRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
        Ok(Message {
            id: MessageId::from_str(row.get::<_, String>(0)?.as_str()).unwrap_or_default(),
            agent_id: AgentId::from_str(row.get::<_, String>(1)?.as_str()).unwrap_or_default(),
            sender: from_json(&row.get::<_, String>(2)?),
            text: row.get(3)?,
            reply_to: row
                .get::<_, Option<String>>(4)?
                .map(|s| MessageId::from_str(&s).unwrap_or_default()),
            attachments: from_json(&row.get::<_, String>(5)?),
            reactions: from_json(&row.get::<_, String>(6)?),
            created_at: row.get(7)?,
            turn_id: row
                .get::<_, Option<String>>(8)?
                .map(|s| TurnId::from_str(&s).unwrap_or_default()),
        })
    }

    pub fn add(&self, message: &Message) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO messages (id, agent_id, sender, text, reply_to, attachments, reactions,
                created_at, turn_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                message.id.as_str(),
                message.agent_id.as_str(),
                to_json(&message.sender),
                message.text,
                message.reply_to.as_ref().map(|m| m.as_str()),
                to_json(&message.attachments),
                to_json(&message.reactions),
                message.created_at,
                message.turn_id.as_ref().map(|m| m.as_str()),
            ],
        )
        .map_err(|e| Error::Storage(format!("message insert: {e}")))?;
        Ok(())
    }

    pub fn get(&self, id: &MessageId) -> Result<Option<Message>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, sender, text, reply_to, attachments, reactions, created_at, turn_id
                 FROM messages WHERE id=?1",
            )
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let row = stmt
            .query_row(params![id.as_str()], |r| Self::row_to_message(r))
            .optional()
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        Ok(row)
    }

    pub fn list_for_agent(&self, agent_id: &AgentId, limit: usize) -> Result<Vec<Message>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, sender, text, reply_to, attachments, reactions, created_at, turn_id
                 FROM messages WHERE agent_id=?1 ORDER BY created_at ASC LIMIT ?2",
            )
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map(params![agent_id.as_str(), limit as i64], |r| {
                Self::row_to_message(r)
            })
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))
    }

    pub fn count_for_agent(&self, agent_id: &AgentId) -> Result<u64> {
        let conn = self.db.conn();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE agent_id=?1",
                params![agent_id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| Error::Storage(format!("count: {e}")))?;
        Ok(n as u64)
    }

    pub fn delete_for_agent(&self, agent_id: &AgentId) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM messages WHERE agent_id=?1", params![agent_id.as_str()])
            .map_err(|e| Error::Storage(format!("delete: {e}")))?;
        Ok(())
    }

    pub fn add_reaction(&self, message_id: &MessageId, emoji: &str, reactor: &str) -> Result<Vec<Reaction>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare("SELECT reactions FROM messages WHERE id=?1")
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let existing: Option<String> = stmt
            .query_row(params![message_id.as_str()], |r| r.get(0))
            .optional()
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        let mut list: Vec<Reaction> = existing
            .map(|s| from_json(&s))
            .unwrap_or_default();
        if let Some(pos) = list.iter().position(|r| r.emoji == emoji && r.reactor == reactor) {
            list.remove(pos);
        } else {
            list.push(Reaction {
                emoji: emoji.to_string(),
                reactor: reactor.to_string(),
            });
        }
        conn.execute(
            "UPDATE messages SET reactions=?2 WHERE id=?1",
            params![message_id.as_str(), to_json(&list)],
        )
        .map_err(|e| Error::Storage(format!("reaction update: {e}")))?;
        Ok(list)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let conn = self.db.conn();
        let pattern = format!("%{}%", query);
        let mut stmt = conn
            .prepare(
                "SELECT m.id, m.agent_id, m.text, m.created_at, a.name
                 FROM messages m JOIN agents a ON a.id = m.agent_id
                 WHERE m.text LIKE ?1 ORDER BY m.created_at DESC LIMIT ?2",
            )
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map(params![pattern, limit as i64], |r| {
                Ok(SearchResult::Message {
                    id: MessageId::from_str(r.get::<_, String>(0)?.as_str()).unwrap_or_default(),
                    agent_id: AgentId::from_str(r.get::<_, String>(1)?.as_str()).unwrap_or_default(),
                    text: r.get(2)?,
                    created_at: r.get(3)?,
                })
            })
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))
    }
}

// ---------------------------------------------------------------------------
// Queue
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct QueueRepository<'a> {
    db: &'a Db,
}

impl<'a> QueueRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<QueueItem> {
        Ok(QueueItem {
            id: QueueId::from_str(row.get::<_, String>(0)?.as_str()).unwrap_or_default(),
            agent_id: AgentId::from_str(row.get::<_, String>(1)?.as_str()).unwrap_or_default(),
            sender: from_json(&row.get::<_, String>(2)?),
            text: row.get(3)?,
            reply_to: row
                .get::<_, Option<String>>(4)?
                .map(|s| MessageId::from_str(&s).unwrap_or_default()),
            attachments: from_json(&row.get::<_, String>(5)?),
            status: row
                .get::<_, String>(6)?
                .parse::<QueueStatus>()
                .unwrap_or(QueueStatus::Queued),
            position: row.get::<_, Option<i64>>(7)?.map(|v| v as u32),
            turn_id: row
                .get::<_, Option<String>>(8)?
                .map(|s| TurnId::from_str(&s).unwrap_or_default()),
            error: row.get(9)?,
            held: row.get::<_, i64>(10)? != 0,
            created_at: row.get(11)?,
        })
    }

    pub fn insert(&self, item: &QueueItem) -> Result<()> {
        let conn = self.db.conn();
        let next_pos: Option<i64> = conn
            .query_row(
                "SELECT MAX(position) FROM queue_items WHERE agent_id=?1 AND status='queued'",
                params![item.agent_id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| Error::Storage(format!("pos: {e}")))?;
        let position = Some(next_pos.unwrap_or(0) + 1);
        conn.execute(
            "INSERT INTO queue_items (id, agent_id, sender, text, reply_to, attachments, status,
                position, turn_id, error, held, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                item.id.as_str(),
                item.agent_id.as_str(),
                to_json(&item.sender),
                item.text,
                item.reply_to.as_ref().map(|m| m.as_str()),
                to_json(&item.attachments),
                item.status.as_str(),
                position,
                item.turn_id.as_ref().map(|m| m.as_str()),
                item.error,
                item.held as i64,
                item.created_at,
            ],
        )
        .map_err(|e| Error::Storage(format!("queue insert: {e}")))?;
        Ok(())
    }

    pub fn get(&self, id: &QueueId) -> Result<Option<QueueItem>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, sender, text, reply_to, attachments, status, position,
                 turn_id, error, held, created_at FROM queue_items WHERE id=?1",
            )
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let row = stmt
            .query_row(params![id.as_str()], |r| Self::row_to_item(r))
            .optional()
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        Ok(row)
    }

    pub fn list_for_agent(&self, agent_id: &AgentId) -> Result<Vec<QueueItem>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, sender, text, reply_to, attachments, status, position,
                 turn_id, error, held, created_at FROM queue_items
                 WHERE agent_id=?1 ORDER BY position ASC, created_at ASC",
            )
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map(params![agent_id.as_str()], |r| Self::row_to_item(r))
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))
    }

    pub fn next_queued(&self, agent_id: &AgentId) -> Result<Option<QueueItem>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, sender, text, reply_to, attachments, status, position,
                 turn_id, error, held, created_at FROM queue_items
                 WHERE agent_id=?1 AND status='queued' AND held=0
                 ORDER BY position ASC LIMIT 1",
            )
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let row = stmt
            .query_row(params![agent_id.as_str()], |r| Self::row_to_item(r))
            .optional()
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        Ok(row)
    }

    pub fn update_status(&self, id: &QueueId, status: QueueStatus, position: Option<u32>, error: Option<&str>, turn_id: Option<&TurnId>) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE queue_items SET status=?2, position=?3, error=?4, turn_id=?5 WHERE id=?1",
            params![
                id.as_str(),
                status.as_str(),
                position.map(|p| p as i64),
                error,
                turn_id.map(|t| t.as_str()),
            ],
        )
        .map_err(|e| Error::Storage(format!("queue update: {e}")))?;
        Ok(())
    }

    pub fn set_held(&self, id: &QueueId, held: bool) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE queue_items SET held=?2 WHERE id=?1",
            params![id.as_str(), held as i64],
        )
        .map_err(|e| Error::Storage(format!("held: {e}")))?;
        Ok(())
    }

    pub fn count_queued(&self, agent_id: &AgentId) -> Result<u64> {
        let conn = self.db.conn();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM queue_items WHERE agent_id=?1 AND status='queued'",
                params![agent_id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| Error::Storage(format!("count: {e}")))?;
        Ok(n as u64)
    }
}

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ChannelRepository<'a> {
    db: &'a Db,
}

impl<'a> ChannelRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn create(&self, channel: &Channel) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO channels (id, name, description, task_owner, status, created_at)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                channel.id.as_str(),
                channel.name,
                channel.description,
                channel.task_owner.as_str(),
                channel.status.as_str(),
                channel.created_at,
            ],
        )
        .map_err(|e| Error::Storage(format!("channel insert: {e}")))?;
        for member in &channel.members {
            conn.execute(
                "INSERT OR IGNORE INTO channel_members (channel_id, agent_id) VALUES (?1,?2)",
                params![channel.id.as_str(), member.as_str()],
            )
            .map_err(|e| Error::Storage(format!("member insert: {e}")))?;
        }
        Ok(())
    }

    pub fn get(&self, id: &ChannelId) -> Result<Option<Channel>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare("SELECT id, name, description, task_owner, status, created_at FROM channels WHERE id=?1")
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let channel = stmt
            .query_row(params![id.as_str()], |r| {
                Ok(Channel {
                    id: ChannelId::from_str(r.get::<_, String>(0)?.as_str()).unwrap_or_default(),
                    name: r.get(1)?,
                    description: r.get(2)?,
                    task_owner: AgentId::from_str(r.get::<_, String>(3)?.as_str()).unwrap_or_default(),
                    status: r.get::<_, String>(4)?.parse().unwrap_or(ChannelStatus::Active),
                    members: Vec::new(),
                    created_at: r.get(5)?,
                })
            })
            .optional()
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        let Some(mut channel) = channel else {
            return Ok(None);
        };
        channel.members = self.members(&channel.id)?;
        Ok(Some(channel))
    }

    pub fn list(&self) -> Result<Vec<Channel>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare("SELECT id, name, description, task_owner, status, created_at FROM channels ORDER BY created_at ASC")
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Channel {
                    id: ChannelId::from_str(r.get::<_, String>(0)?.as_str()).unwrap_or_default(),
                    name: r.get(1)?,
                    description: r.get(2)?,
                    task_owner: AgentId::from_str(r.get::<_, String>(3)?.as_str()).unwrap_or_default(),
                    status: r.get::<_, String>(4)?.parse().unwrap_or(ChannelStatus::Active),
                    members: Vec::new(),
                    created_at: r.get(5)?,
                })
            })
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        let mut channels: Vec<Channel> = rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))?;
        for channel in &mut channels {
            channel.members = self.members(&channel.id)?;
        }
        Ok(channels)
    }

    pub fn members(&self, id: &ChannelId) -> Result<Vec<AgentId>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare("SELECT agent_id FROM channel_members WHERE channel_id=?1")
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map(params![id.as_str()], |r| {
                Ok(AgentId::from_str(r.get::<_, String>(0)?.as_str()).unwrap_or_default())
            })
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))
    }

    pub fn add_member(&self, id: &ChannelId, agent_id: &AgentId) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT OR IGNORE INTO channel_members (channel_id, agent_id) VALUES (?1,?2)",
            params![id.as_str(), agent_id.as_str()],
        )
        .map_err(|e| Error::Storage(format!("member insert: {e}")))?;
        Ok(())
    }

    pub fn remove_member(&self, id: &ChannelId, agent_id: &AgentId) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "DELETE FROM channel_members WHERE channel_id=?1 AND agent_id=?2",
            params![id.as_str(), agent_id.as_str()],
        )
        .map_err(|e| Error::Storage(format!("member delete: {e}")))?;
        Ok(())
    }

    pub fn set_status(&self, id: &ChannelId, status: ChannelStatus) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE channels SET status=?2 WHERE id=?1",
            params![id.as_str(), status.as_str()],
        )
        .map_err(|e| Error::Storage(format!("channel status: {e}")))?;
        Ok(())
    }

    pub fn set_owner(&self, id: &ChannelId, owner: &AgentId) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE channels SET task_owner=?2 WHERE id=?1",
            params![id.as_str(), owner.as_str()],
        )
        .map_err(|e| Error::Storage(format!("owner: {e}")))?;
        Ok(())
    }

    pub fn delete(&self, id: &ChannelId) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM channels WHERE id=?1", params![id.as_str()])
            .map_err(|e| Error::Storage(format!("channel delete: {e}")))?;
        conn.execute("DELETE FROM channel_members WHERE channel_id=?1", params![id.as_str()])
            .map_err(|e| Error::Storage(format!("members delete: {e}")))?;
        conn.execute("DELETE FROM channel_messages WHERE channel_id=?1", params![id.as_str()])
            .map_err(|e| Error::Storage(format!("messages delete: {e}")))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Channel messages
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ChannelMessageRepository<'a> {
    db: &'a Db,
}

impl<'a> ChannelMessageRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn add(&self, msg: &ChannelMessage) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO channel_messages (id, channel_id, sender, text, delegation_of, reply_to,
                attachments, reactions, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                msg.id.as_str(),
                msg.channel_id.as_str(),
                to_json(&msg.sender),
                msg.text,
                msg.delegation_of.as_ref().map(|m| m.as_str()),
                msg.reply_to.as_ref().map(|m| m.as_str()),
                to_json(&msg.attachments),
                to_json(&msg.reactions),
                msg.created_at,
            ],
        )
        .map_err(|e| Error::Storage(format!("channel message insert: {e}")))?;
        Ok(())
    }

    pub fn list(&self, channel_id: &ChannelId, limit: usize) -> Result<Vec<ChannelMessage>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, channel_id, sender, text, delegation_of, reply_to, attachments, reactions, created_at
                 FROM channel_messages WHERE channel_id=?1 ORDER BY created_at ASC LIMIT ?2",
            )
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map(params![channel_id.as_str(), limit as i64], |r| {
                Ok(ChannelMessage {
                    id: MessageId::from_str(r.get::<_, String>(0)?.as_str()).unwrap_or_default(),
                    channel_id: ChannelId::from_str(r.get::<_, String>(1)?.as_str()).unwrap_or_default(),
                    sender: from_json(&r.get::<_, String>(2)?),
                    text: r.get(3)?,
                    delegation_of: r
                        .get::<_, Option<String>>(4)?
                        .map(|s| MessageId::from_str(&s).unwrap_or_default()),
                    reply_to: r
                        .get::<_, Option<String>>(5)?
                        .map(|s| MessageId::from_str(&s).unwrap_or_default()),
                    attachments: from_json(&r.get::<_, String>(6)?),
                    reactions: from_json(&r.get::<_, String>(7)?),
                    created_at: r.get(8)?,
                })
            })
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))
    }

    pub fn add_reaction(&self, message_id: &MessageId, emoji: &str, reactor: &str) -> Result<Vec<Reaction>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare("SELECT reactions FROM channel_messages WHERE id=?1")
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let existing: Option<String> = stmt
            .query_row(params![message_id.as_str()], |r| r.get(0))
            .optional()
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        let mut list: Vec<Reaction> = existing.map(|s| from_json(&s)).unwrap_or_default();
        if let Some(pos) = list.iter().position(|r| r.emoji == emoji && r.reactor == reactor) {
            list.remove(pos);
        } else {
            list.push(Reaction {
                emoji: emoji.to_string(),
                reactor: reactor.to_string(),
            });
        }
        conn.execute(
            "UPDATE channel_messages SET reactions=?2 WHERE id=?1",
            params![message_id.as_str(), to_json(&list)],
        )
        .map_err(|e| Error::Storage(format!("reaction update: {e}")))?;
        Ok(list)
    }
}

// ---------------------------------------------------------------------------
// Turns
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TurnRepository<'a> {
    db: &'a Db,
}

impl<'a> TurnRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn create(&self, turn: &Turn) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO turns (id, agent_id, status, prompt, started_at, ended_at, error)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                turn.id.as_str(),
                turn.agent_id.as_str(),
                turn.status.as_str(),
                turn.prompt,
                turn.started_at,
                turn.ended_at,
                turn.error,
            ],
        )
        .map_err(|e| Error::Storage(format!("turn insert: {e}")))?;
        Ok(())
    }

    pub fn finish(&self, id: &TurnId, status: TurnStatus, error: Option<&str>) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE turns SET status=?2, ended_at=?3, error=?4 WHERE id=?1",
            params![id.as_str(), status.as_str(), Utc::now(), error],
        )
        .map_err(|e| Error::Storage(format!("turn finish: {e}")))?;
        Ok(())
    }

    pub fn get(&self, id: &TurnId) -> Result<Option<Turn>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare("SELECT id, agent_id, status, prompt, started_at, ended_at, error FROM turns WHERE id=?1")
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let turn = stmt
            .query_row(params![id.as_str()], |r| {
                Ok(Turn {
                    id: TurnId::from_str(r.get::<_, String>(0)?.as_str()).unwrap_or_default(),
                    agent_id: AgentId::from_str(r.get::<_, String>(1)?.as_str()).unwrap_or_default(),
                    status: r.get::<_, String>(2)?.parse().unwrap_or(TurnStatus::Running),
                    prompt: r.get(3)?,
                    started_at: r.get(4)?,
                    ended_at: r.get(5)?,
                    error: r.get(6)?,
                })
            })
            .optional()
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        Ok(turn)
    }
}

// ---------------------------------------------------------------------------
// Routines
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct RoutineRepository<'a> {
    db: &'a Db,
}

impl<'a> RoutineRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn create(&self, routine: &Routine) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO routines (id, agent_id, name, prompt, interval_minutes, enabled,
                last_run_at, next_run_at, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                routine.id.as_str(),
                routine.agent_id.as_str(),
                routine.name,
                routine.prompt,
                routine.interval_minutes as i64,
                routine.enabled as i64,
                routine.last_run_at,
                routine.next_run_at,
                routine.created_at,
            ],
        )
        .map_err(|e| Error::Storage(format!("routine insert: {e}")))?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<Routine>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, name, prompt, interval_minutes, enabled, last_run_at, next_run_at, created_at
                 FROM routines ORDER BY created_at ASC",
            )
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Routine {
                    id: RoutineId::from_str(r.get::<_, String>(0)?.as_str()).unwrap_or_default(),
                    agent_id: AgentId::from_str(r.get::<_, String>(1)?.as_str()).unwrap_or_default(),
                    name: r.get(2)?,
                    prompt: r.get(3)?,
                    interval_minutes: r.get::<_, i64>(4)? as u64,
                    enabled: r.get::<_, i64>(5)? != 0,
                    last_run_at: r.get(6)?,
                    next_run_at: r.get(7)?,
                    created_at: r.get(8)?,
                })
            })
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))
    }

    pub fn delete(&self, id: &RoutineId) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM routines WHERE id=?1", params![id.as_str()])
            .map_err(|e| Error::Storage(format!("routine delete: {e}")))?;
        Ok(())
    }

    pub fn mark_run(&self, id: &RoutineId, when: DateTime<Utc>, interval_minutes: u64) -> Result<()> {
        let conn = self.db.conn();
        let next = when + chrono::Duration::minutes(interval_minutes as i64);
        conn.execute(
            "UPDATE routines SET last_run_at=?2, next_run_at=?3 WHERE id=?1",
            params![id.as_str(), when, next],
        )
        .map_err(|e| Error::Storage(format!("routine run: {e}")))?;
        Ok(())
    }

    pub fn next_due(&self) -> Result<Option<Routine>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, name, prompt, interval_minutes, enabled, last_run_at, next_run_at, created_at
                 FROM routines WHERE enabled=1 AND (next_run_at IS NULL OR next_run_at <= ?1) LIMIT 1",
            )
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let row = stmt
            .query_row(params![Utc::now()], |r| {
                Ok(Routine {
                    id: RoutineId::from_str(r.get::<_, String>(0)?.as_str()).unwrap_or_default(),
                    agent_id: AgentId::from_str(r.get::<_, String>(1)?.as_str()).unwrap_or_default(),
                    name: r.get(2)?,
                    prompt: r.get(3)?,
                    interval_minutes: r.get::<_, i64>(4)? as u64,
                    enabled: r.get::<_, i64>(5)? != 0,
                    last_run_at: r.get(6)?,
                    next_run_at: r.get(7)?,
                    created_at: r.get(8)?,
                })
            })
            .optional()
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        Ok(row)
    }
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SettingsRepository<'a> {
    db: &'a Db,
}

impl<'a> SettingsRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )
        .map_err(|e| Error::Storage(format!("settings set: {e}")))?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let conn = self.db.conn();
        let value = conn
            .query_row("SELECT value FROM settings WHERE key=?1", params![key], |r| r.get(0))
            .optional()
            .map_err(|e| Error::Storage(format!("settings get: {e}")))?;
        Ok(value)
    }

    pub fn get_json<T: DeserializeOwned + Default>(&self, key: &str) -> Result<T> {
        match self.get(key)? {
            Some(v) => Ok(from_json(&v)),
            None => Ok(T::default()),
        }
    }

    pub fn set_json<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.set(key, &to_json(value))
    }

    pub fn all(&self) -> Result<std::collections::BTreeMap<String, String>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare("SELECT key, value FROM settings")
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        rows.collect::<std::result::Result<_, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))
    }
}

// ---------------------------------------------------------------------------
// Quick actions
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct QuickActionRepository<'a> {
    db: &'a Db,
}

impl<'a> QuickActionRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn create(&self, action: &QuickAction) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO quick_actions (id, label, prompt, icon, color, \"order\")
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![action.id, action.label, action.prompt, action.icon, action.color, action.order],
        )
        .map_err(|e| Error::Storage(format!("quick action insert: {e}")))?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<QuickAction>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare("SELECT id, label, prompt, icon, color, \"order\" FROM quick_actions ORDER BY \"order\" ASC")
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(QuickAction {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    prompt: r.get(2)?,
                    icon: r.get(3)?,
                    color: r.get(4)?,
                    order: r.get(5)?,
                })
            })
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM quick_actions WHERE id=?1", params![id])
            .map_err(|e| Error::Storage(format!("quick action delete: {e}")))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Templates (agent marketplace)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TemplateRepository<'a> {
    db: &'a Db,
}

impl<'a> TemplateRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn upsert(&self, t: &AgentTemplate) -> Result<()> {
        let conn = self.db.conn();
        // Deterministic id derived from the name so re-seeding/updating the same
        // template updates the existing row instead of inserting a duplicate.
        let slug: String = t
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let id = format!("tpl_{slug}");
        conn.execute(
            "INSERT INTO templates (id, name, description, instructions, avatar, avatar_color,
                provider, model, tags, builtin) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name, description=excluded.description,
                instructions=excluded.instructions, avatar=excluded.avatar,
                avatar_color=excluded.avatar_color, provider=excluded.provider,
                model=excluded.model, tags=excluded.tags, builtin=excluded.builtin",
            params![
                id,
                t.name,
                t.description,
                t.instructions,
                t.avatar,
                t.avatar_color,
                t.provider.as_str(),
                t.model,
                to_json(&t.tags),
                t.builtin as i64,
            ],
        )
        .map_err(|e| Error::Storage(format!("template insert: {e}")))?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<AgentTemplate>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(
                "SELECT name, description, instructions, avatar, avatar_color, provider, model, tags, builtin
                 FROM templates ORDER BY builtin DESC, name ASC",
            )
            .map_err(|e| Error::Storage(format!("prepare: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(AgentTemplate {
                    name: r.get(0)?,
                    description: r.get(1)?,
                    instructions: r.get(2)?,
                    avatar: r.get(3)?,
                    avatar_color: r.get(4)?,
                    provider: r.get::<_, String>(5)?.parse().unwrap_or_default(),
                    model: r.get(6)?,
                    tags: from_json(&r.get::<_, String>(7)?),
                    builtin: r.get::<_, i64>(8)? != 0,
                })
            })
            .map_err(|e| Error::Storage(format!("query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Storage(format!("row: {e}")))
    }
}
