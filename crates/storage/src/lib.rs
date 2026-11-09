//! Local-first SQLite storage for OpenBot-RS.
//!
//! Mirrors OpenBot's "local data, crash-safe persistence" model: all state
//! (agents, messages, queues, channels, routines, settings) lives in a single
//! SQLite database inside the app's data directory.

pub mod db;
pub mod repositories;

pub use db::Db;
pub use repositories::{
    AgentRepository, ChannelMessageRepository, ChannelRepository, MessageRepository,
    QueueRepository, QuickActionRepository, RoutineRepository, SettingsRepository,
    TemplateRepository, TurnRepository,
};
