//! OpenBot-RS core domain models.
//!
//! These mirror the OpenBot (nightly-labs/openbot) domain: agents, conversations,
//! task queues, agent-to-agent messaging, shared channel chats, routines, and
//! workspace files. The core crate is intentionally free of I/O so it can be
//! reused by storage, engine, server, and FFI layers.

uniffi::setup_scaffolding!();

pub mod api;
pub mod error;
pub mod ids;
pub mod models;

pub use api::{AgentBrief, ChannelMessageView, MessageSender, QueueItemView, TurnRunner, TurnTrigger};
pub use error::{Error, Result};
pub use ids::{AgentId, ChannelId, MessageId, QueueId, RoutineId, TurnId};
pub use models::*;
