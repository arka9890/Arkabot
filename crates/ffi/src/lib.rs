//! uniffi bindings for the Android (Kotlin) app.
//!
//! Domain objects cross the FFI as JSON strings (compact, future-proof), and
//! events are pushed to a Kotlin `EventListener` registered on the handle.

use app_core::{AgentInput, OpenBot};

use crate::error::FfiError;

type FfiResult<T> = std::result::Result<T, FfiError>;
use openbot_core::ids::*;
use openbot_core::models::*;
use serde::Serialize;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

uniffi::setup_scaffolding!();

mod error;

/// Callbacks pushed from Rust into Kotlin.
#[uniffi::export(callback_interface)]
pub trait EventListener: Send + Sync {
    fn on_event(&self, kind: String, agent_id: Option<String>, channel_id: Option<String>, payload: String);
}

/// Handle to the running OpenBot workspace. Construct once, share everywhere.
#[derive(uniffi::Object)]
pub struct ArkaBotHandle {
    inner: Arc<OpenBot>,
    runtime: Runtime,
    listener: Arc<Mutex<Option<Box<dyn EventListener>>>>,
}

fn ser<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn parse_agent_input(s: &str) -> FfiResult<AgentInput> {
    serde_json::from_str(s).map_err(|e| FfiError::Core(e.to_string()))
}

fn parse_id(s: &str) -> FfiResult<AgentId> {
    AgentId::from_str(s).map_err(|e| FfiError::Core(e.to_string()))
}

#[uniffi::export]
impl ArkaBotHandle {
    /// Open (or create) a workspace and start the event forwarder.
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> FfiResult<Arc<ArkaBotHandle>> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("arkabot-rust")
            .build()
            .map_err(|e| openbot_core::Error::Internal(format!("runtime: {e}")))?;
        let inner = OpenBot::new(data_dir)?;
        inner.start_routine_scheduler();
        let handle = Arc::new(ArkaBotHandle {
            inner,
            runtime,
            listener: Arc::new(Mutex::new(None)),
        });

        let listener_slot = handle.listener.clone();
        let mut rx = handle.inner.subscribe();
        handle.runtime.spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let kind = serde_json::to_string(&event.kind)
                            .unwrap_or_default()
                            .trim_matches('"')
                            .to_string();
                        let guard = listener_slot.lock().unwrap();
                        if let Some(listener) = guard.as_ref() {
                            listener.on_event(
                                kind,
                                event.agent_id,
                                event.channel_id,
                                event.payload,
                            );
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
        });
        Ok(handle)
    }

    /// Register the Kotlin event listener (replaces any previous one).
    pub fn register_listener(&self, listener: Box<dyn EventListener>) {
        *self.listener.lock().unwrap() = Some(listener);
    }

    // ------- agents --------------------------------------------------------

    pub fn create_agent(&self, input_json: String) -> FfiResult<String> {
        Ok(ser(&self.inner.create_agent(parse_agent_input(&input_json)?)?))
    }

    pub fn update_agent(&self, agent_id: String, input_json: String) -> FfiResult<String> {
        Ok(ser(&self.inner.update_agent(&parse_id(&agent_id)?, parse_agent_input(&input_json)?)?))
    }

    pub fn list_agents(&self) -> FfiResult<String> {
        Ok(ser(&self.inner.list_agents()?))
    }

    pub fn get_agent(&self, agent_id: String) -> FfiResult<String> {
        Ok(ser(&self.inner.agent(&parse_id(&agent_id)?)?))
    }

    pub fn delete_agent(&self, agent_id: String) -> FfiResult<String> {
        self.inner.delete_agent(&parse_id(&agent_id)?)?;
        Ok("{\"ok\":true}".into())
    }

    pub fn restart_agent(&self, agent_id: String) -> FfiResult<String> {
        self.inner.restart_agent(&parse_id(&agent_id)?)?;
        Ok("{\"ok\":true}".into())
    }

    // ------- chat ----------------------------------------------------------

    pub fn send_message(&self, agent_id: String, text: String, reply_to: Option<String>) -> FfiResult<String> {
        let msg = self.inner.send_message(
            &parse_id(&agent_id)?,
            text,
            Vec::new(),
            reply_to.as_deref().and_then(|t| MessageId::from_str(t).ok()),
        )?;
        Ok(ser(&msg))
    }

    pub fn agent_messages(&self, agent_id: String, limit: u32) -> FfiResult<String> {
        Ok(ser(&self.inner.agent_messages(&parse_id(&agent_id)?, limit as usize)?))
    }

    pub fn react(&self, message_id: String, emoji: String) -> FfiResult<String> {
        Ok(ser(&self.inner.react(&MessageId::from_str(&message_id)?, &emoji)?))
    }

    pub fn message_reactions(&self, message_id: String) -> FfiResult<String> {
        Ok(ser(&self.inner.message_reactions(&MessageId::from_str(&message_id)?)?))
    }

    // ------- queue ---------------------------------------------------------

    pub fn queue_list(&self, agent_id: String) -> FfiResult<String> {
        Ok(ser(&self.inner.queue_list(&parse_id(&agent_id)?)?))
    }

    pub fn queue_pause(&self, agent_id: String) -> FfiResult<String> {
        self.inner.queue_pause(&parse_id(&agent_id)?)?;
        Ok("{\"ok\":true}".into())
    }

    pub fn queue_resume(&self, agent_id: String) -> FfiResult<String> {
        self.inner.queue_resume(&parse_id(&agent_id)?)?;
        Ok("{\"ok\":true}".into())
    }

    pub fn queue_cancel(&self, item_id: String) -> FfiResult<String> {
        self.inner.queue_cancel(&QueueId::from_str(&item_id)?)?;
        Ok("{\"ok\":true}".into())
    }

    // ------- channels ------------------------------------------------------

    pub fn create_channel(
        &self,
        name: String,
        description: String,
        task_owner: String,
        members_json: String,
    ) -> FfiResult<String> {
        let members: Vec<AgentId> = serde_json::from_str(&members_json).map_err(|e| FfiError::Core(e.to_string()))?;
        Ok(ser(&self.inner.create_channel(name, description, parse_id(&task_owner)?, members)?))
    }

    pub fn list_channels(&self) -> FfiResult<String> {
        Ok(ser(&self.inner.list_channels()?))
    }

    pub fn channel_messages(&self, channel_id: String, limit: u32) -> FfiResult<String> {
        Ok(ser(&self.inner.channel_messages(&ChannelId::from_str(&channel_id)?, limit as usize)?))
    }

    pub fn send_channel_message(&self, channel_id: String, text: String, delegation_of: Option<String>) -> FfiResult<String> {
        Ok(ser(&self.inner.send_channel_message(
            &ChannelId::from_str(&channel_id)?,
            text,
            delegation_of.as_deref().and_then(|t| MessageId::from_str(t).ok()),
        )?))
    }

    pub fn channel_action(&self, channel_id: String, action: String, agent_id: Option<String>) -> FfiResult<String> {
        Ok(ser(&self.inner.channel_action(
            &ChannelId::from_str(&channel_id)?,
            &action,
            agent_id.as_deref().and_then(|a| AgentId::from_str(a).ok()).as_ref(),
        )?))
    }

    pub fn channel_reaction(&self, message_id: String, emoji: String) -> FfiResult<String> {
        Ok(ser(&self.inner.channel_add_reaction(&MessageId::from_str(&message_id)?, &emoji)?))
    }

    pub fn delete_channel(&self, channel_id: String) -> FfiResult<String> {
        self.inner.delete_channel(&ChannelId::from_str(&channel_id)?)?;
        Ok("{\"ok\":true}".into())
    }

    // ------- workspace files ----------------------------------------------

    pub fn files_tree(&self, agent_id: String) -> FfiResult<String> {
        Ok(ser(&self.inner.workspace_tree(&parse_id(&agent_id)?)?))
    }

    pub fn workspace_read(&self, agent_id: String, path: String) -> FfiResult<String> {
        Ok(ser(&json!({ "content": self.inner.workspace_read(&parse_id(&agent_id)?, &path)? })))
    }

    pub fn workspace_write(&self, agent_id: String, path: String, content: String) -> FfiResult<String> {
        Ok(ser(&self.inner.workspace_write(&parse_id(&agent_id)?, &path, &content)?))
    }

    pub fn workspace_create_dir(&self, agent_id: String, path: String) -> FfiResult<String> {
        self.inner.workspace_create_dir(&parse_id(&agent_id)?, &path)?;
        Ok("{\"ok\":true}".into())
    }

    pub fn workspace_delete(&self, agent_id: String, path: String) -> FfiResult<String> {
        self.inner.workspace_delete(&parse_id(&agent_id)?, &path)?;
        Ok("{\"ok\":true}".into())
    }

    // ------- attachments ---------------------------------------------------

    pub fn save_attachment(&self, name: String, mime: String, data: Vec<u8>) -> FfiResult<String> {
        Ok(ser(&self.inner.save_attachment(&name, &mime, &data)?))
    }

    pub fn attachment_bytes(&self, attachment_id: String, name: String) -> FfiResult<Vec<u8>> {
        self.inner.attachment_bytes(&attachment_id, &name).map_err(FfiError::from)
    }

    // ------- settings -----------------------------------------------------

    pub fn settings(&self) -> FfiResult<String> {
        Ok(ser(&self.inner.get_settings()?))
    }

    pub fn update_settings(&self, settings_json: String) -> FfiResult<String> {
        let settings: AppSettings = serde_json::from_str(&settings_json).map_err(|e| FfiError::Core(e.to_string()))?;
        Ok(ser(&self.runtime.block_on(self.inner.update_settings(settings))?))
    }

    pub fn check_provider(&self) -> FfiResult<String> {
        Ok(ser(&self.runtime.block_on(self.inner.check_provider())?))
    }

    // ------- quick actions -------------------------------------------------

    pub fn quick_actions(&self) -> FfiResult<String> {
        Ok(ser(&self.inner.list_quick_actions()?))
    }

    pub fn add_quick_action(&self, label: String, prompt: String, icon: String, color: String) -> FfiResult<String> {
        Ok(ser(&self.inner.add_quick_action(label, prompt, icon, color)?))
    }

    pub fn delete_quick_action(&self, id: String) -> FfiResult<String> {
        self.inner.delete_quick_action(&id)?;
        Ok("{\"ok\":true}".into())
    }

    // ------- templates ------------------------------------------------------

    pub fn templates(&self) -> FfiResult<String> {
        Ok(ser(&self.inner.list_templates()?))
    }

    pub fn import_template(&self, template_json: String) -> FfiResult<String> {
        let template: AgentTemplate = serde_json::from_str(&template_json).map_err(|e| FfiError::Core(e.to_string()))?;
        Ok(ser(&self.inner.import_template_as_agent(template)?))
    }

    // ------- routines ------------------------------------------------------

    pub fn create_routine(&self, agent_id: String, name: String, prompt: String, interval_minutes: u64) -> FfiResult<String> {
        Ok(ser(&self.inner.create_routine(parse_id(&agent_id)?, name, prompt, interval_minutes)?))
    }

    pub fn list_routines(&self) -> FfiResult<String> {
        Ok(ser(&self.inner.list_routines()?))
    }

    pub fn delete_routine(&self, id: String) -> FfiResult<String> {
        self.inner.delete_routine(&RoutineId::from_str(&id)?)?;
        Ok("{\"ok\":true}".into())
    }

    // ------- search / data --------------------------------------------------

    pub fn search(&self, query: String) -> FfiResult<String> {
        Ok(ser(&self.inner.search(&query, 50)?))
    }

    pub fn export_data(&self, target_dir: String) -> FfiResult<String> {
        Ok(ser(&json!({ "path": self.inner.export_data(&target_dir)? })))
    }

    pub fn import_data(&self, source_dir: String) -> FfiResult<String> {
        self.inner.import_data(&source_dir)?;
        Ok("{\"ok\":true}".into())
    }
}
