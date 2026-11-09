//! Agent tool definitions and execution.

use super::AgentEngine;
use openbot_core::error::{Error, Result};
use openbot_core::ids::*;
use openbot_core::models::*;
use llm::{ToolCall, ToolDefinition};
use serde_json::{json, Value};

/// Build the tool list for an agent, filtered by its capabilities.
pub fn tool_definitions(agent: &Agent, has_browser: bool) -> Vec<ToolDefinition> {
    let mut tools = vec![
        ToolDefinition {
            kind: "function".into(),
            function: llm::ToolFunctionDef::new(
                "send_message",
                "Kirim pesan langsung ke agent lain. Gunakan ini untuk berkolaborasi.",
                json!({
                    "type": "object",
                    "properties": {
                        "to": { "type": "string", "description": "Nama atau id agent penerima" },
                        "text": { "type": "string", "description": "Pesan teks" }
                    },
                    "required": ["to", "text"]
                }),
            ),
        },
        ToolDefinition {
            kind: "function".into(),
            function: llm::ToolFunctionDef::new(
                "create_task",
                "Tambahkan tugas ke queue agent lain. Tugas akan dijalankan agent tersebut setelah selesai dengan tugas lain.",
                json!({
                    "type": "object",
                    "properties": {
                        "agent": { "type": "string", "description": "Nama atau id agent" },
                        "text": { "type": "string", "description": "Deskripsi tugas" }
                    },
                    "required": ["agent", "text"]
                }),
            ),
        },
        ToolDefinition {
            kind: "function".into(),
            function: llm::ToolFunctionDef::new(
                "read_file",
                " Baca file dari workspace Anda (path relatif, misal notes.md).",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path relatif di workspace" }
                    },
                    "required": ["path"]
                }),
            ),
        },
        ToolDefinition {
            kind: "function".into(),
            function: llm::ToolFunctionDef::new(
                "write_file",
                "Tulis atau perbarui file di workspace Anda.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path relatif di workspace" },
                        "content": { "type": "string", "description": "Konten file" }
                    },
                    "required": ["path", "content"]
                }),
            ),
        },
        ToolDefinition {
            kind: "function".into(),
            function: llm::ToolFunctionDef::new(
                "list_files",
                "List file dan folder di workspace Anda.",
                json!({
                    "type": "object",
                    "properties": {
                        "dir": { "type": "string", "description": "Dir relatif, default root" }
                    }
                }),
            ),
        },
        ToolDefinition {
            kind: "function".into(),
            function: llm::ToolFunctionDef::new(
                "get_queue_status",
                "Cek jumlah tugas yang menunggu di queue Anda.",
                json!({ "type": "object", "properties": {} }),
            ),
        },
    ];
    if agent.has_capability(Capability::Browser) && has_browser {
        tools.extend([
            ToolDefinition {
                kind: "function".into(),
                function: llm::ToolFunctionDef::new(
                    "browser_navigate",
                    "Buka URL di browser terkontrol.",
                    json!({
                        "type": "object",
                        "properties": {
                            "url": { "type": "string" }
                        },
                        "required": ["url"]
                    }),
                ),
            },
            ToolDefinition {
                kind: "function".into(),
                function: llm::ToolFunctionDef::new(
                    "browser_evaluate",
                    "Jalankan JavaScript di halaman browser dan kembalikan hasilnya.",
                    json!({
                        "type": "object",
                        "properties": {
                            "expression": { "type": "string" }
                        },
                        "required": ["expression"]
                    }),
                ),
            },
            ToolDefinition {
                kind: "function".into(),
                function: llm::ToolFunctionDef::new(
                    "browser_screenshot",
                    "Ambil screenshot halaman browser (PNG base64).",
                    json!({ "type": "object", "properties": {} }),
                ),
            },
        ]);
    }
    tools
}

/// Execute one tool call. Returns the tool result as a JSON string.
pub async fn execute_tool(
    engine: &AgentEngine,
    agent: &Agent,
    call: &ToolCall,
) -> Result<String> {
    let name = call.function.name.as_str();
    let args: Value = serde_json::from_str(&call.function.arguments)
        .unwrap_or_else(|_| json!({ "error": "invalid arguments" }));

    let result = match name {
        "send_message" => {
            let to = args.get("to").and_then(|v| v.as_str()).unwrap_or("");
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let recipient = resolve_agent(engine, to)?;
            let messaging = engine.messaging_sender().ok_or_else(|| {
                Error::Tool("messaging service not wired".into())
            })?;
            let item = messaging.send_to_agent(&recipient, Some(&agent.id), text.to_string(), Vec::new())?;
            json!({ "ok": true, "queued_for": item.agent_id.as_str(), "status": item.status }).to_string()
        }
        "create_task" => {
            let agent_ref = args.get("agent").and_then(|v| v.as_str()).unwrap_or("");
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let target = resolve_agent(engine, agent_ref)?;
            let item = engine.enqueue_task(&target, Sender::Agent { agent_id: agent.id.clone() }, text.to_string(), None, Vec::new()).await?;
            json!({ "ok": true, "queue_id": item.id.as_str(), "position": item.position }).to_string()
        }
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = engine.workspace().read(&agent.id, path)?;
            json!({ "ok": true, "path": path, "content": content }).to_string()
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let entry = engine.workspace().write(&agent.id, path, content)?;
            json!({ "ok": true, "path": entry.path }).to_string()
        }
        "list_files" => {
            let dir = args.get("dir").and_then(|v| v.as_str()).unwrap_or("");
            let entries = engine.workspace().list(&agent.id, dir)?;
            let items: Vec<Value> = entries
                .iter()
                .map(|e| json!({ "path": e.path, "dir": e.is_dir, "size": e.size }))
                .collect();
            json!({ "ok": true, "entries": items }).to_string()
        }
        "get_queue_status" => {
            let queued = engine.queue_count(&agent.id)?;
            json!({ "ok": true, "queued": queued }).to_string()
        }
        "browser_navigate" => {
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("about:blank");
            let Some(browser) = engine.browser_control() else {
                return Ok(json!({ "ok": false, "error": "browser unavailable" }).to_string());
            };
            match browser.navigate(url).await {
                Ok(out) => json!({ "ok": true, "output": out }).to_string(),
                Err(e) => json!({ "ok": false, "error": e.to_string() }).to_string(),
            }
        }
        "browser_evaluate" => {
            let expr = args.get("expression").and_then(|v| v.as_str()).unwrap_or("");
            let Some(browser) = engine.browser_control() else {
                return Ok(json!({ "ok": false, "error": "browser unavailable" }).to_string());
            };
            match browser.evaluate(expr).await {
                Ok(out) => json!({ "ok": true, "output": out }).to_string(),
                Err(e) => json!({ "ok": false, "error": e.to_string() }).to_string(),
            }
        }
        "browser_screenshot" => {
            let Some(browser) = engine.browser_control() else {
                return Ok(json!({ "ok": false, "error": "browser unavailable" }).to_string());
            };
            match browser.screenshot().await {
                Ok(out) => json!({ "ok": true, "image": out }).to_string(),
                Err(e) => json!({ "ok": false, "error": e.to_string() }).to_string(),
            }
        }
        other => json!({ "ok": false, "error": format!("unknown tool: {other}") }).to_string(),
    };

    Ok(result)
}

fn resolve_agent(engine: &AgentEngine, name_or_id: &str) -> Result<AgentId> {
    let agents = engine.list_agents_for_tools()?;
    if let Some(a) = agents.iter().find(|a| a.id.as_str() == name_or_id) {
        return Ok(a.id.clone());
    }
    let lower = name_or_id.to_lowercase();
    agents
        .iter()
        .find(|a| a.name.to_lowercase() == lower)
        .map(|a| a.id.clone())
        .ok_or_else(|| Error::Tool(format!("agent tidak ditemukan: {name_or_id}")))
}
