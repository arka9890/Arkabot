//! OpenBot-RS application core.
//!
//! Wires storage, the agent engine, messaging, workspace files, and routines
//! into the single `OpenBot` facade used by the Android app (through FFI) and
//! the local HTTP/WS server.

use agent_engine::{AgentEngine, tools as engine_tools};
use browser::BrowserControl;
pub mod portability;
pub mod skills;
use skills::SkillLibrary;use openbot_core::api::{ChannelMessageView, MessageSender, TurnRunner, TurnTrigger};
use openbot_core::error::{Error, Result};
use openbot_core::ids::*;
use openbot_core::models::*;
use openbot_core::AppEvent;
use llm::{build_client, LlmClient, ProviderConfig};
use messaging::MessagingService;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use storage::{
    AgentRepository, ChannelRepository, MessageRepository, QueueRepository,
    QuickActionRepository, SettingsRepository, TemplateRepository, Db,
};
use tokio::sync::broadcast;

/// Input for creating or updating an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInput {
    pub name: String,
    pub description: String,
    pub avatar: String,
    pub avatar_color: String,
    pub instructions: String,
    pub provider: ProviderKind,
    pub model: String,
    pub temperature: f32,
    pub max_tool_iterations: u32,
    pub compaction_messages: u32,
    pub busy_mode: BusyMessageMode,
    /// Named browser profile; empty means the shared default browser.
    pub browser_profile: String,
    pub capabilities: Vec<Capability>,
}

impl Default for AgentInput {
    fn default() -> Self {
        Self {
            name: "Agent Baru".into(),
            description: String::new(),
            avatar: "🤖".into(),
            avatar_color: "#7aa2f7".into(),
            instructions: String::new(),
            provider: ProviderKind::default(),
            model: String::new(),
            temperature: 0.7,
            max_tool_iterations: 8,
            compaction_messages: 60,
            busy_mode: BusyMessageMode::default(),
            browser_profile: String::new(),
            capabilities: vec![
                Capability::Chat,
                Capability::Files,
                Capability::TaskQueue,
                Capability::Notifications,
            ],
        }
    }
}

pub struct OpenBot {
    inner: Arc<OpenBotInner>,
}

struct OpenBotInner {
    db: Arc<Db>,
    data_dir: PathBuf,
    engine: Arc<AgentEngine>,
    messaging: Arc<MessagingService>,
    events: broadcast::Sender<AppEvent>,
    routine_scheduler_started: AtomicBool,
    /// Tokio handle used for background spawns (routines, etc.).
    handle: tokio::runtime::Handle,
    /// Keeps a privately-owned runtime alive when `OpenBot` is constructed
    /// outside any tokio runtime (the Android FFI case).
    _runtime: Option<tokio::runtime::Runtime>,
    /// Shared library of reusable `SKILL.md` workflows.
    skills: SkillLibrary,
}

impl OpenBot {
    /// Open (or create) a local-first OpenBot workspace in `data_dir`.
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Arc<Self>> {
        let data_dir = data_dir.into();
        std::fs::create_dir_all(&data_dir).map_err(|e| Error::Storage(format!("create data dir: {e}")))?;

        let db = Arc::new(Db::open(data_dir.join("openbot.db"))?);

        // Default settings (demo mode first-launch, like OpenBot's local-first default).
        let settings_repo = SettingsRepository::new(&db);
        let settings: AppSettings = settings_repo.get_json("app_settings")?;
        settings_repo.set_json("app_settings", &settings)?;

        // Seed defaults.
        Self::seed_defaults(&db, &settings)?;

        let (events, _) = broadcast::channel(512);

        // Reuse the current tokio runtime when one exists (e.g. the HTTP
        // server); otherwise own a private runtime so background spawns work
        // when called from a plain thread (the Android FFI entry points).
        let (handle, owned_runtime) = match tokio::runtime::Handle::try_current() {
            Ok(h) => (h, None),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("arkabot")
                    .build()
                    .map_err(|e| Error::Internal(format!("runtime: {e}")))?;
                let h = rt.handle().clone();
                (h, Some(rt))
            }
        };

        let engine = AgentEngine::new(
            db.clone(),
            data_dir.clone(),
            settings,
            events.clone(),
            handle.clone(),
        );
        engine.load_pause_state();

        let engine_dyn: Arc<dyn TurnRunner> = engine.clone();
        let trigger: Arc<dyn TurnTrigger> = engine.clone();
        let messaging = Arc::new(MessagingService::new(
            db.clone(),
            engine_dyn.clone(),
            trigger.clone(),
            events.clone(),
            handle.clone(),
        ));

        // Two-way wiring: engine tools use messaging; messaging wakes the engine.
        engine.set_messaging(messaging.clone());
        // Browser control is attached by the platform (Android WebView bridge or
        // desktop Chrome remote debugging) through `set_browser_control`.

        let skills = SkillLibrary::new(data_dir.clone());
        let inner = OpenBotInner {
            db,
            data_dir,
            engine,
            messaging,
            events,
            routine_scheduler_started: AtomicBool::new(false),
            handle,
            _runtime: owned_runtime,
            skills,
        };
        Ok(Arc::new(OpenBot {
            inner: Arc::new(inner),
        }))
    }

    fn seed_defaults(db: &Db, settings: &AppSettings) -> Result<()> {
        // First-launch quick actions.
        let actions = QuickActionRepository::new(db);
        if actions.list()?.is_empty() {
            for (icon, color, label, prompt) in [
                ("❓", "#7aa2f7", "Tanya", ""),
                ("📋", "#9ece6a", "Tugas", ""),
                ("🔍", "#e0af68", "Cari", ""),
                ("📝", "#f7768e", "Catat", ""),
            ] {
                actions.create(&QuickAction {
                    id: uuid::Uuid::new_v4().to_string(),
                    label: label.into(),
                    prompt: prompt.into(),
                    icon: icon.into(),
                    color: color.into(),
                    order: 0,
                })?;
            }
        }

        // Builtin agent templates (the marketplace).
        let templates = TemplateRepository::new(db);
        if templates.list()?.iter().all(|t| !t.builtin) {
            for (name, description, instructions, avatar, color, tags) in [
                (
                    "Asisten",
                    "Asisten umum untuk tugas sehari-hari.",
                    "Anda adalah asisten yang membantu dengan tugas sehari-hari. Berikan jawaban singkat dan jelas dalam bahasa Indonesia.",
                    "🤖",
                    "#7aa2f7",
                    vec!["assistant".to_string(), "general".to_string()],
                ),
                (
                    "Coder",
                    "Agent spesialis pemrograman.",
                    "Anda adalah coder yang membantu menulis, menjelaskan, dan memperbaiki kode. Selalu berikan kode dalam blok code dan jelaskan pendekatannya.",
                    "💻",
                    "#9ece6a",
                    vec!["coding".to_string(), "programming".to_string()],
                ),
                (
                    "Researcher",
                    "Agent riset untuk mencari dan merangkum informasi.",
                    "Anda adalah researcher. Rangkum informasi dengan akurat, sebutkan sumber apabila tersedia, dan bedakan fakta dengan opini.",
                    "🔍",
                    "#e0af68",
                    vec!["research".to_string(), "summarize".to_string()],
                ),
            ] {
                templates.upsert(&AgentTemplate {
                    name: name.into(),
                    description: description.into(),
                    instructions: instructions.into(),
                    avatar: avatar.into(),
                    avatar_color: color.into(),
                    provider: settings.provider.clone(),
                    model: settings.model.clone(),
                    tags,
                    builtin: true,
                })?;
            }
        }

        // First-launch demo agent.
        let agents = AgentRepository::new(db);
        if agents.count()? == 0 {
            let agent = Agent {
                id: AgentId::new(),
                name: "Agen Demo".into(),
                description: "Agent pertama Anda. Edit instruksinya di pengaturan.".to_string(),
                avatar: "🤖".into(),
                avatar_color: "#7aa2f7".into(),
                instructions: "Anda adalah agent OpenBot-RS. Bantu pengguna dengan ramah, jawab dalam bahasa Indonesia, dan gunakan tools saat dibutuhkan.".to_string(),
                provider: settings.provider.clone(),
                model: settings.model.clone(),
                temperature: settings.temperature,
                max_tool_iterations: 8,
                compaction_messages: 60,
                busy_mode: BusyMessageMode::default(),
                phase: AgentPhase::Idle,
                workspace_root: String::new(),
                browser_profile: String::new(),
                capabilities: vec![
                    Capability::Chat,
                    Capability::Files,
                    Capability::TaskQueue,
                    Capability::Notifications,
                ],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            let mut agent = agent;
            agent.workspace_root = format!("agents/{}/workspace", agent.id.as_str());
            agents.create(&agent)?;
        }
        Ok(())
    }

    // ------- accessors ----------------------------------------------------

    pub fn db(&self) -> &Arc<Db> {
        &self.inner.db
    }

    pub fn engine(&self) -> &Arc<AgentEngine> {
        &self.inner.engine
    }

    pub fn messaging(&self) -> &Arc<MessagingService> {
        &self.inner.messaging
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.inner.events.subscribe()
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.inner.data_dir
    }

    /// Attach a browser control implementation (CDP or WebView bridge).
    pub fn set_browser_control(&self, browser: Arc<dyn BrowserControl>) {
        self.inner.engine.set_browser(browser);
    }

    /// Register a named browser profile. An agent whose `browser_profile`
    /// names it drives that browser instead of the shared default.
    pub fn set_browser_profile(&self, name: &str, browser: Arc<dyn BrowserControl>) {
        self.inner.engine.set_browser_profile(name, browser);
    }

    /// Connect to the app's WebView bridge and register it as a browser.
    ///
    /// `name` empty registers the shared default; otherwise it registers a
    /// named profile that only the agents naming it can drive.
    pub async fn connect_browser(&self, name: &str, base_url: &str) -> Result<()> {
        let browser = browser::HttpBrowser::new(base_url);
        if name.trim().is_empty() {
            self.set_browser_control(Arc::new(browser));
        } else {
            self.set_browser_profile(name, Arc::new(browser));
        }
        Ok(())
    }

    /// Start the routine scheduler (polls due routines every 30s).
    pub fn start_routine_scheduler(&self) {
        if self.inner.routine_scheduler_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let messaging = self.inner.messaging.clone();
        self.inner.handle.spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                ticker.tick().await;
                let _ = messaging.run_due_routines().await;
            }
        });
    }

    // ------- agents --------------------------------------------------------

    pub fn create_agent(&self, input: AgentInput) -> Result<Agent> {
        let id = AgentId::new();
        let now = chrono::Utc::now();
        let agent = Agent {
            id: id.clone(),
            name: input.name,
            description: input.description,
            avatar: input.avatar,
            avatar_color: input.avatar_color,
            instructions: input.instructions,
            provider: input.provider,
            model: input.model,
            temperature: input.temperature,
            max_tool_iterations: input.max_tool_iterations,
            compaction_messages: input.compaction_messages,
            busy_mode: input.busy_mode,
            phase: AgentPhase::Idle,
            workspace_root: format!("agents/{}/workspace", id.as_str()),
            browser_profile: input.browser_profile,
            capabilities: input.capabilities,
            created_at: now,
            updated_at: now,
        };
        self.engine().workspace().ensure_root(&agent.id)?;
        // Write AGENTS.md with the instructions.
        if !agent.instructions.is_empty() {
            self.engine()
                .workspace()
                .write(&agent.id, "AGENTS.md", &agent.instructions)?;
        }
        AgentRepository::new(&self.inner.db).create(&agent)?;
        self.emit(
            EventKind::AgentCreated,
            Some(agent.id.as_str().to_string()),
            None,
            json!({ "agent_id": agent.id.as_str(), "name": agent.name.clone() }).to_string(),
        );
        Ok(agent)
    }

    pub fn update_agent(&self, id: &AgentId, input: AgentInput) -> Result<Agent> {
        let mut agent = self
            .agent(id)?
            .ok_or_else(|| Error::NotFound(format!("agent {id}")))?;
        agent.name = input.name;
        agent.description = input.description;
        agent.avatar = input.avatar;
        agent.avatar_color = input.avatar_color;
        agent.instructions = input.instructions;
        agent.provider = input.provider;
        agent.model = input.model;
        agent.temperature = input.temperature;
        agent.max_tool_iterations = input.max_tool_iterations;
        agent.compaction_messages = input.compaction_messages;
        agent.busy_mode = input.busy_mode;
        agent.browser_profile = input.browser_profile;
        agent.capabilities = input.capabilities;
        agent.updated_at = chrono::Utc::now();
        AgentRepository::new(&self.inner.db).update(&agent)?;
        // Sync AGENTS.md.
        self.engine()
            .workspace()
            .write(&agent.id, "AGENTS.md", &agent.instructions)?;
        self.emit(
            EventKind::AgentUpdated,
            Some(agent.id.as_str().to_string()),
            None,
            json!({ "agent_id": agent.id.as_str() }).to_string(),
        );
        Ok(agent)
    }

    pub fn agent(&self, id: &AgentId) -> Result<Option<Agent>> {
        AgentRepository::new(&self.inner.db).get(id)
    }

    pub fn list_agents(&self) -> Result<Vec<Agent>> {
        AgentRepository::new(&self.inner.db).list()
    }

    pub fn delete_agent(&self, id: &AgentId) -> Result<()> {
        AgentRepository::new(&self.inner.db).delete(id)?;
        MessageRepository::new(&self.inner.db).delete_for_agent(id)?;
        let _ = std::fs::remove_dir_all(self.engine().workspace().agent_root(id));
        self.emit(
            EventKind::AgentDeleted,
            Some(id.as_str().to_string()),
            None,
            json!({ "agent_id": id.as_str() }).to_string(),
        );
        Ok(())
    }

    pub fn restart_agent(&self, id: &AgentId) -> Result<()> {
        self.engine().agents().set_phase(id, AgentPhase::Restarting).ok();
        self.emit(
            EventKind::AgentPhaseChanged,
            Some(id.as_str().to_string()),
            None,
            json!({ "phase": "restarting" }).to_string(),
        );
        Ok(())
    }

    // ------- chat ----------------------------------------------------------

    pub fn send_message(
        &self,
        agent_id: &AgentId,
        text: String,
        attachments: Vec<Attachment>,
        reply_to: Option<MessageId>,
    ) -> Result<Message> {
        self.messaging()
            .send_user_message(agent_id, text, attachments, reply_to)
    }

    pub fn agent_messages(&self, agent_id: &AgentId, limit: usize) -> Result<Vec<Message>> {
        MessageRepository::new(&self.inner.db).list_for_agent(agent_id, limit)
    }

    pub fn message(&self, id: &MessageId) -> Result<Option<Message>> {
        MessageRepository::new(&self.inner.db).get(id)
    }

    pub fn react(&self, message_id: &MessageId, emoji: &str) -> Result<Vec<Reaction>> {
        let reactions = MessageRepository::new(&self.inner.db)
            .add_reaction(message_id, emoji, "user")?;
        self.emit(
            EventKind::MessageUpdated,
            None,
            None,
            json!({ "message_id": message_id.as_str(), "reactions": reactions }).to_string(),
        );
        Ok(reactions)
    }

    pub fn message_reactions(&self, message_id: &MessageId) -> Result<Vec<Reaction>> {
        Ok(MessageRepository::new(&self.inner.db)
            .get(message_id)?
            .map(|m| m.reactions)
            .unwrap_or_default())
    }

    // ------- queue ---------------------------------------------------------

    pub fn queue_list(&self, agent_id: &AgentId) -> Result<Vec<QueueItem>> {
        QueueRepository::new(&self.inner.db).list_for_agent(agent_id)
    }

    pub fn queue_pause(&self, agent_id: &AgentId) -> Result<()> {
        self.engine().queue_pause(agent_id)
    }

    pub fn queue_resume(&self, agent_id: &AgentId) -> Result<()> {
        self.engine().queue_resume(agent_id)
    }

    pub fn queue_cancel(&self, item_id: &QueueId) -> Result<()> {
        self.engine().queue_cancel(item_id)
    }

    // ------- channels ------------------------------------------------------

    pub fn create_channel(
        &self,
        name: String,
        description: String,
        task_owner: AgentId,
        members: Vec<AgentId>,
    ) -> Result<Channel> {
        self.messaging().create_channel(name, description, task_owner, members)
    }

    pub fn list_channels(&self) -> Result<Vec<Channel>> {
        ChannelRepository::new(&self.inner.db).list()
    }

    pub fn channel(&self, id: &ChannelId) -> Result<Option<Channel>> {
        ChannelRepository::new(&self.inner.db).get(id)
    }

    pub fn channel_messages(&self, channel_id: &ChannelId, limit: usize) -> Result<Vec<ChannelMessage>> {
        self.messaging().channel_messages(channel_id, limit)
    }

    pub fn send_channel_message(
        &self,
        channel_id: &ChannelId,
        text: String,
        delegation_of: Option<MessageId>,
    ) -> Result<ChannelMessageView> {
        self.messaging()
            .send_to_channel(channel_id, None, text, delegation_of)
    }

    pub fn channel_action(&self, channel_id: &ChannelId, action: &str, agent_id: Option<&AgentId>) -> Result<Channel> {
        self.messaging().channel_actions(channel_id, action, agent_id)
    }

    pub fn channel_add_reaction(&self, message_id: &MessageId, emoji: &str) -> Result<Vec<Reaction>> {
        self.messaging().channel_add_reaction(message_id, emoji, "user")
    }

    pub fn delete_channel(&self, channel_id: &ChannelId) -> Result<()> {
        self.messaging().delete_channel(channel_id)
    }

    // ------- workspace files ----------------------------------------------

    pub fn workspace_tree(&self, agent_id: &AgentId) -> Result<FileTreeNode> {
        self.engine().workspace().tree(agent_id, "")
    }

    pub fn workspace_list(&self, agent_id: &AgentId, dir: &str) -> Result<Vec<FileEntry>> {
        self.engine().workspace().list(agent_id, dir)
    }

    pub fn workspace_read(&self, agent_id: &AgentId, path: &str) -> Result<String> {
        self.engine().workspace().read(agent_id, path)
    }

    pub fn workspace_write(&self, agent_id: &AgentId, path: &str, content: &str) -> Result<FileEntry> {
        let entry = self.engine().workspace().write(agent_id, path, content)?;
        self.emit(
            EventKind::AgentUpdated,
            Some(agent_id.as_str().to_string()),
            None,
            json!({ "file": entry.path.clone() }).to_string(),
        );
        Ok(entry)
    }

    pub fn workspace_create_dir(&self, agent_id: &AgentId, path: &str) -> Result<()> {
        self.engine().workspace().create_dir(agent_id, path)
    }

    pub fn workspace_delete(&self, agent_id: &AgentId, path: &str) -> Result<()> {
        self.engine().workspace().delete(agent_id, path)
    }

    // ------- attachments ---------------------------------------------------

    pub fn save_attachment(&self, name: &str, mime: &str, bytes: &[u8]) -> Result<Attachment> {
        let id = uuid::Uuid::new_v4().to_string();
        // Only keep the final path segment of the filename to avoid traversal.
        let safe_name = Path::new(name)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty() && s != "." && s != "..")
            .unwrap_or_else(|| "attachment".to_string());
        let dir = self.engine().workspace().attachments_dir();
        let dir = dir.join(&id);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(&safe_name), bytes)?;
        Ok(Attachment {
            id: id.clone(),
            name: safe_name.clone(),
            mime: mime.to_string(),
            size: bytes.len() as u64,
            path: format!("attachments/{id}/{safe_name}"),
        })
    }

    pub fn attachment_bytes(&self, attachment_id: &str, name: &str) -> Result<Vec<u8>> {
        // Sanitize both segments: attachment ids are uuids, names are file names.
        let safe_id = Path::new(attachment_id)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let safe_name = Path::new(name)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if safe_id.is_empty() || safe_name.is_empty() {
            return Err(Error::Workspace("invalid attachment path".into()));
        }
        let dir = self.engine().workspace().attachments_dir();
        let path = dir.join(&safe_id).join(&safe_name);
        std::fs::read(&path)
            .map_err(|e| Error::Workspace(format!("read attachment: {e}")))
    }

    // ------- settings -----------------------------------------------------

    pub fn get_settings(&self) -> Result<AppSettings> {
        SettingsRepository::new(&self.inner.db).get_json("app_settings")
    }

    pub async fn update_settings(&self, settings: AppSettings) -> Result<ProviderStatus> {
        self.engine().update_settings(settings.clone())?;
        self.check_provider().await
    }

    pub async fn check_provider(&self) -> Result<ProviderStatus> {
        let settings = self.get_settings()?;
        let config = ProviderConfig {
            base_url: settings.base_url.clone(),
            api_key: settings.api_key.clone(),
            model: settings.model.clone(),
            temperature: settings.temperature,
            max_tokens: None,
            acp_command: settings.acp_command.clone(),
            acp_args: settings.acp_args.clone(),
            acp_cwd: settings.acp_cwd.clone(),
        };
        let client: Arc<dyn LlmClient> = Arc::from(build_client(&config));
        match client.check().await {
            Ok(status) => Ok(ProviderStatus {
                state: status.state,
                message: status.message,
                model: status.model,
                base_url: status.base_url,
            }),
            Err(e) => Ok(ProviderStatus {
                state: "error".into(),
                message: Some(e.to_string()),
                model: Some(settings.model),
                base_url: Some(settings.base_url),
            }),
        }
    }

    // ------- quick actions -------------------------------------------------

    pub fn list_quick_actions(&self) -> Result<Vec<QuickAction>> {
        QuickActionRepository::new(&self.inner.db).list()
    }

    pub fn add_quick_action(&self, label: String, prompt: String, icon: String, color: String) -> Result<QuickAction> {
        let action = QuickAction {
            id: uuid::Uuid::new_v4().to_string(),
            label,
            prompt,
            icon,
            color,
            order: 0,
        };
        QuickActionRepository::new(&self.inner.db).create(&action)?;
        Ok(action)
    }

    pub fn delete_quick_action(&self, id: &str) -> Result<()> {
        QuickActionRepository::new(&self.inner.db).delete(id)
    }

    // ------- routines ------------------------------------------------------

    pub fn create_routine(&self, agent_id: AgentId, name: String, prompt: String, interval_minutes: u64) -> Result<Routine> {
        let routine = Routine {
            id: RoutineId::new(),
            agent_id: agent_id.clone(),
            name,
            prompt,
            interval_minutes,
            enabled: true,
            last_run_at: None,
            next_run_at: Some(chrono::Utc::now() + chrono::Duration::minutes(interval_minutes as i64)),
            created_at: chrono::Utc::now(),
        };
        self.messaging().create_routine(&routine)?;
        self.emit(
            EventKind::RoutineUpdated,
            Some(agent_id.as_str().to_string()),
            None,
            json!({ "routine_id": routine.id.as_str(), "name": routine.name.clone() }).to_string(),
        );
        Ok(routine)
    }

    pub fn list_routines(&self) -> Result<Vec<Routine>> {
        self.messaging().list_routines()
    }

    pub fn delete_routine(&self, id: &RoutineId) -> Result<()> {
        self.messaging().delete_routine(id)
    }

    // ------- templates (marketplace) --------------------------------------

    pub fn list_templates(&self) -> Result<Vec<AgentTemplate>> {
        TemplateRepository::new(&self.inner.db).list()
    }

    pub fn add_template(&self, template: &AgentTemplate) -> Result<()> {
        TemplateRepository::new(&self.inner.db).upsert(template)
    }

    pub fn import_template_as_agent(&self, template: AgentTemplate) -> Result<Agent> {
        self.create_agent(AgentInput {
            name: template.name,
            description: template.description,
            avatar: template.avatar,
            avatar_color: template.avatar_color,
            instructions: template.instructions,
            provider: template.provider,
            model: template.model,
            ..Default::default()
        })
    }

    // ------- agent import / export -----------------------------------------

    /// Write an agent and its workspace into a zip file inside `target_dir`.
    ///
    /// Returns the path of the written archive.
    pub fn export_agent(&self, agent_id: &AgentId, target_dir: &str) -> Result<String> {
        let agent = self
            .agent(agent_id)?
            .ok_or_else(|| Error::NotFound(format!("agent {}", agent_id)))?;
        let target = PathBuf::from(target_dir);
        std::fs::create_dir_all(&target)?;
        let path = portability::archive_path(&target, &agent.name);
        let workspace = self.inner.data_dir.join("agents").join(agent_id.as_str()).join("workspace");
        portability::export_agent_to_zip(&agent, &workspace, &path)?;
        Ok(path.to_string_lossy().into_owned())
    }

    /// Rebuild an agent from a zip archive.
    ///
    /// The agent gets a new id and its own workspace; the archive's files are
    /// written into it.
    pub fn import_agent(&self, archive_path: &str) -> Result<Agent> {
        let imported = portability::read_agent_zip(&PathBuf::from(archive_path))?;
        let mut agent = imported.agent;
        let created = self.create_agent(AgentInput {
            name: agent.name.clone(),
            description: agent.description.clone(),
            avatar: agent.avatar.clone(),
            avatar_color: agent.avatar_color.clone(),
            instructions: agent.instructions.clone(),
            provider: agent.provider.clone(),
            model: agent.model.clone(),
            temperature: agent.temperature,
            max_tool_iterations: agent.max_tool_iterations,
            compaction_messages: agent.compaction_messages,
            busy_mode: agent.busy_mode.clone(),
            browser_profile: agent.browser_profile.clone(),
            capabilities: agent.capabilities.clone(),
        })?;
        for (rel, data) in imported.files {
            let text = String::from_utf8_lossy(&data).into_owned();
            self.workspace_write(&created.id, &rel, &text)?;
        }
        agent = created;
        Ok(agent)
    }

    // ------- ACP providers --------------------------------------------------

    /// The ACP agents the app can drive, with their default commands.
    pub fn acp_providers(&self) -> Vec<acp::AcpProvider> {
        acp::providers()
    }

    // ------- local skills ---------------------------------------------------

    /// Every skill in the shared library.
    pub fn list_skills(&self) -> Result<Vec<Skill>> {
        self.inner.skills.list()
    }

    /// The library skills, each marked with whether `agent_id` has it installed.
    pub fn list_skills_for_agent(&self, agent_id: &AgentId) -> Result<Vec<Skill>> {
        self.inner.skills.list_for_agent(agent_id)
    }

    /// The skills installed for one agent.
    pub fn agent_skills(&self, agent_id: &AgentId) -> Result<Vec<Skill>> {
        self.inner.skills.installed(agent_id)
    }

    /// The full text of one library skill, frontmatter included.
    pub fn read_skill(&self, id: &str) -> Result<String> {
        let skill = self.inner.skills.read(id)?;
        Ok(skills::SkillFile::render(
            &skills::SkillFrontmatter {
                name: skill.name,
                description: skill.description,
                version: skill.version,
            },
            &skill.body,
        ))
    }

    /// Create or revise a library skill.
    ///
    /// `expected_version` must be the version the caller read; a mismatch is
    /// rejected so a concurrent edit is not overwritten.
    pub fn save_skill(&self, id: &str, content: &str, expected_version: Option<&str>) -> Result<Skill> {
        self.inner.skills.save(id, content, expected_version)
    }

    /// Remove a skill from the library.
    pub fn delete_skill(&self, id: &str) -> Result<()> {
        self.inner.skills.delete(id)
    }

    /// Install a library skill into an agent's workspace.
    pub fn install_skill(&self, agent_id: &AgentId, skill_id: &str) -> Result<Skill> {
        self.inner.skills.install(agent_id, skill_id)
    }

    /// Remove an installed skill from an agent's workspace.
    pub fn uninstall_skill(&self, agent_id: &AgentId, skill_id: &str) -> Result<()> {
        self.inner.skills.uninstall(agent_id, skill_id)
    }

    // ------- search --------------------------------------------------------

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let mut results: Vec<SearchResult> = Vec::new();
        let q = query.to_lowercase();

        for agent in self.list_agents()? {
            if agent.name.to_lowercase().contains(&q) || agent.description.to_lowercase().contains(&q) {
                results.push(SearchResult::Agent {
                    id: agent.id.clone(),
                    name: agent.name,
                    description: agent.description,
                    avatar: agent.avatar,
                });
            }
        }
        results.extend(MessageRepository::new(&self.inner.db).search(query, limit)?);
        for channel in self.list_channels()? {
            if channel.name.to_lowercase().contains(&q) || channel.description.to_lowercase().contains(&q) {
                results.push(SearchResult::Channel {
                    id: channel.id,
                    name: channel.name,
                    description: channel.description,
                });
            }
        }
        for action in self.list_quick_actions()? {
            if action.label.to_lowercase().contains(&q) || action.prompt.to_lowercase().contains(&q) {
                results.push(SearchResult::QuickAction {
                    id: action.id,
                    label: action.label,
                    prompt: action.prompt,
                });
            }
        }
        for routine in self.list_routines()? {
            if routine.name.to_lowercase().contains(&q) {
                results.push(SearchResult::Routine {
                    id: routine.id,
                    name: routine.name,
                    agent_id: routine.agent_id,
                });
            }
        }
        Ok(results)
    }

    // ------- data export/import -------------------------------------------

    pub fn export_data(&self, target_dir: &str) -> Result<String> {
        let target = PathBuf::from(target_dir);
        std::fs::create_dir_all(&target)?;
        copy_dir(&self.inner.data_dir, &target)?;
        Ok(target.to_string_lossy().into_owned())
    }

    pub fn import_data(&self, source_dir: &str) -> Result<()> {
        let source = PathBuf::from(source_dir);
        // Never overwrite the live SQLite files while the connection is open —
        // that corrupts the database. Everything else (workspaces, attachments)
        // is restored.
        let skip_db = |name: &str| name == "openbot.db" || name.starts_with("openbot.db-");
        copy_dir_filtered(&source, &self.inner.data_dir, &skip_db)?;
        Ok(())
    }

    // ------- events ---------------------------------------------------------

    fn emit(&self, kind: EventKind, agent_id: Option<String>, channel_id: Option<String>, payload: String) {
        let _ = self.inner.events.send(AppEvent {
            kind,
            agent_id,
            channel_id,
            payload,
        });
    }
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> Result<()> {
    copy_dir_filtered(from, to, &|_| false)
}

fn copy_dir_filtered(
    from: &std::path::Path,
    to: &std::path::Path,
    skip: &dyn Fn(&str) -> bool,
) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from).map_err(|e| Error::Storage(format!("read_dir {from:?}: {e}")))? {
        let entry = entry?;
        let name = entry.file_name();
        if skip(&name.to_string_lossy()) {
            continue;
        }
        let target = to.join(&name);
        if entry.path().is_dir() {
            copy_dir_filtered(&entry.path(), &target, skip)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

// Re-export tool definitions for the server layer.
pub use engine_tools::tool_definitions;
