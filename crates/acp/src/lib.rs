//! Agent Client Protocol (ACP) client.
//!
//! ACP is JSON-RPC 2.0 over the agent's stdio, one JSON object per line.
//! This crate speaks the client side: it starts an agent process,
//! negotiates the protocol version, opens a session, and runs prompt turns,
//! collecting the streamed output the agent reports.
//!
//! Reference: <https://agentclientprotocol.com>.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot};

/// The protocol version this client speaks.
pub const PROTOCOL_VERSION: u32 = 1;

/// Errors raised while talking to an ACP agent.
#[derive(Debug, thiserror::Error)]
pub enum AcpError {
    #[error("acp: agent did not start: {0}")]
    Spawn(String),

    #[error("acp: agent exited: {0}")]
    Exited(String),

    #[error("acp: protocol error: {0}")]
    Protocol(String),

    #[error("acp: agent reported an error: {0}")]
    Agent(String),

    #[error("acp: {0}")]
    Io(String),
}

/// A tool call the agent reported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallReport {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
}

/// What one prompt turn produced.
#[derive(Debug, Clone, Default)]
pub struct PromptOutcome {
    /// The agent's message text, concatenated from its chunks.
    pub text: String,
    /// The agent's reasoning text, when it streamed any.
    pub thought: String,
    pub tool_calls: Vec<ToolCallReport>,
    /// The agent's stop reason: `end_turn`, `cancelled`, `refusal`, …
    pub stop_reason: String,
}

/// How the client answers the requests an agent may send.
#[derive(Clone, Default)]
pub struct ClientHandlers {
    /// Read a file for the agent. `None` refuses the read.
    pub read_file: Option<Arc<dyn Fn(&str) -> Option<String> + Send + Sync>>,
    /// Write a file for the agent. `false` refuses the write.
    pub write_file: Option<Arc<dyn Fn(&str, &str) -> bool + Send + Sync>>,
    /// Answer a permission request. `true` allows it.
    pub allow_permission: Option<Arc<dyn Fn(&Value) -> bool + Send + Sync>>,
}

impl ClientHandlers {
    /// Handlers that refuse everything — the safe default.
    pub fn deny_all() -> Self {
        Self::default()
    }

    /// Handlers that grant every request, like OpenBot's full-access
    /// agent profile.
    pub fn allow_all() -> Self {
        Self {
            read_file: None,
            write_file: None,
            allow_permission: Some(Arc::new(|_| true)),
        }
    }
}

/// The text and tool calls seen during one turn.
#[derive(Default)]
struct TurnCollector {
    text: String,
    thought: String,
    tool_calls: Vec<ToolCallReport>,
}

type Pending = Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>>;

/// A running ACP agent.
pub struct AcpClient {
    child: Child,
    stdin: Arc<tokio::sync::Mutex<ChildStdin>>,
    /// Replies to the requests we sent, keyed by JSON-RPC id.
    pending: Pending,
    next_id: AtomicI64,
    /// Text the agent streamed during the running turn.
    collected: Arc<Mutex<TurnCollector>>,
    /// Replies to `session/prompt`, which the reader task routes here.
    turn_rx: mpsc::UnboundedReceiver<(i64, Result<Value, String>)>,
    handlers: Arc<Mutex<ClientHandlers>>,
}

impl AcpClient {
    /// Start an agent process and negotiate the protocol.
    ///
    /// `command` is the agent executable; `args` its arguments; `cwd` the
    /// working directory the agent runs in.
    pub async fn start(
        command: &str,
        args: &[String],
        cwd: Option<&PathBuf>,
        handlers: ClientHandlers,
    ) -> Result<Self, AcpError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| AcpError::Spawn(format!("{command}: {e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AcpError::Spawn("no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AcpError::Spawn("no stdout".into()))?;

        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let collected: Arc<Mutex<TurnCollector>> = Arc::new(Mutex::new(TurnCollector::default()));
        let handlers = Arc::new(Mutex::new(handlers));
        let (turn_tx, turn_rx) = mpsc::unbounded_channel();

        let client = Self {
            child,
            stdin: Arc::new(tokio::sync::Mutex::new(stdin)),
            pending: pending.clone(),
            next_id: AtomicI64::new(1),
            collected: collected.clone(),
            turn_rx,
            handlers: handlers.clone(),
        };

        // Reader task: every line is one JSON-RPC message.
        let writer = client.stdin.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let Ok(msg) = serde_json::from_str::<Value>(line) else {
                    continue;
                };
                let reply = handle_message(msg, &pending, &collected, &handlers, &turn_tx);
                if let Some(reply) = reply {
                    let mut stdin = writer.lock().await;
                    let mut text = serde_json::to_string(&reply).unwrap_or_default();
                    text.push('\n');
                    let _ = stdin.write_all(text.as_bytes()).await;
                    let _ = stdin.flush().await;
                }
            }
        });

        Ok(client)
    }

    /// Replace the handlers that answer the agent's requests.
    pub fn set_handlers(&self, handlers: ClientHandlers) {
        *self.handlers.lock().unwrap() = handlers;
    }

    /// The agent's process id, when it is still running.
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Stop the agent.
    pub async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
    }

    fn next_request_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a request and wait for its reply.
    async fn request(&mut self, method: &str, params: Value) -> Result<Value, AcpError> {
        let id = self.next_request_id();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.send(&msg).await?;
        match rx.await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(message)) => Err(AcpError::Agent(message)),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(AcpError::Protocol(format!("no reply for {method}")))
            }
        }
    }

    /// Send a notification (no reply expected).
    async fn notify(&mut self, method: &str, params: Value) -> Result<(), AcpError> {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.send(&msg).await
    }

    async fn send(&mut self, msg: &Value) -> Result<(), AcpError> {
        let mut line = serde_json::to_string(msg)
            .map_err(|e| AcpError::Protocol(format!("encode: {e}")))?;
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| AcpError::Io(format!("write: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| AcpError::Io(format!("flush: {e}")))
    }

    /// Negotiate the protocol version and capabilities.
    pub async fn initialize(&mut self) -> Result<Value, AcpError> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "clientCapabilities": {
                "fs": { "readTextFile": true, "writeTextFile": true },
                "terminal": false,
            },
            "clientInfo": {
                "name": "arkabot",
                "title": "ArkaBot",
                "version": env!("CARGO_PKG_VERSION"),
            }
        });
        let result = self.request("initialize", params).await?;
        let version = result
            .get("protocolVersion")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if version != PROTOCOL_VERSION as u64 {
            return Err(AcpError::Protocol(format!(
                "agent speaks protocol version {version}, client speaks {PROTOCOL_VERSION}"
            )));
        }
        Ok(result)
    }

    /// Open a session rooted at `cwd`.
    pub async fn new_session(&mut self, cwd: &str) -> Result<String, AcpError> {
        let params = json!({
            "cwd": cwd,
            "mcpServers": [],
        });
        let result = self.request("session/new", params).await?;
        result
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AcpError::Protocol("session/new returned no sessionId".into()))
    }

    /// Run one prompt turn and wait for the agent to finish it.
    pub async fn prompt(&mut self, session_id: &str, text: &str) -> Result<PromptOutcome, AcpError> {
        {
            let mut collected = self.collected.lock().unwrap();
            *collected = TurnCollector::default();
        }
        let params = json!({
            "sessionId": session_id,
            "prompt": [{ "type": "text", "text": text }],
        });
        let id = self.next_request_id();
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "session/prompt",
            "params": params,
        });
        self.send(&msg).await?;

        // The agent's reply arrives through the reader task.
        let result = loop {
            match self.turn_rx.recv().await {
                Some((reply_id, outcome)) if reply_id == id => break outcome,
                Some(_) => continue,
                None => return Err(AcpError::Exited("agent closed the connection".into())),
            }
        };
        let stop_reason = match &result {
            Ok(value) => value
                .get("stopReason")
                .and_then(|v| v.as_str())
                .unwrap_or("end_turn")
                .to_string(),
            Err(message) => return Err(AcpError::Agent(message.clone())),
        };
        let collected = self.collected.lock().unwrap();
        Ok(PromptOutcome {
            text: collected.text.clone(),
            thought: collected.thought.clone(),
            tool_calls: collected.tool_calls.clone(),
            stop_reason,
        })
    }

    /// Ask the agent to stop the running turn.
    pub async fn cancel(&mut self, session_id: &str) -> Result<(), AcpError> {
        self.notify("session/cancel", json!({ "sessionId": session_id }))
            .await
    }
}

/// Route one incoming JSON-RPC message.
///
/// Returns the reply the client has to send back, if the agent asked
/// something.
fn handle_message(
    msg: Value,
    pending: &Pending,
    collected: &Arc<Mutex<TurnCollector>>,
    handlers: &Arc<Mutex<ClientHandlers>>,
    turn_results: &mpsc::UnboundedSender<(i64, Result<Value, String>)>,
) -> Option<Value> {
    let id = msg.get("id").and_then(|v| v.as_i64());
    let method = msg.get("method").and_then(|v| v.as_str());

    match (id, method) {
        // A response to something we sent.
        (Some(id), None) => {
            let outcome = if let Some(err) = msg.get("error") {
                Err(err
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error")
                    .to_string())
            } else {
                Ok(msg.get("result").cloned().unwrap_or(Value::Null))
            };
            let mut map = pending.lock().unwrap();
            if let Some(tx) = map.remove(&id) {
                let _ = tx.send(outcome);
            } else {
                // A reply to a turn request the reader task already owns.
                let _ = turn_results.send((id, outcome));
            }
            None
        }
        // A request from the agent: answer it.
        (Some(id), Some(method)) => {
            let params = msg.get("params").cloned().unwrap_or(Value::Null);
            let handlers = handlers.lock().unwrap().clone();
            let result = answer_agent_request(method, &params, &handlers);
            Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
        }
        // A notification from the agent.
        (None, Some(method)) => {
            if method == "session/update" {
                let params = msg.get("params").cloned().unwrap_or(Value::Null);
                let update = params.get("update").cloned().unwrap_or(Value::Null);
                let kind = update
                    .get("sessionUpdate")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let mut collected = collected.lock().unwrap();
                match kind {
                    "agent_message_chunk" => {
                        if let Some(text) = chunk_text(&update) {
                            collected.text.push_str(&text);
                        }
                    }
                    "agent_thought_chunk" => {
                        if let Some(text) = chunk_text(&update) {
                            collected.thought.push_str(&text);
                        }
                    }
                    "tool_call" => {
                        collected.tool_calls.push(ToolCallReport {
                            id: update
                                .get("toolCallId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            title: update
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            kind: update
                                .get("kind")
                                .and_then(|v| v.as_str())
                                .unwrap_or("other")
                                .to_string(),
                            status: update
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("pending")
                                .to_string(),
                        });
                    }
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

/// The text of a content chunk, when it carries one.
fn chunk_text(update: &Value) -> Option<String> {
    let content = update.get("content")?;
    if content.get("type").and_then(|v| v.as_str()) == Some("text") {
        return content
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }
    None
}

/// Answer a request the agent sent to the client.
fn answer_agent_request(
    method: &str,
    params: &Value,
    handlers: &ClientHandlers,
) -> Value {
    match method {
        "session/request_permission" => {
            let allowed = handlers
                .allow_permission
                .as_ref()
                .map(|f| f(params))
                .unwrap_or(false);
            let outcome = if allowed { "selected" } else { "cancelled" };
            json!({ "outcome": { "outcome": outcome } })
        }
        "fs/read_text_file" => {
            let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match handlers.read_file.as_ref().and_then(|f| f(path)) {
                Some(content) => json!({ "content": content }),
                None => json!({ "error": { "code": -32001, "message": "cannot read file" } }),
            }
        }
        "fs/write_text_file" => {
            let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let ok = handlers
                .write_file
                .as_ref()
                .map(|f| f(path, content))
                .unwrap_or(false);
            if ok {
                json!({})
            } else {
                json!({ "error": { "code": -32001, "message": "cannot write file" } })
            }
        }
        _ => json!({ "error": { "code": -32601, "message": format!("unsupported method {method}") } }),
    }
}

/// A known ACP agent the app can drive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpProvider {
    /// Stable id used in the app's settings.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Executable that speaks ACP.
    pub command: String,
    /// Arguments the executable needs.
    pub args: Vec<String>,
    /// How the executable has to be present on the device.
    pub note: String,
}

/// The ACP agents OpenBot supports, with their default commands.
///
/// The command is a starting point: an agent installed somewhere else is
/// selected by giving its own path in the app's settings.
pub fn providers() -> Vec<AcpProvider> {
    vec![
        AcpProvider {
            id: "claude-code".into(),
            name: "Claude Code".into(),
            command: "claude-code-acp".into(),
            args: vec![],
            note: "Adapter untuk Claude Code. Jalankan `claude-code-acp` dari Termux atau proot.".into(),
        },
        AcpProvider {
            id: "codex".into(),
            name: "Codex".into(),
            command: "codex-acp".into(),
            args: vec![],
            note: "Adapter untuk OpenAI Codex.".into(),
        },
        AcpProvider {
            id: "gemini".into(),
            name: "Gemini".into(),
            command: "gemini".into(),
            args: vec!["--experimental-acp".into()],
            note: "Gemini CLI bawaan Google dengan mode ACP.".into(),
        },
        AcpProvider {
            id: "cursor".into(),
            name: "Cursor".into(),
            command: "cursor-agent".into(),
            args: vec!["acp".into()],
            note: "Cursor Agent CLI.".into(),
        },
        AcpProvider {
            id: "cline".into(),
            name: "Cline".into(),
            command: "cline".into(),
            args: vec!["acp".into()],
            note: "Cline CLI.".into(),
        },
        AcpProvider {
            id: "kimi".into(),
            name: "Kimi".into(),
            command: "kimi".into(),
            args: vec!["acp".into()],
            note: "Moonshot Kimi CLI.".into(),
        },
        AcpProvider {
            id: "mistral".into(),
            name: "Mistral Vibe".into(),
            command: "vibe-acp".into(),
            args: vec![],
            note: "ACP server bawaan Mistral Vibe.".into(),
        },
        AcpProvider {
            id: "hermes".into(),
            name: "Hermes".into(),
            command: "hermes".into(),
            args: vec!["acp".into()],
            note: "Hermes Agent milik Nous Portal.".into(),
        },
        AcpProvider {
            id: "opencode".into(),
            name: "OpenCode".into(),
            command: "opencode".into(),
            args: vec!["acp".into()],
            note: "OpenCode CLI, bila versinya mendukung mode ACP.".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_ids_are_unique() {
        let list = providers();
        let mut ids: Vec<&str> = list.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate provider id");
    }

    #[test]
    fn every_provider_has_a_command() {
        for p in providers() {
            assert!(!p.command.is_empty(), "{} has no command", p.id);
        }
    }

    #[test]
    fn chunk_text_reads_text_blocks() {
        let update = json!({ "content": { "type": "text", "text": "hi" } });
        assert_eq!(chunk_text(&update), Some("hi".to_string()));
        let other = json!({ "content": { "type": "image", "data": "..." } });
        assert_eq!(chunk_text(&other), None);
    }

    #[test]
    fn permission_is_granted_when_allowed() {
        let handlers = ClientHandlers::allow_all();
        let reply = answer_agent_request("session/request_permission", &json!({}), &handlers);
        assert_eq!(
            reply.get("outcome").and_then(|o| o.get("outcome")).and_then(|v| v.as_str()),
            Some("selected")
        );
    }

    #[test]
    fn permission_is_denied_by_default() {
        let handlers = ClientHandlers::deny_all();
        let reply = answer_agent_request("session/request_permission", &json!({}), &handlers);
        assert_eq!(
            reply.get("outcome").and_then(|o| o.get("outcome")).and_then(|v| v.as_str()),
            Some("cancelled")
        );
    }
}
