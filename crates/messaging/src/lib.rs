//! Messaging: direct agent-to-agent messages, shared channel chats,
//! task queue delivery, and routines.
//!
//! The service talks to the agent engine through the `TurnRunner` /
//! `TurnTrigger` traits from `core`, which keeps the crate graph acyclic.

use openbot_core::api::{AgentBrief, ChannelMessageView, MessageSender, QueueItemView, TurnRunner, TurnTrigger};
use openbot_core::error::{Error, Result};
use openbot_core::ids::*;
use openbot_core::models::*;
use openbot_core::AppEvent;
use serde_json::json;
use std::sync::Arc;
use storage::{
    AgentRepository, ChannelMessageRepository, ChannelRepository, QueueRepository,
    RoutineRepository,
};
use tokio::sync::broadcast;

pub struct MessagingService {
    db: Arc<storage::Db>,
    engine: Arc<dyn TurnRunner>,
    trigger: Arc<dyn TurnTrigger>,
    events: broadcast::Sender<AppEvent>,
    handle: tokio::runtime::Handle,
}

impl MessagingService {
    pub fn new(
        db: Arc<storage::Db>,
        engine: Arc<dyn TurnRunner>,
        trigger: Arc<dyn TurnTrigger>,
        events: broadcast::Sender<AppEvent>,
        handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            db,
            engine,
            trigger,
            events,
            handle,
        }
    }

    fn emit(&self, kind: EventKind, agent_id: Option<String>, channel_id: Option<String>, payload: String) {
        let _ = self.events.send(AppEvent {
            kind,
            agent_id,
            channel_id,
            payload,
        });
    }

    fn agents(&self) -> AgentRepository<'_> {
        AgentRepository::new(&self.db)
    }

    fn queue(&self) -> QueueRepository<'_> {
        QueueRepository::new(&self.db)
    }

    // ------- direct messages ------------------------------------------------

    /// Send a message from the user to an agent.
    ///
    /// Idle agents start immediately; busy agents receive the message through
    /// their queue (or steer mode when the agent allows it).
    pub fn send_user_message(
        &self,
        agent_id: &AgentId,
        text: String,
        attachments: Vec<Attachment>,
        reply_to: Option<MessageId>,
    ) -> Result<Message> {
        let agent = self.agents().get(agent_id)?.ok_or_else(|| Error::NotFound(format!("agent {agent_id}")))?;
        let message = Message {
            id: MessageId::new(),
            agent_id: agent_id.clone(),
            sender: Sender::User,
            text: text.clone(),
            reply_to,
            attachments,
            reactions: Vec::new(),
            created_at: chrono::Utc::now(),
            turn_id: None,
        };
        // The prompt is persisted exactly once by `run_turn` (whether the turn
        // starts immediately or is delivered from the queue later), so we must
        // not insert it here — otherwise every message would be duplicated.

        if !agent.phase.is_running() {
            let text = message.text.clone();
            let engine = self.engine.clone();
            let agent_id = agent_id.clone();
            let sender = message.sender.clone();
            let reply_to = message.reply_to.clone();
            let attachments = message.attachments.clone();
            self.handle.spawn(async move {
                let _ = engine.run_turn(&agent_id, text, sender, reply_to, attachments).await;
            });
        } else if agent.busy_mode == BusyMessageMode::Steer {
            // Steer mode: inject the message into the currently running turn.
            // If no turn is running, `steer` falls back to the queue (which is
            // persisted by `run_turn` on delivery).
            let trigger = self.trigger.clone();
            let agent_id = agent_id.clone();
            let text = message.text.clone();
            self.handle.spawn(async move {
                let _ = trigger.steer(&agent_id, text).await;
            });
        } else {
            self.enqueue(agent_id, Sender::User, text, message.reply_to.clone(), message.attachments.clone())?;
        }
        Ok(message)
    }

    /// Insert a queue item and wake the engine.
    pub fn enqueue(
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
        self.trigger.notify_agent(agent_id.clone());
        Ok(item)
    }

    // ------- channels -------------------------------------------------------

    pub fn create_channel(
        &self,
        name: String,
        description: String,
        task_owner: AgentId,
        members: Vec<AgentId>,
    ) -> Result<Channel> {
        let channel = Channel {
            id: ChannelId::new(),
            name,
            description,
            task_owner,
            status: ChannelStatus::Active,
            members,
            created_at: chrono::Utc::now(),
        };
        ChannelRepository::new(&self.db).create(&channel)?;
        self.emit(
            EventKind::ChannelCreated,
            Some(channel.task_owner.as_str().to_string()),
            Some(channel.id.as_str().to_string()),
            json!({ "channel_id": channel.id.as_str(), "name": channel.name.clone() }).to_string(),
        );
        Ok(channel)
    }

    pub fn channel_messages(&self, channel_id: &ChannelId, limit: usize) -> Result<Vec<ChannelMessage>> {
        ChannelMessageRepository::new(&self.db).list(channel_id, limit)
    }

    pub fn channel_add_reaction(&self, message_id: &MessageId, emoji: &str, reactor: &str) -> Result<Vec<Reaction>> {
        ChannelMessageRepository::new(&self.db).add_reaction(message_id, emoji, reactor)
    }

    pub fn channel_actions(&self, channel_id: &ChannelId, action: &str, agent_id: Option<&AgentId>) -> Result<Channel> {
        let repo = ChannelRepository::new(&self.db);
        let mut channel = repo
            .get(channel_id)?
            .ok_or_else(|| Error::NotFound(format!("channel {channel_id}")))?;
        let updated = match action {
            "stop" => {
                repo.set_status(&channel.id, ChannelStatus::Stopped)?;
                channel.status = ChannelStatus::Stopped;
                channel
            }
            "resume" => {
                repo.set_status(&channel.id, ChannelStatus::Active)?;
                channel.status = ChannelStatus::Active;
                channel
            }
            "archive" => {
                repo.set_status(&channel.id, ChannelStatus::Archived)?;
                channel.status = ChannelStatus::Archived;
                channel
            }
            "restore" => {
                repo.set_status(&channel.id, ChannelStatus::Active)?;
                channel.status = ChannelStatus::Active;
                channel
            }
            "reassign" => {
                let owner = agent_id.ok_or_else(|| Error::InvalidInput("reassign needs an agent".into()))?;
                repo.set_owner(&channel.id, owner)?;
                channel.task_owner = owner.clone();
                channel
            }
            _ => return Err(Error::InvalidInput(format!("unknown channel action: {action}"))),
        };
        self.emit(
            EventKind::ChannelUpdated,
            Some(updated.task_owner.as_str().to_string()),
            Some(updated.id.as_str().to_string()),
            json!({ "action": action, "status": updated.status.as_str() }).to_string(),
        );
        Ok(updated)
    }

    pub fn delete_channel(&self, channel_id: &ChannelId) -> Result<()> {
        ChannelRepository::new(&self.db).delete(channel_id)
    }

    // ------- routines ------------------------------------------------------

    pub fn create_routine(&self, routine: &Routine) -> Result<()> {
        RoutineRepository::new(&self.db).create(routine)
    }

    pub fn list_routines(&self) -> Result<Vec<Routine>> {
        RoutineRepository::new(&self.db).list()
    }

    pub fn delete_routine(&self, id: &RoutineId) -> Result<()> {
        RoutineRepository::new(&self.db).delete(id)
    }

    /// Run every routine that is due. Called periodically by the app core.
    pub async fn run_due_routines(&self) -> Result<()> {
        // Drain every due routine; `mark_run` advances `next_run_at` using the
        // routine's own interval, so the loop always terminates.
        loop {
            let Some(routine) = RoutineRepository::new(&self.db).next_due()? else {
                break;
            };
            let repo = RoutineRepository::new(&self.db);
            let agent_id = routine.agent_id.clone();
            let prompt = routine.prompt.clone();
            let engine = self.engine.clone();
            repo.mark_run(&routine.id, chrono::Utc::now(), routine.interval_minutes)?;
            self.emit(
                EventKind::RoutineUpdated,
                Some(agent_id.as_str().to_string()),
                None,
                json!({ "routine_id": routine.id.as_str(), "name": routine.name.clone(), "action": "run" }).to_string(),
            );
            self.handle.spawn(async move {
                let _ = engine
                    .run_turn(
                        &agent_id,
                        prompt,
                        Sender::Routine {
                            routine_id: routine.id.clone(),
                            run_id: uuid::Uuid::new_v4().to_string(),
                            name: routine.name.clone(),
                        },
                        None,
                        Vec::new(),
                    )
                    .await;
            });
        }
        Ok(())
    }

    // ------- MessageSender ------------------------------------------------

    fn user_message_view(&self, message: &Message) -> QueueItemView {
        QueueItemView {
            id: message.id.as_str().to_string(),
            agent_id: message.agent_id.clone(),
            text: message.text.clone(),
            status: "started".into(),
        }
    }
}

#[async_trait::async_trait]
impl MessageSender for MessagingService {
    fn send_to_agent(
        &self,
        to: &AgentId,
        from: Option<&AgentId>,
        text: String,
        attachments: Vec<Attachment>,
    ) -> Result<QueueItemView> {
        let agent = self
            .agents()
            .get(to)?
            .ok_or_else(|| Error::NotFound(format!("agent {to}")))?;

        if agent.phase.is_running() {
            let sender = from
                .cloned()
                .map(|agent_id| Sender::Agent { agent_id })
                .unwrap_or(Sender::User);
            let item = self.enqueue(to, sender, text.clone(), None, attachments)?;
            Ok(QueueItemView {
                id: item.id.as_str().to_string(),
                agent_id: to.clone(),
                text,
                status: "queued".into(),
            })
        } else {
            // Persist happens inside `run_turn`, so we only build the message
            // here for the return value / view (avoids duplicating the row).
            let message = Message {
                id: MessageId::new(),
                agent_id: to.clone(),
                sender: from
                    .cloned()
                    .map(|agent_id| Sender::Agent { agent_id })
                    .unwrap_or(Sender::User),
                text: text.clone(),
                reply_to: None,
                attachments,
                reactions: Vec::new(),
                created_at: chrono::Utc::now(),
                turn_id: None,
            };
            let engine = self.engine.clone();
            let to = to.clone();
            let sender = message.sender.clone();
            let msg_text = message.text.clone();
            self.handle.spawn(async move {
                let _ = engine.run_turn(&to, msg_text, sender, None, Vec::new()).await;
            });
            Ok(self.user_message_view(&message))
        }
    }

    fn send_to_channel(
        &self,
        channel_id: &ChannelId,
        from: Option<&AgentId>,
        text: String,
        delegation_of: Option<MessageId>,
    ) -> Result<ChannelMessageView> {
        let channel = ChannelRepository::new(&self.db)
            .get(channel_id)?
            .ok_or_else(|| Error::NotFound(format!("channel {channel_id}")))?;

        let message = ChannelMessage {
            id: MessageId::new(),
            channel_id: channel_id.clone(),
            sender: from
                .cloned()
                .map(|agent_id| Sender::Agent { agent_id })
                .unwrap_or(Sender::User),
            text: text.clone(),
            delegation_of,
            reply_to: None,
            attachments: Vec::new(),
            reactions: Vec::new(),
            created_at: chrono::Utc::now(),
        };
        ChannelMessageRepository::new(&self.db).add(&message)?;
        self.emit(
            EventKind::ChannelMessageAdded,
            Some(channel.task_owner.as_str().to_string()),
            Some(channel_id.as_str().to_string()),
            json!({ "message_id": message.id.as_str(), "sender": format!("{:?}", message.sender), "text": text.clone() }).to_string(),
        );

        // An agent message on an active channel wakes the channel's task owner.
        if channel.status == ChannelStatus::Active {
            if let Sender::Agent { agent_id: sender_id } = message.sender {
                if sender_id != channel.task_owner {
                    let task = format!(
                        "[Channel \"{}\"] Agent {} mengatakan:\n{}",
                        channel.name, sender_id.as_str(), text
                    );
                    self.enqueue(
                        &channel.task_owner,
                        Sender::Agent {
                            agent_id: sender_id.clone(),
                        },
                        task,
                        None,
                        Vec::new(),
                    )?;
                }
            } else if let Sender::User = message.sender {
                let task = format!("[Channel \"{}\"] User mengatakan:\n{}", channel.name, text);
                self.enqueue(&channel.task_owner, Sender::User, task, None, Vec::new())?;
            }
        }

        Ok(ChannelMessageView {
            id: message.id,
            channel_id: channel_id.clone(),
            text,
        })
    }

    fn agent_roster(&self) -> Vec<AgentBrief> {
        self.agents()
            .list()
            .unwrap_or_default()
            .iter()
            .map(|a| AgentBrief {
                id: a.id.clone(),
                name: a.name.clone(),
                description: a.description.clone(),
            })
            .collect()
    }
}
