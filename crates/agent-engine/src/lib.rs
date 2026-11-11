//! OpenBot-RS agent engine.
//!
//! Runs agent turns (prompt → LLM → tools → reply), delivers queued tasks,
//! compacts long conversations, and wires tools (messaging, workspace files,
//! browser) into the agent loop.

pub mod tools;
pub mod workspace;

use browser::BrowserControl;
use openbot_core::api::{MessageSender, TurnRunner, TurnTrigger};
use openbot_core::error::{Error, Result};
use openbot_core::ids::*;
use openbot_core::models::*;
use openbot_core::AppEvent;
use llm::{
    build_client, ChatMessage, ChatRequest, ChatResponse, LlmClient, ProviderConfig,
};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::Mutex as AsyncMutex;
use storage::{AgentRepository, Db, MessageRepository, QueueRepository, SettingsRepository, TurnRepository};
use tokio::sync::broadcast;
use workspace::WorkspaceStore;

/// The agent brain: one instance drives all agents.
pub struct AgentEngine {
    inner: Arc<EngineInner>,
}

struct EngineInner {
    db: Arc<Db>,
    data_dir: PathBuf,
    llm: RwLock<Arc<Box<dyn LlmClient>>>,
    events: broadcast::Sender<AppEvent>,
    /// Tokio handle used for all background spawns. When the engine is created
    /// inside an existing runtime (e.g. the HTTP server) this reuses it;
    /// otherwise (Android FFI) it owns a private runtime so `spawn` works from
    /// any thread without panicking.
    handle: tokio::runtime::Handle,
    /// Per-agent locks serialise turns (one turn at a time per agent).
    locks: std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    messaging: RwLock<Option<Arc<dyn MessageSender>>>,
    browser: RwLock<Option<Arc<dyn BrowserControl>>>,
    /// Named browser profiles. An agent with `browser_profile` set drives
    /// the profile of that name; a name that is not registered gives the
    /// agent no browser, so two agents never share one session.
    browser_profiles: RwLock<HashMap<String, Arc<dyn BrowserControl>>>,
    /// Queue pause flags, persisted in settings KV.
    paused: std::sync::Mutex<HashMap<String, bool>>,
    /// Buffer of the currently running turn, for steer mode.
    turn_buffers: std::sync::Mutex<HashMap<String, Arc<AsyncMutex<Vec<ChatMessage>>>>>,
}

impl AgentEngine {
    /// Build the engine from a database handle, initial settings, a shared
    /// event channel, and a tokio handle used for background spawns.
    pub fn new(
        db: Arc<Db>,
        data_dir: PathBuf,
        settings: AppSettings,
        events: broadcast::Sender<AppEvent>,
        handle: tokio::runtime::Handle,
    ) -> Arc<Self> {
        let client = build_client(&ProviderConfig {
            base_url: settings.base_url.clone(),
            api_key: settings.api_key.clone(),
            model: settings.model.clone(),
            temperature: settings.temperature,
            max_tokens: None,
            acp_command: settings.acp_command.clone(),
            acp_args: settings.acp_args.clone(),
            acp_cwd: settings.acp_cwd.clone(),
        });
        let client = Arc::new(client);
        let inner = EngineInner {
            db: db.clone(),
            data_dir,
            llm: RwLock::new(client),
            events,
            handle,
            locks: Mutex::new(HashMap::new()),
            messaging: RwLock::new(None),
            browser: RwLock::new(None),
            browser_profiles: RwLock::new(HashMap::new()),
            paused: Mutex::new(HashMap::new()),
            turn_buffers: Mutex::new(HashMap::new()),
        };
        Arc::new(Self { inner: Arc::new(inner) })
    }

    // -----------------------------------------------------------------------
    // Wiring
    // -----------------------------------------------------------------------

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.inner.events.subscribe()
    }

    fn emit(
        &self,
        kind: EventKind,
        agent_id: Option<String>,
        channel_id: Option<String>,
        payload: String,
    ) {
        let _ = self.inner.events.send(AppEvent {
            kind,
            agent_id,
            channel_id,
            payload,
        });
    }

    pub fn set_messaging(&self, messaging: Arc<dyn MessageSender>) {
        *self.inner.messaging.write().unwrap() = Some(messaging);
    }

    pub fn set_browser(&self, browser: Arc<dyn BrowserControl>) {
        *self.inner.browser.write().unwrap() = Some(browser);
    }

    /// Register a named browser profile (one WebView or Chrome instance).
    pub fn set_browser_profile(&self, name: &str, browser: Arc<dyn BrowserControl>) {
        self.inner
            .browser_profiles
            .write()
            .unwrap()
            .insert(name.to_string(), browser);
    }

    /// The browser an agent drives: its named profile when it has one,
    /// otherwise the shared default.
    pub fn browser_for(&self, agent: &Agent) -> Option<Arc<dyn BrowserControl>> {
        if !agent.browser_profile.is_empty() {
            return self
                .inner
                .browser_profiles
                .read()
                .unwrap()
                .get(&agent.browser_profile)
                .cloned();
        }
        self.inner.browser.read().unwrap().clone()
    }

    pub fn messaging_sender(&self) -> Option<Arc<dyn MessageSender>> {
        self.inner.messaging.read().unwrap().clone()
    }

    pub fn browser_control(&self) -> Option<Arc<dyn BrowserControl>> {
        self.inner.browser.read().unwrap().clone()
    }

    pub fn workspace(&self) -> WorkspaceStore {
        WorkspaceStore::new(self.inner.data_dir.clone())
    }

    // -----------------------------------------------------------------------
    // Repositories
    // -----------------------------------------------------------------------

    pub fn agents(&self) -> AgentRepository<'_> {
        AgentRepository::new(&self.inner.db)
    }

    pub fn messages(&self) -> MessageRepository<'_> {
        MessageRepository::new(&self.inner.db)
    }

    pub fn queue(&self) -> QueueRepository<'_> {
        QueueRepository::new(&self.inner.db)
    }

    pub fn turns(&self) -> TurnRepository<'_> {
        TurnRepository::new(&self.inner.db)
    }

    pub fn settings(&self) -> SettingsRepository<'_> {
        SettingsRepository::new(&self.inner.db)
    }

    pub fn load_settings(&self) -> Result<AppSettings> {
        self.settings().get_json::<AppSettings>("app_settings")
    }

    pub fn update_settings(&self, settings: AppSettings) -> Result<()> {
        self.settings().set_json("app_settings", &settings)?;
        // Rebuild the provider client.
        let client = build_client(&ProviderConfig {
            base_url: settings.base_url.clone(),
            api_key: settings.api_key.clone(),
            model: settings.model.clone(),
            temperature: settings.temperature,
            max_tokens: None,
            acp_command: settings.acp_command.clone(),
            acp_args: settings.acp_args.clone(),
            acp_cwd: settings.acp_cwd.clone(),
        });
        *self.inner.llm.write().unwrap() = Arc::new(client);
        self.emit(
            EventKind::SettingsChanged,
            None,
            None,
            json!({ "provider": settings.provider.as_str(), "model": settings.model }).to_string(),
        );
        Ok(())
    }

    fn model_for(&self, agent: &Agent) -> String {
        if !agent.model.is_empty() {
            agent.model.clone()
        } else {
            let settings = self.load_settings().unwrap_or_default();
            if settings.model.is_empty() {
                "simulated".into()
            } else {
                settings.model
            }
        }
    }

    pub fn agent(&self, id: &AgentId) -> Result<Agent> {
        self.agents()
            .get(id)?
            .ok_or_else(|| Error::NotFound(format!("agent {}", id)))
    }

    pub fn list_agents_for_tools(&self) -> Result<Vec<Agent>> {
        self.agents().list()
    }

    // -----------------------------------------------------------------------
    // Turns
    // -----------------------------------------------------------------------

    fn agent_lock(&self, id: &AgentId) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.inner.locks.lock().unwrap();
        locks
            .entry(id.as_str().to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// TurnRunner implementation: persists the user message, runs the turn,
    /// persists the assistant reply.
    async fn run_turn(
        &self,
        agent_id: &AgentId,
        prompt: String,
        sender: Sender,
        reply_to: Option<MessageId>,
        attachments: Vec<Attachment>,
    ) -> Result<Message> {
        let agent = self.agent(agent_id)?;
        self.workspace().ensure_root(agent_id)?;

        // Persist the incoming message (user, agent-to-agent, or routine).
        let user_msg = Message {
            id: MessageId::new(),
            agent_id: agent_id.clone(),
            sender: sender.clone(),
            text: prompt.clone(),
            reply_to: reply_to.clone(),
            attachments: attachments.clone(),
            reactions: Vec::new(),
            created_at: chrono::Utc::now(),
            turn_id: None,
        };
        self.messages().add(&user_msg)?;
        self.emit(
            EventKind::MessageAdded,
            Some(agent_id.as_str().to_string()),
            None,
            json!({ "message_id": user_msg.id.as_str(), "sender": format!("{:?}", sender), "text": prompt.clone() }).to_string(),
        );

        // Serialise turns per agent.
        let lock = self.agent_lock(agent_id);
        let _guard = lock.lock().await;

        self.agents()
            .set_phase(agent_id, AgentPhase::Starting)
            .ok();
        self.emit(
            EventKind::AgentPhaseChanged,
            Some(agent_id.as_str().to_string()),
            None,
            json!({ "phase": "starting" }).to_string(),
        );

        let turn = Turn {
            id: TurnId::new(),
            agent_id: agent_id.clone(),
            status: TurnStatus::Running,
            prompt: prompt.clone(),
            started_at: chrono::Utc::now(),
            ended_at: None,
            error: None,
        };
        self.turns().create(&turn).ok();

        let buffer: Arc<AsyncMutex<Vec<ChatMessage>>> = Arc::new(AsyncMutex::new(Vec::new()));
        {
            self.inner
                .turn_buffers
                .lock()
                .unwrap()
                .insert(agent_id.as_str().to_string(), buffer.clone());
        }

        let result = self.execute_turn(&agent, &turn, &prompt, buffer.clone()).await;

        // Steer buffer cleanup.
        self.inner.turn_buffers.lock().unwrap().remove(agent_id.as_str());

        match result {
            Ok(reply) => {
                self.turns()
                    .finish(&turn.id, TurnStatus::Completed, None)
                    .ok();
                self.emit(
                    EventKind::TurnCompleted,
                    Some(agent_id.as_str().to_string()),
                    None,
                    json!({ "turn_id": turn.id.as_str(), "reply_message_id": reply.id.as_str() }).to_string(),
                );
                Ok(reply)
            }
            Err(e) => {
                self.turns()
                    .finish(&turn.id, TurnStatus::Failed, Some(&e.to_string()))
                    .ok();
                self.emit(
                    EventKind::TurnFailed,
                    Some(agent_id.as_str().to_string()),
                    None,
                    json!({ "turn_id": turn.id.as_str(), "error": e.to_string() }).to_string(),
                );
                Err(e)
            }
        }
    }

    async fn execute_turn(
        &self,
        agent: &Agent,
        turn: &Turn,
        _prompt: &str,
        steer_buffer: Arc<AsyncMutex<Vec<ChatMessage>>>,
    ) -> Result<Message> {
        let model = self.model_for(agent);

        // 1. System prompt.
        let system = self.build_system_prompt(agent).await;
        let mut messages = vec![ChatMessage::system(system)];

        // 2. Conversation history with compaction.
        let history = self.messages().list_for_agent(&agent.id, 200)?;
        if history.len() > agent.compaction_messages as usize {
            let summary = self.summarize(agent, &history).await;
            messages.push(ChatMessage::system(format!(
                "[Riwayat] Ringkasan percakapan sebelumnya:\n{summary}\nSekarang lanjutkan percakapan baru."
            )));
            let keep_from = history.len().saturating_sub(20);
            for m in &history[keep_from..] {
                messages.push(history_to_chat(m));
            }
        } else {
            for m in &history {
                messages.push(history_to_chat(m));
            }
        }

        // 3. Tool loop.
        let tools = tools::tool_definitions(agent, self.browser_for(agent).is_some());
        let mut reply_text = String::new();
        let mut last_error = None;

        for _ in 0..agent.max_tool_iterations.max(1) {
            // Steered messages join the running turn at the next provider step.
            let steered: Vec<ChatMessage> = {
                let guard = steer_buffer.lock().await;
                guard.clone()
            };
            let mut request_messages = messages.clone();
            request_messages.extend(steered);

            let client = self.inner.llm.read().unwrap().clone();
            let response = client
                .chat(&ChatRequest {
                    model: model.clone(),
                    messages: request_messages,
                    temperature: Some(agent.temperature),
                    max_tokens: None,
                    tools: tools.clone(),
                })
                .await;

            let ChatResponse {
                content,
                tool_calls,
                finish_reason,
                ..
            } = match response {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(e.to_string());
                    break;
                }
            };

            if tool_calls.is_empty() {
                reply_text = content;
                break;
            }

            messages.push(ChatMessage {
                role: llm::ChatRole::Assistant,
                content: content.clone(),
                tool_calls: Some(tool_calls.clone()),
                tool_call_id: None,
            });

            for call in &tool_calls {
                let result = tools::execute_tool(self, agent, call).await;
                messages.push(match result {
                    Ok(text) => ChatMessage::tool_result(call.id.clone(), text),
                    Err(e) => ChatMessage::tool_result(call.id.clone(), json!({ "ok": false, "error": e.to_string() }).to_string()),
                });
            }

            if finish_reason == "length" {
                last_error = Some("model menghasilkan tool_calls tanpa henti".into());
                break;
            }
        }

        if reply_text.is_empty() {
            if let Some(err) = last_error {
                return Err(Error::Llm(err));
            }
            reply_text = "(agent tidak memberikan balasan untuk pesan ini.)".to_string();
        }

        // 4. Persist the assistant reply.
        let reply = Message {
            id: MessageId::new(),
            agent_id: agent.id.clone(),
            sender: Sender::Agent {
                agent_id: agent.id.clone(),
            },
            text: reply_text.clone(),
            reply_to: None,
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at: chrono::Utc::now(),
            turn_id: Some(turn.id.clone()),
        };
        self.messages().add(&reply)?;
        self.emit(
            EventKind::MessageAdded,
            Some(agent.id.as_str().to_string()),
            None,
            json!({ "message_id": reply.id.as_str(), "sender": "agent", "text": reply_text.clone() }).to_string(),
        );
        Ok(reply)
    }

    async fn build_system_prompt(&self, agent: &Agent) -> String {
        let roster = match self.inner.messaging.read().unwrap().clone() {
            Some(m) => m.agent_roster(),
            None => Vec::new(),
        };
        let roster_text = if roster.is_empty() {
            "Tidak ada agent lain saat ini.".to_string()
        } else {
            roster
                .iter()
                .map(|a| format!("- {} (id: {}): {}", a.name, a.id.as_str(), a.description))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let workspace_hint = self
            .workspace()
            .list(&agent.id, "")
            .map(|entries| {
                entries
                    .iter()
                    .map(|e| format!("  - {}", e.path))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let tools_hint = if self.browser_for(agent).is_some() {
            "Anda memiliki akses ke browser terkontrol (browser_navigate, browser_evaluate, browser_screenshot)."
        } else {
            "Browser tidak tersedia di perangkat ini."
        };

        format!(
            "Anda adalah agent bernama \"{}\" di dalam OpenBot-RS, sebuah workspace AI yang berjalan di Android.\n\
             \nPROFIL:\n{}",
            agent.name, agent.instructions
        ) + &format!(
            "\n\nAGENTS LAIN:\n{roster_text}\n\n\
             TOOLS: {tools_hint}. Gunakan send_message untuk berkolaborasi dengan agent lain. Gunakan read_file/write_file/list_files untuk mengelola workspace Anda.\n\
             \nWORKSPACE ANDA (root):\n{workspace_hint}\n\n\
             Panduan: jawab dengan bahasa yang digunakan lawan bicara (default: Indonesia). Jangan membuat tool call tanpa kebutuhan.",
        )
    }

    async fn summarize(&self, agent: &Agent, history: &[Message]) -> String {
        let mut text = String::new();
        for m in history.iter().take(30) {
            let who = match &m.sender {
                Sender::User => "user",
                Sender::Agent { agent_id } => agent_id.as_str(),
                Sender::Routine { name, .. } => name,
            };
            text.push_str(&format!("{}: {}\n", who, m.text.chars().take(200).collect::<String>()));
        }
        let request = ChatRequest {
            model: self.model_for(agent),
            messages: vec![
                ChatMessage::system("Ringkas percakapan berikut dalam 3-5 kalimat penting, bahasa Indonesia."),
                ChatMessage::user(&text),
            ],
            temperature: Some(0.3),
            max_tokens: None,
            tools: Vec::new(),
        };
        let client = self.inner.llm.read().unwrap().clone();
        match client.chat(&request).await {
            Ok(r) => r.content,
            Err(_) => "[riwayat panjang, dirangkum otomatis]".to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // Queue engine
    // -----------------------------------------------------------------------

    /// Enqueue a task/message for an agent.
    pub async fn enqueue_task(
        &self,
        agent_id: &AgentId,
        sender: Sender,
        text: String,
        reply_to: Option<MessageId>,
        attachments: Vec<Attachment>,
    ) -> Result<QueueItem> {
        let item = QueueItem {
            id: QueueId::new(),
            agent_id: agent_id.clone(),
            sender: sender.clone(),
            text,
            reply_to,
            attachments,
            status: QueueStatus::Queued,
            position: None,
            turn_id: None,
            error: None,
            held: false,
            created_at: chrono::Utc::now(),
        };
        self.queue().insert(&item)?;
        self.emit(
            EventKind::QueueItemAdded,
            Some(agent_id.as_str().to_string()),
            None,
            json!({ "queue_id": item.id.as_str(), "text": item.text.clone(), "position": item.position }).to_string(),
        );
        // Wake the engine so the queued task is actually delivered.
        self.notify_agent(agent_id.clone());
        Ok(item)
    }

    pub fn queue_count(&self, agent_id: &AgentId) -> Result<u64> {
        self.queue().count_queued(agent_id)
    }

    pub fn queue_list(&self, agent_id: &AgentId) -> Result<Vec<QueueItem>> {
        self.queue().list_for_agent(agent_id)
    }

    /// Deliver queued tasks one by one until the queue drains or is paused.
    pub async fn deliver_queue(&self, agent_id: &AgentId) {
        if self.is_paused(agent_id) {
            return;
        }
        loop {
            let item = match self.queue().next_queued(agent_id) {
                Ok(Some(item)) => item,
                Ok(None) => break,
                Err(_) => break,
            };
            self.queue()
                .update_status(&item.id, QueueStatus::Starting, None, None, None)
                .ok();
            self.emit(
                EventKind::QueueItemUpdated,
                Some(agent_id.as_str().to_string()),
                None,
                json!({ "queue_id": item.id.as_str(), "status": "starting" }).to_string(),
            );

            let result = self
                .run_turn(
                    agent_id,
                    item.text.clone(),
                    item.sender.clone(),
                    item.reply_to.clone(),
                    item.attachments.clone(),
                )
                .await;

            match result {
                Ok(reply) => {
                    // Use the turn id the engine actually created for this run.
                    self.queue()
                        .update_status(
                            &item.id,
                            QueueStatus::Completed,
                            None,
                            None,
                            reply.turn_id.as_ref(),
                        )
                        .ok();
                }
                Err(e) => {
                    self.queue()
                        .update_status(&item.id, QueueStatus::Failed, None, Some(&e.to_string()), None)
                        .ok();
                }
            }
            self.emit(
                EventKind::QueueItemUpdated,
                Some(agent_id.as_str().to_string()),
                None,
                json!({ "queue_id": item.id.as_str(), "status": "done" }).to_string(),
            );

            if self.is_paused(agent_id) {
                break;
            }
        }
        // Agent is idle again.
        self.agents().set_phase(agent_id, AgentPhase::Idle).ok();
        self.emit(
            EventKind::AgentPhaseChanged,
            Some(agent_id.as_str().to_string()),
            None,
            json!({ "phase": "idle" }).to_string(),
        );
    }

    fn is_paused(&self, agent_id: &AgentId) -> bool {
        self.inner.paused.lock().unwrap().get(agent_id.as_str()).copied().unwrap_or(false)
    }

    pub fn queue_pause(&self, agent_id: &AgentId) -> Result<()> {
        self.settings()
            .set(&format!("queue_paused:{}", agent_id.as_str()), "1")?;
        self.inner
            .paused
            .lock()
            .unwrap()
            .insert(agent_id.as_str().to_string(), true);
        self.emit(
            EventKind::QueueItemUpdated,
            Some(agent_id.as_str().to_string()),
            None,
            json!({ "paused": true }).to_string(),
        );
        Ok(())
    }

    pub fn queue_resume(&self, agent_id: &AgentId) -> Result<()> {
        self.settings()
            .set(&format!("queue_paused:{}", agent_id.as_str()), "0")?;
        self.inner
            .paused
            .lock()
            .unwrap()
            .insert(agent_id.as_str().to_string(), false);
        self.emit(
            EventKind::QueueItemUpdated,
            Some(agent_id.as_str().to_string()),
            None,
            json!({ "paused": false }).to_string(),
        );
        Ok(())
    }

    pub fn queue_cancel(&self, item_id: &QueueId) -> Result<()> {
        let Some(item) = self.queue().get(item_id)? else {
            return Err(Error::NotFound(format!("queue item {}", item_id)));
        };
        self.queue()
            .update_status(&item.id, QueueStatus::Cancelled, None, None, None)?;
        self.emit(
            EventKind::QueueItemUpdated,
            Some(item.agent_id.as_str().to_string()),
            None,
            json!({ "queue_id": item.id.as_str(), "status": "cancelled" }).to_string(),
        );
        Ok(())
    }

    /// Steer mode: append a message to the currently running turn.
    pub async fn steer_message(&self, agent_id: &AgentId, text: String) -> Result<bool> {
        let key = agent_id.as_str().to_string();
        let is_running = self.inner.turn_buffers.lock().unwrap().contains_key(&key);
        if is_running {
            let buffer = self.inner.turn_buffers.lock().unwrap().get(&key).cloned();
            if let Some(buffer) = buffer {
                buffer.lock().await.push(ChatMessage::user(text));
                return Ok(true);
            }
        }
        // Fallback: queue it.
        self.enqueue_task(agent_id, Sender::User, text, None, Vec::new())
            .await?;
        Ok(false)
    }

    /// Restore pause flags from storage.
    pub fn load_pause_state(&self) {
        let mut paused = HashMap::new();
        for (key, value) in self.settings().all().unwrap_or_default() {
            if let Some(rest) = key.strip_prefix("queue_paused:") {
                paused.insert(rest.to_string(), value == "1");
            }
        }
        *self.inner.paused.lock().unwrap() = paused;
    }

    /// TurnTrigger implementation: wake the engine to deliver queued work.
    fn notify_agent(&self, agent_id: AgentId) {
        let engine = self.clone();
        self.inner.handle.spawn(async move {
            engine.deliver_queue(&agent_id).await;
        });
    }
}

impl Clone for AgentEngine {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[async_trait::async_trait]
impl TurnRunner for AgentEngine {
    async fn run_turn(
        &self,
        agent_id: &AgentId,
        prompt: String,
        sender: Sender,
        reply_to: Option<MessageId>,
        attachments: Vec<Attachment>,
    ) -> Result<Message> {
        AgentEngine::run_turn(self, agent_id, prompt, sender, reply_to, attachments).await
    }
}

#[async_trait::async_trait]
impl TurnTrigger for AgentEngine {
    fn notify_agent(&self, agent_id: AgentId) {
        AgentEngine::notify_agent(self, agent_id);
    }

    async fn steer(&self, agent_id: &AgentId, text: String) -> Result<bool> {
        self.steer_message(agent_id, text).await
    }
}

fn history_to_chat(m: &Message) -> ChatMessage {
    let role = match &m.sender {
        Sender::User => llm::ChatRole::User,
        Sender::Agent { .. } | Sender::Routine { .. } => llm::ChatRole::Assistant,
    };
    ChatMessage {
        role,
        content: m.text.clone(),
        tool_calls: None,
        tool_call_id: None,
    }
}
