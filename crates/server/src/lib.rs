//! Local HTTP/WebSocket API.
//!
//! Exposes the same operations the Android UI uses through FFI, so remote
//! clients (browser, CI scripts, the OpenBot mobile companion) can drive the
//! workspace: agents, chat, queues, channels, files, settings, and a
//! live event stream.

use app_core::{AgentInput, OpenBot};
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Path as AxumPath, Query, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json,
    Router,
};
use openbot_core::error::Result;
use openbot_core::ids::*;
use futures_util::SinkExt;
use openbot_core::models::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use futures_util::StreamExt;

#[derive(Clone)]
pub struct AppState {
    pub openbot: Arc<OpenBot>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/agents", get(list_agents).post(create_agent))
        .route("/api/agents/{id}", get(get_agent).put(update_agent).delete(delete_agent))
        .route("/api/agents/{id}/restart", post(restart_agent))
        .route("/api/agents/{id}/messages", get(agent_messages).post(send_message))
        .route("/api/messages/{id}/react", post(react_message))
        .route("/api/agents/{id}/queue", get(queue_list))
        .route("/api/agents/{id}/queue/pause", post(queue_pause))
        .route("/api/agents/{id}/queue/resume", post(queue_resume))
        .route("/api/queue/{item_id}/cancel", post(queue_cancel))
        .route("/api/channels", get(list_channels).post(create_channel))
        .route("/api/channels/{id}/messages", get(channel_messages).post(send_channel_message))
        .route("/api/channels/{id}/actions", post(channel_action))
        .route("/api/channels/{id}", delete(delete_channel))
        .route("/api/settings", get(get_settings).put(update_settings))
        .route("/api/provider/check", get(check_provider))
        .route("/api/files/{agent_id}/tree", get(workspace_tree))
        .route("/api/files/{agent_id}/raw", get(workspace_read).post(workspace_write))
        .route("/api/search", get(search))
        .route("/api/quick-actions", get(quick_actions).post(add_quick_action))
        .route("/api/templates", get(list_templates))
        .route("/api/routines", get(list_routines).post(create_routine))
        .route("/api/routines/{id}", delete(delete_routine))
        .route("/api/events", get(ws_events))
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}

fn ok<T: Serialize>(v: T) -> Response {
    (StatusCode::OK, Json(json!({ "ok": true, "data": v }))).into_response()
}

fn err(e: openbot_core::Error) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "ok": false, "error": e.to_string() }))).into_response()
}

async fn health() -> Response {
    ok(json!({ "service": "openbot-rs", "version": env!("CARGO_PKG_VERSION") }))
}

async fn list_agents(State(s): State<AppState>) -> Response {
    match s.openbot.list_agents() {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct CreateAgentReq {
    #[serde(flatten)]
    pub input: AgentInput,
}

async fn create_agent(State(s): State<AppState>, Json(req): Json<CreateAgentReq>) -> Response {
    match s.openbot.create_agent(req.input) {
        Ok(agent) => ok(agent),
        Err(e) => err(e),
    }
}

async fn get_agent(State(s): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match s.openbot.agent(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new())) {
        Ok(Some(a)) => ok(a),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": "not found" }))).into_response(),
        Err(e) => err(e),
    }
}

async fn update_agent(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<AgentInput>,
) -> Response {
    match s.openbot.update_agent(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new()), input) {
        Ok(agent) => ok(agent),
        Err(e) => err(e),
    }
}

async fn delete_agent(State(s): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match s.openbot.delete_agent(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new())) {
        Ok(()) => ok(json!({ "deleted": id })),
        Err(e) => err(e),
    }
}

async fn restart_agent(State(s): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match s.openbot.restart_agent(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new())) {
        Ok(()) => ok(json!({ "restarted": id })),
        Err(e) => err(e),
    }
}

async fn agent_messages(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    match s.openbot.agent_messages(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new()), 200) {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct SendMessageReq {
    text: String,
    reply_to: Option<String>,
}

async fn send_message(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<SendMessageReq>,
) -> Response {
    match s.openbot.send_message(
        &AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new()),
        req.text,
        Vec::new(),
        req.reply_to.as_deref().and_then(|t| MessageId::from_str(t).ok()),
    ) {
        Ok(m) => ok(m),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct ReactReq {
    emoji: String,
}

async fn react_message(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ReactReq>,
) -> Response {
    match s.openbot.react(&MessageId::from_str(&id).unwrap_or_else(|_| MessageId::new()), &req.emoji) {
        Ok(reactions) => ok(reactions),
        Err(e) => err(e),
    }
}

async fn queue_list(State(s): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match s.openbot.queue_list(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new())) {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

async fn queue_pause(State(s): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match s.openbot.queue_pause(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new())) {
        Ok(()) => ok(json!({ "paused": id })),
        Err(e) => err(e),
    }
}

async fn queue_resume(State(s): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match s.openbot.queue_resume(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new())) {
        Ok(()) => ok(json!({ "resumed": id })),
        Err(e) => err(e),
    }
}

async fn queue_cancel(State(s): State<AppState>, AxumPath(item_id): AxumPath<String>) -> Response {
    match s.openbot.queue_cancel(&QueueId::from_str(&item_id).unwrap_or_else(|_| QueueId::new())) {
        Ok(()) => ok(json!({ "cancelled": item_id })),
        Err(e) => err(e),
    }
}

async fn list_channels(State(s): State<AppState>) -> Response {
    match s.openbot.list_channels() {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct CreateChannelReq {
    name: String,
    description: String,
    task_owner: String,
    members: Vec<String>,
}

async fn create_channel(State(s): State<AppState>, Json(req): Json<CreateChannelReq>) -> Response {
    match s.openbot.create_channel(
        req.name,
        req.description,
        AgentId::from_str(&req.task_owner).unwrap_or_default(),
        req.members.iter().filter_map(|m| AgentId::from_str(m).ok()).collect(),
    ) {
        Ok(channel) => ok(channel),
        Err(e) => err(e),
    }
}

async fn channel_messages(State(s): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match s.openbot.channel_messages(&ChannelId::from_str(&id).unwrap_or_else(|_| ChannelId::new()), 200) {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct ChannelMessageReq {
    text: String,
    delegation_of: Option<String>,
}

async fn send_channel_message(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ChannelMessageReq>,
) -> Response {
    match s.openbot.send_channel_message(
        &ChannelId::from_str(&id).unwrap_or_else(|_| ChannelId::new()),
        req.text,
        req.delegation_of.as_deref().and_then(|t| MessageId::from_str(t).ok()),
    ) {
        Ok(m) => ok(m),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct ChannelActionReq {
    action: String,
    agent_id: Option<String>,
}

async fn channel_action(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<ChannelActionReq>,
) -> Response {
    match s.openbot.channel_action(
        &ChannelId::from_str(&id).unwrap_or_else(|_| ChannelId::new()),
        &req.action,
        req.agent_id.as_deref().and_then(|a| AgentId::from_str(a).ok()).as_ref(),
    ) {
        Ok(channel) => ok(channel),
        Err(e) => err(e),
    }
}

async fn delete_channel(State(s): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match s.openbot.delete_channel(&ChannelId::from_str(&id).unwrap_or_else(|_| ChannelId::new())) {
        Ok(()) => ok(json!({ "deleted": id })),
        Err(e) => err(e),
    }
}

async fn get_settings(State(s): State<AppState>) -> Response {
    match s.openbot.get_settings() {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

async fn update_settings(State(s): State<AppState>, Json(settings): Json<AppSettings>) -> Response {
    match s.openbot.update_settings(settings).await {
        Ok(status) => ok(status),
        Err(e) => err(e),
    }
}

async fn check_provider(State(s): State<AppState>) -> Response {
    match s.openbot.check_provider().await {
        Ok(status) => ok(status),
        Err(e) => err(e),
    }
}

async fn workspace_tree(State(s): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match s.openbot.workspace_tree(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new())) {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

#[derive(serde::Deserialize)]
struct RawQuery {
    path: String,
}

async fn workspace_read(State(s): State<AppState>, AxumPath(id): AxumPath<String>, Query(q): Query<RawQuery>) -> Response {
    match s.openbot.workspace_read(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new()), &q.path) {
        Ok(content) => ok(json!({ "content": content })),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct WriteFileReq {
    path: String,
    content: String,
}

async fn workspace_write(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<WriteFileReq>,
) -> Response {
    match s.openbot.workspace_write(&AgentId::from_str(&id).unwrap_or_else(|_| AgentId::new()), &req.path, &req.content) {
        Ok(entry) => ok(entry),
        Err(e) => err(e),
    }
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search(State(s): State<AppState>, Query(q): Query<SearchQuery>) -> Response {
    match s.openbot.search(&q.q, 50) {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

async fn quick_actions(State(s): State<AppState>) -> Response {
    match s.openbot.list_quick_actions() {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct QuickActionReq {
    label: String,
    prompt: String,
    icon: String,
    color: String,
}

async fn add_quick_action(State(s): State<AppState>, Json(req): Json<QuickActionReq>) -> Response {
    match s.openbot.add_quick_action(req.label, req.prompt, req.icon, req.color) {
        Ok(action) => ok(action),
        Err(e) => err(e),
    }
}

async fn list_templates(State(s): State<AppState>) -> Response {
    match s.openbot.list_templates() {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

async fn list_routines(State(s): State<AppState>) -> Response {
    match s.openbot.list_routines() {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct RoutineReq {
    agent_id: String,
    name: String,
    prompt: String,
    interval_minutes: u64,
}

async fn create_routine(State(s): State<AppState>, Json(req): Json<RoutineReq>) -> Response {
    match s.openbot.create_routine(
        AgentId::from_str(&req.agent_id).unwrap_or_default(),
        req.name,
        req.prompt,
        req.interval_minutes,
    ) {
        Ok(routine) => ok(routine),
        Err(e) => err(e),
    }
}

async fn delete_routine(State(s): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    match s.openbot.delete_routine(&RoutineId::from_str(&id).unwrap_or_else(|_| RoutineId::new())) {
        Ok(()) => ok(json!({ "deleted": id })),
        Err(e) => err(e),
    }
}

async fn ws_events(State(s): State<AppState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| handle_ws(socket, s))
}

async fn handle_ws(socket: WebSocket, state: AppState) {
    let mut rx = state.openbot.subscribe();
    let (mut sender, mut receiver) = socket.split();

    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if sender.send(WsMessage::Text(event.payload.clone().into())).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(WsMessage::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}

/// Convenience for tests and scripts.
pub async fn serve(openbot: Arc<OpenBot>, bind: &str) -> Result<()> {
    let state = AppState { openbot };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
