//! OpenAI-compatible LLM provider client.
//!
//! Any endpoint that implements `/v1/chat/completions` works: OpenAI, Azure,
//! Groq, Together, Ollama (`/v1`), LM Studio (`/v1`), and custom OpenAI-
//! compatible servers. A built-in simulated provider lets the app run without
//! a model server for demos and tests.

use openbot_core::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Chat types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    /// JSON string of arguments.
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    /// Set for assistant messages that request a tool call.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Set for tool role messages.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// Approximate token count (~4 chars/token).
    pub fn estimate_tokens(&self) -> usize {
        self.content.len() / 4 + 1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolFunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionDef {
    pub name: String,
    pub description: String,
    /// JSON Schema object describing the parameters.
    pub parameters: Value,
}

impl ToolFunctionDef {
    pub fn new(name: &str, description: &str, parameters: Value) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        }
    }

    pub fn object(parameters: Value) -> Self {
        Self::new("", "", parameters)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: String,
    pub usage: Option<Usage>,
}

// ---------------------------------------------------------------------------
// Providers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: Option<u32>,
    /// The ACP agent executable. Empty means this is not an ACP provider.
    pub acp_command: String,
    /// Arguments for the ACP agent executable.
    pub acp_args: Vec<String>,
    /// Working directory the ACP agent runs in.
    pub acp_cwd: String,
}

impl ProviderConfig {
    pub fn demo() -> Self {
        Self {
            base_url: "simulated".to_string(),
            api_key: String::new(),
            model: "simulated".to_string(),
            temperature: 0.7,
            max_tokens: None,
            ..Default::default()
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    /// True when this config points at a local ACP agent.
    pub fn is_acp(&self) -> bool {
        !self.acp_command.trim().is_empty()
    }
}

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse>;

    /// Cheap liveness check used by settings > provider.
    async fn check(&self) -> Result<ProviderStatus> {
        self.chat(&ChatRequest {
            model: self.model(),
            messages: vec![ChatMessage::user("ping")],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
        })
        .await
        .map(|_| ProviderStatus {
            state: "available".into(),
            message: None,
            model: Some(self.model()),
            base_url: Some(self.base_url()),
        })
        .map_err(|e| {
            Error::Provider(format!("provider check failed: {e}"))
        })
    }

    fn model(&self) -> String;
    fn base_url(&self) -> String;
}

#[derive(Debug, Clone)]
pub struct ProviderStatus {
    pub state: String,
    pub message: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

/// Real OpenAI-compatible HTTP client.
pub struct OpenAiClient {
    http: reqwest::Client,
    config: ProviderConfig,
}

impl OpenAiClient {
    pub fn new(config: ProviderConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap_or_default();
        Self { http, config }
    }
}

#[async_trait::async_trait]
impl LlmClient for OpenAiClient {
    fn model(&self) -> String {
        self.config.model.clone()
    }

    fn base_url(&self) -> String {
        self.config.base_url.clone()
    }

    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse> {
        let body = serde_json::json!({
            "model": req.model,
            "messages": req.messages,
            "temperature": req.temperature.unwrap_or(self.config.temperature),
            "max_tokens": req.max_tokens.or(self.config.max_tokens),
            "tools": req.tools,
        });

        let mut http_req = self
            .http
            .post(self.config.endpoint())
            .json(&body);
        if !self.config.api_key.is_empty() {
            http_req = http_req.bearer_auth(&self.config.api_key);
        }

        let resp = http_req.send().await.map_err(|e| {
            Error::Provider(format!("request failed: {e}"))
        })?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| Error::Provider(format!("body: {e}")))?;
        if !status.is_success() {
            return Err(Error::Provider(format!(
                "HTTP {}: {}",
                status.as_u16(),
                text.chars().take(500).collect::<String>()
            )));
        }

        let value: Value = serde_json::from_str(&text)
            .map_err(|e| Error::Llm(format!("invalid response: {e}")))?;
        parse_chat_response(&value)
    }
}

fn parse_chat_response(value: &Value) -> Result<ChatResponse> {
    let choice = value
        .get("choices")
        .and_then(|c| c.get(0))
        .ok_or_else(|| Error::Llm("missing choices in response".into()))?;
    let message = choice.get("message").ok_or_else(|| Error::Llm("missing message".into()))?;

    let content = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let tool_calls = message
        .get("tool_calls")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|tc| serde_json::from_value::<ToolCall>(tc.clone()).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let finish_reason = choice
        .get("finish_reason")
        .and_then(|f| f.as_str())
        .unwrap_or("stop")
        .to_string();
    let usage = value.get("usage").and_then(|u| serde_json::from_value(u.clone()).ok());

    Ok(ChatResponse {
        content,
        tool_calls,
        finish_reason,
        usage,
    })
}

/// Simulated provider for demos and tests without a model server.
///
/// It answers with canned text and, on request, calls one tool so the full
/// agent loop (queue, tool execution, persistence) can be exercised offline.
pub struct SimulatedClient {
    calls: std::sync::atomic::AtomicU32,
}

impl SimulatedClient {
    pub fn new() -> Self {
        Self {
            calls: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

impl Default for SimulatedClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LlmClient for SimulatedClient {
    fn model(&self) -> String {
        "simulated".into()
    }

    fn base_url(&self) -> String {
        "simulated".into()
    }

    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse> {
        let idx = self.calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // If the request asks for the weather, simulate a tool call.
        let user_text = req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.to_lowercase())
            .unwrap_or_default();

        if user_text.contains("weather") {
            return Ok(ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: format!("call_{idx}"),
                    kind: "function".into(),
                    function: ToolCallFunction {
                        name: "get_weather".into(),
                        arguments: serde_json::json!({ "city": "Jakarta" }).to_string(),
                    },
                }],
                finish_reason: "tool_calls".into(),
                usage: None,
            });
        }

        let reply = format!(
            "(simulated) Halo! Saya adalah agent demo. Pesan terakhir: \"{}\". \
             Configure provider OpenAI-compatible yang sebenarnya di Settings agar saya bisa menjawab dengan model sungguhan.",
            user_text.chars().take(120).collect::<String>()
        );
        Ok(ChatResponse {
            content: reply,
            tool_calls: Vec::new(),
            finish_reason: "stop".into(),
            usage: None,
        })
    }
}

// ---------------------------------------------------------------------------
// ACP provider
// ---------------------------------------------------------------------------

/// Drives a local agent CLI that speaks the Agent Client Protocol.
///
/// The CLI keeps the conversation; the app sends the user's message and
/// receives the agent's reply. The agent runs its own tools, so this client
/// never returns tool calls to the local agent loop.
pub struct AcpLlmClient {
    command: String,
    args: Vec<String>,
    cwd: Option<std::path::PathBuf>,
    /// The running agent and its session, started on the first turn.
    state: tokio::sync::Mutex<Option<AcpSession>>,
}

/// A started ACP agent and the session open on it.
struct AcpSession {
    client: acp::AcpClient,
    session_id: String,
}

impl AcpLlmClient {
    pub fn new(config: &ProviderConfig) -> Self {
        Self {
            command: config.acp_command.clone(),
            args: config.acp_args.clone(),
            cwd: if config.acp_cwd.trim().is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(&config.acp_cwd))
            },
            state: tokio::sync::Mutex::new(None),
        }
    }

    /// The last user message of a request.
    fn last_user_message(req: &ChatRequest) -> String {
        req.messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl LlmClient for AcpLlmClient {
    fn model(&self) -> String {
        self.command.clone()
    }

    fn base_url(&self) -> String {
        "acp".into()
    }

    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse> {
        let text = Self::last_user_message(req);
        let mut state = self.state.lock().await;
        if state.is_none() {
            let mut client = acp::AcpClient::start(
                &self.command,
                &self.args,
                self.cwd.as_ref(),
                acp::ClientHandlers::allow_all(),
            )
            .await
            .map_err(|e| Error::Provider(format!("acp: {e}")))?;
            client
                .initialize()
                .await
                .map_err(|e| Error::Provider(format!("acp initialize: {e}")))?;
            let cwd = self
                .cwd
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "/".to_string());
            let session_id = client
                .new_session(&cwd)
                .await
                .map_err(|e| Error::Provider(format!("acp session: {e}")))?;
            *state = Some(AcpSession { client, session_id });
        }
        let session = state.as_mut().expect("session just created");
        let outcome = session
            .client
            .prompt(&session.session_id, &text)
            .await
            .map_err(|e| Error::Provider(format!("acp prompt: {e}")))?;
        let content = if outcome.text.is_empty() {
            outcome.thought
        } else {
            outcome.text
        };
        Ok(ChatResponse {
            content,
            tool_calls: Vec::new(),
            finish_reason: outcome.stop_reason,
            usage: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Client factory
// ---------------------------------------------------------------------------

/// Maps a provider config to a concrete client.
pub fn build_client(config: &ProviderConfig) -> Box<dyn LlmClient> {
    if config.is_acp() {
        Box::new(AcpLlmClient::new(config))
    } else if config.base_url == "simulated" || config.base_url.is_empty() {
        Box::new(SimulatedClient::new())
    } else {
        Box::new(OpenAiClient::new(config.clone()))
    }
}

/// Rough token estimate for text (used for compaction thresholds).
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() / 4).max(1)
}

/// Known Ollama / LM Studio defaults for the settings form.
pub fn default_presets() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("Ollama (localhost)", "http://10.0.2.2:11434/v1", "llama3.2"),
        ("LM Studio (localhost)", "http://10.0.2.2:1234/v1", "local-model"),
        ("OpenAI", "https://api.openai.com/v1", "gpt-4o-mini"),
        ("Groq", "https://api.groq.com/openai/v1", "llama-3.3-70b-versatile"),
        ("OpenRouter", "https://openrouter.ai/api/v1", "anthropic/claude-sonnet-4.5"),
    ]
}

/// Keep a typed alias for tool dispatch helpers.
pub type ToolRegistry = HashMap<String, ToolDefinition>;
