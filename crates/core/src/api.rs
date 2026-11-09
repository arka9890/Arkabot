//! Cross-crate service interfaces.
//!
//! `messaging` depends on these traits and `agent-engine` implements them,
//! which keeps the crate graph acyclic (messaging → core ← agent-engine).

use crate::ids::*;
use crate::models::{Attachment, Message, Sender};
use async_trait::async_trait;

/// Runs one agent turn (user prompt → tool loop → assistant reply).
#[async_trait]
pub trait TurnRunner: Send + Sync {
    /// Persists the user message and produces the assistant reply.
    async fn run_turn(
        &self,
        agent_id: &AgentId,
        prompt: String,
        sender: Sender,
        reply_to: Option<MessageId>,
        attachments: Vec<Attachment>,
    ) -> crate::Result<Message>;
}

/// Tells the engine that an agent has new work waiting (queue or channel).
#[async_trait::async_trait]
pub trait TurnTrigger: Send + Sync {
    fn notify_agent(&self, agent_id: AgentId);

    /// Steer mode: inject a message into the agent's currently running turn.
    /// Returns `true` if it was steered into a running turn, `false` if it fell
    /// back to the queue (which the engine persists on delivery).
    async fn steer(&self, agent_id: &AgentId, text: String) -> crate::Result<bool>;
}

/// Sends messages on behalf of agents (the `send_message` tool).
pub trait MessageSender: Send + Sync {
    /// Send a direct message to another agent.
    fn send_to_agent(
        &self,
        to: &AgentId,
        from: Option<&AgentId>,
        text: String,
        attachments: Vec<Attachment>,
    ) -> crate::Result<QueueItemView>;

    /// Post to a shared channel.
    fn send_to_channel(
        &self,
        channel_id: &ChannelId,
        from: Option<&AgentId>,
        text: String,
        delegation_of: Option<MessageId>,
    ) -> crate::Result<ChannelMessageView>;

    /// Brief agent list for the system prompt (id, name, role).
    fn agent_roster(&self) -> Vec<AgentBrief>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueItemView {
    pub id: String,
    pub agent_id: AgentId,
    pub text: String,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChannelMessageView {
    pub id: MessageId,
    pub channel_id: ChannelId,
    pub text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentBrief {
    pub id: AgentId,
    pub name: String,
    pub description: String,
}
