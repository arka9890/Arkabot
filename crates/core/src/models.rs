//! Domain models mirroring OpenBot's concepts.

use crate::ids::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type Timestamp = DateTime<Utc>;

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------

/// Agent phases (see OpenBot `AGENT_PHASES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPhase {
    Idle,
    Starting,
    Ready,
    Restarting,
    Blocked,
    Stopped,
}

impl AgentPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentPhase::Idle => "idle",
            AgentPhase::Starting => "starting",
            AgentPhase::Ready => "ready",
            AgentPhase::Restarting => "restarting",
            AgentPhase::Blocked => "blocked",
            AgentPhase::Stopped => "stopped",
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, AgentPhase::Starting | AgentPhase::Ready | AgentPhase::Restarting)
    }
}

impl Default for AgentPhase {
    fn default() -> Self {
        AgentPhase::Idle
    }
}

macro_rules! impl_from_str {
    ($type:ident, $default:expr, { $($variant:ident => $value:literal),* $(,)? }) => {
        impl std::str::FromStr for $type {
            type Err = String;
            fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                match s {
                    $($value => Ok($type::$variant),)*
                    _ => Ok($default),
                }
            }
        }
    };
}

impl_from_str!(AgentPhase, AgentPhase::Idle, {
    Idle => "idle", Starting => "starting", Ready => "ready",
    Restarting => "restarting", Blocked => "blocked", Stopped => "stopped",
});

impl_from_str!(CapabilityState, CapabilityState::Unavailable, {
    Ready => "ready", SetupRequired => "setup-required", Unavailable => "unavailable",
});

impl_from_str!(BusyMessageMode, BusyMessageMode::Queue, {
    Queue => "queue", Steer => "steer",
});

impl_from_str!(ProviderKind, ProviderKind::Simulated, {
    OpenaiCompatible => "openai-compatible", Ollama => "ollama",
    LmStudio => "lmstudio", Simulated => "simulated",
});

impl_from_str!(QueueStatus, QueueStatus::Queued, {
    Queued => "queued", Starting => "starting", Running => "running",
    Completed => "completed", Failed => "failed", Interrupted => "interrupted",
    Cancelled => "cancelled",
});

impl_from_str!(ChannelStatus, ChannelStatus::Active, {
    Active => "active", Stopped => "stopped", Archived => "archived",
});

impl_from_str!(TurnStatus, TurnStatus::Running, {
    Running => "running", Completed => "completed", Failed => "failed", Cancelled => "cancelled",
});

impl_from_str!(ThemeMode, ThemeMode::System, {
    System => "system", Dark => "dark", Light => "light",
});

impl_from_str!(Capability, Capability::Chat, {
    Chat => "chat", Files => "files", Browser => "browser",
    TaskQueue => "task_queue", Notifications => "notifications",
});

/// Capability availability states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Ready,
    SetupRequired,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Chat,
    Files,
    Browser,
    TaskQueue,
    Notifications,
}

/// How a message sent to a busy agent is handled (OpenBot `BUSY_MESSAGE_MODES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusyMessageMode {
    Queue,
    Steer,
}

impl Default for BusyMessageMode {
    fn default() -> Self {
        BusyMessageMode::Queue
    }
}

impl BusyMessageMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            BusyMessageMode::Queue => "queue",
            BusyMessageMode::Steer => "steer",
        }
    }
}

/// The LLM provider kind used by an agent or as the app default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::upper_case_acronyms)]
pub enum ProviderKind {
    /// Any OpenAI-compatible endpoint (OpenAI, Azure, Groq, Together, ...).
    #[serde(rename = "openai-compatible")]
    OpenaiCompatible,
    /// Local Ollama server (usually http://10.0.2.2:11434 on Android emulator).
    Ollama,
    /// Local LM Studio server.
    // Kotlin sends "lmstudio" (SettingsScreen FilterChip) and `as_str()`
    // returns "lmstudio" — snake_case would produce "lm_studio" and break
    // `serde_json::from_str` in the FFI. Alias keeps old stored data readable.
    #[serde(rename = "lmstudio", alias = "lm_studio")]
    LmStudio,
    /// Built-in simulated provider for demos without a model server.
    Simulated,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::OpenaiCompatible => "openai-compatible",
            ProviderKind::Ollama => "ollama",
            ProviderKind::LmStudio => "lmstudio",
            ProviderKind::Simulated => "simulated",
        }
    }
}

impl Default for ProviderKind {
    fn default() -> Self {
        ProviderKind::Simulated
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub description: String,
    /// Emoji or short text avatar (OpenBot agents have avatars).
    pub avatar: String,
    /// Accent color for the avatar, hex `#rrggbb`.
    pub avatar_color: String,
    /// The agent's instructions, stored as `AGENTS.md` in its workspace.
    pub instructions: String,
    pub provider: ProviderKind,
    pub model: String,
    pub temperature: f32,
    pub max_tool_iterations: u32,
    /// Compact the conversation when it grows past this many messages.
    pub compaction_messages: u32,
    pub busy_mode: BusyMessageMode,
    pub phase: AgentPhase,
    /// Relative path of the agent's workspace inside the data directory.
    pub workspace_root: String,
    pub capabilities: Vec<Capability>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Agent {
    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }
}

/// Provider readiness, used by `check_provider` in settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub state: String, // connecting | checking | available | sign_in_required | not_installed | outdated | error | not_started
    pub message: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Sender {
    User,
    /// Struct variant (not a newtype): serde internally-tagged enums cannot
    /// serialize newtype variants containing a plain string, which silently
    /// turned every agent reply into `null` → read back as `User`.
    Agent { agent_id: AgentId },
    Routine {
        routine_id: RoutineId,
        run_id: String,
        name: String,
    },
}

impl Default for Sender {
    fn default() -> Self {
        Sender::User
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    /// Relative path inside the data directory.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: String,
    pub reactor: String, // "user" or agent id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub agent_id: AgentId,
    pub sender: Sender,
    pub text: String,
    pub reply_to: Option<MessageId>,
    pub attachments: Vec<Attachment>,
    pub reactions: Vec<Reaction>,
    pub created_at: Timestamp,
    pub turn_id: Option<TurnId>,
}

impl Message {
    pub fn from_user(agent_id: AgentId, text: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            agent_id,
            sender: Sender::User,
            text: text.into(),
            reply_to: None,
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at: Utc::now(),
            turn_id: None,
        }
    }

    pub fn from_agent(agent_id: AgentId, text: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            agent_id: agent_id.clone(),
            sender: Sender::Agent { agent_id },
            text: text.into(),
            reply_to: None,
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at: Utc::now(),
            turn_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Task queue
// ---------------------------------------------------------------------------

/// Queue delivery statuses (OpenBot `QUEUE_DELIVERY_STATUSES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    Queued,
    Starting,
    Running,
    Completed,
    Failed,
    Interrupted,
    Cancelled,
}

impl QueueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueueStatus::Queued => "queued",
            QueueStatus::Starting => "starting",
            QueueStatus::Running => "running",
            QueueStatus::Completed => "completed",
            QueueStatus::Failed => "failed",
            QueueStatus::Interrupted => "interrupted",
            QueueStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: QueueId,
    /// The agent that receives this queued message.
    pub agent_id: AgentId,
    pub sender: Sender,
    pub text: String,
    pub reply_to: Option<MessageId>,
    pub attachments: Vec<Attachment>,
    pub status: QueueStatus,
    /// FIFO position within the agent's queue; null when not queued.
    pub position: Option<u32>,
    pub turn_id: Option<TurnId>,
    pub error: Option<String>,
    /// Another editor holds this item; it keeps its place and is not delivered.
    pub held: bool,
    pub created_at: Timestamp,
}

// ---------------------------------------------------------------------------
// Channels (shared channel chats)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelStatus {
    Active,
    Stopped,
    Archived,
}

impl ChannelStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChannelStatus::Active => "active",
            ChannelStatus::Stopped => "stopped",
            ChannelStatus::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: ChannelId,
    pub name: String,
    pub description: String,
    /// The single task owner; only this agent runs tasks delegated to it.
    pub task_owner: AgentId,
    pub status: ChannelStatus,
    pub members: Vec<AgentId>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub id: MessageId,
    pub channel_id: ChannelId,
    pub sender: Sender,
    pub text: String,
    /// When an agent replies to a channel message on behalf of its owner.
    pub delegation_of: Option<MessageId>,
    pub reply_to: Option<MessageId>,
    pub attachments: Vec<Attachment>,
    pub reactions: Vec<Reaction>,
    pub created_at: Timestamp,
}

// ---------------------------------------------------------------------------
// Turns (a single agent run)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TurnStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TurnStatus::Running => "running",
            TurnStatus::Completed => "completed",
            TurnStatus::Failed => "failed",
            TurnStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub id: TurnId,
    pub agent_id: AgentId,
    pub status: TurnStatus,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub error: Option<String>,
    pub prompt: String,
}

// ---------------------------------------------------------------------------
// Routines (scheduled tasks)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Routine {
    pub id: RoutineId,
    pub agent_id: AgentId,
    pub name: String,
    pub prompt: String,
    pub interval_minutes: u64,
    pub enabled: bool,
    pub last_run_at: Option<Timestamp>,
    pub next_run_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

// ---------------------------------------------------------------------------
// Quick actions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickAction {
    pub id: String,
    pub label: String,
    pub prompt: String,
    pub icon: String,
    pub color: String,
    pub order: i32,
}

// ---------------------------------------------------------------------------
// Workspace files
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String, // relative to the agent workspace root
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeNode {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub children: Vec<FileTreeNode>,
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    System,
    Dark,
    Light,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::System
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub provider: ProviderKind,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub theme: ThemeMode,
    pub busy_mode: BusyMessageMode,
    /// Simulated provider mode: true lets agents answer without a model server.
    pub demo_mode: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            provider: ProviderKind::Simulated,
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            temperature: 0.7,
            theme: ThemeMode::System,
            busy_mode: BusyMessageMode::Queue,
            // First launch runs in demo mode, no server required.
            demo_mode: true,
        }
    }
}

impl AppSettings {
    pub fn is_demo(&self) -> bool {
        self.demo_mode || self.provider == ProviderKind::Simulated
    }
}

// ---------------------------------------------------------------------------
// Templates (agent marketplace)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTemplate {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub avatar: String,
    pub avatar_color: String,
    pub provider: ProviderKind,
    pub model: String,
    pub tags: Vec<String>,
    pub builtin: bool,
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SearchResult {
    Agent { id: AgentId, name: String, description: String, avatar: String },
    Message { id: MessageId, agent_id: AgentId, text: String, created_at: Timestamp },
    Channel { id: ChannelId, name: String, description: String },
    QuickAction { id: String, label: String, prompt: String },
    Routine { id: RoutineId, name: String, agent_id: AgentId },
}

// ---------------------------------------------------------------------------
// App events (pushed to the UI / FFI)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    AgentCreated,
    AgentUpdated,
    AgentDeleted,
    AgentPhaseChanged,
    TurnStarted,
    TurnCompleted,
    TurnFailed,
    MessageAdded,
    MessageUpdated,
    QueueItemAdded,
    QueueItemUpdated,
    ChannelCreated,
    ChannelUpdated,
    ChannelMessageAdded,
    RoutineUpdated,
    SettingsChanged,
    ProviderStatusChanged,
    Error,
}

/// A pushed event. `payload` is a JSON string with the details of the event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEvent {
    pub kind: EventKind,
    pub agent_id: Option<String>,
    pub channel_id: Option<String>,
    pub payload: String,
}

impl AppEvent {
    pub fn new(kind: EventKind, payload: impl Into<String>) -> Self {
        Self {
            kind,
            agent_id: None,
            channel_id: None,
            payload: payload.into(),
        }
    }

    pub fn for_agent(kind: EventKind, agent_id: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            kind,
            agent_id: Some(agent_id.into()),
            channel_id: None,
            payload: payload.into(),
        }
    }

    pub fn for_channel(kind: EventKind, channel_id: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            kind,
            agent_id: None,
            channel_id: Some(channel_id.into()),
            payload: payload.into(),
        }
    }
}
