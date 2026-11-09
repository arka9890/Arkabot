//! Browser control for agents.
//!
//! Two backends:
//! - `CdpBrowser` — full Chrome DevTools Protocol over WebSocket. Connect to a
//!   Chrome/Chromium started with `--remote-debugging-port=9222`, or to a
//!   WebView debug endpoint exposed by the Android app.
//! - `RemoteBrowser` — the small JSON protocol spoken by the Android app's
//!   WebView bridge (see `android/.../webview-bridge`), so agents can control
//!   the embedded browser in the phone.
//!
//! `NullBrowser` reports every command as unavailable.

use async_trait::async_trait;
use openbot_core::error::{Error, Result};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// What the agent can ask the browser to do.
#[async_trait]
pub trait BrowserControl: Send + Sync {
    async fn navigate(&self, url: &str) -> Result<String>;
    async fn evaluate(&self, expression: &str) -> Result<String>;
    /// PNG screenshot as base64.
    async fn screenshot(&self) -> Result<String>;
    async fn open(&self, url: &str) -> Result<()>;
}

/// No browser available (commands fail with "unavailable").
pub struct NullBrowser;

#[async_trait]
impl BrowserControl for NullBrowser {
    async fn navigate(&self, _url: &str) -> Result<String> {
        Err(Error::Browser("no browser connected".into()))
    }
    async fn evaluate(&self, _expression: &str) -> Result<String> {
        Err(Error::Browser("no browser connected".into()))
    }
    async fn screenshot(&self) -> Result<String> {
        Err(Error::Browser("no browser connected".into()))
    }
    async fn open(&self, _url: &str) -> Result<()> {
        Err(Error::Browser("no browser connected".into()))
    }
}

/// Chrome DevTools Protocol client for one page target.
pub struct CdpBrowser {
    ws: Arc<Mutex<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>>,
    next_id: AtomicU64,
}

impl CdpBrowser {
    /// Connect to a page websocket URL, e.g. `ws://localhost:9222/devtools/page/ABC`.
    pub async fn connect(ws_url: &str) -> Result<Self> {
        let (ws, _) = tokio_tungstenite::connect_async(ws_url)
            .await
            .map_err(|e| Error::Browser(format!("ws connect: {e}")))?;
        Ok(Self {
            ws: Arc::new(Mutex::new(ws)),
            next_id: AtomicU64::new(1),
        })
    }

    /// Find the first page target of a Chrome with remote debugging and
    /// connect to it. `http_base` is like `http://localhost:9222`.
    pub async fn connect_to_remote(http_base: &str) -> Result<Self> {
        let url = format!("{}/json", http_base.trim_end_matches('/'));
        let targets: Vec<Value> = reqwest_like_get(&url).await?;
        let page = targets
            .iter()
            .find(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
            .ok_or_else(|| Error::Browser("no page target found".into()))?;
        let ws_url = page
            .get("webSocketDebuggerUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Browser("no webSocketDebuggerUrl".into()))?;
        Self::connect(ws_url).await
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let msg = json!({ "id": id, "method": method, "params": params });
        let mut ws = self.ws.lock().await;
        ws.send(WsMessage::Text(msg.to_string().into()))
            .await
            .map_err(|e| Error::Browser(format!("send: {e}")))?;
        loop {
            let reply = ws
                .next()
                .await
                .ok_or_else(|| Error::Browser("websocket closed".into()))?
                .map_err(|e| Error::Browser(format!("recv: {e}")))?;
            if let WsMessage::Text(text) = reply {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    if value.get("id").and_then(|v| v.as_u64()) == Some(id) {
                        if let Some(err) = value.get("error") {
                            return Err(Error::Browser(format!("cdp error: {err}")));
                        }
                        return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                    }
                }
            }
        }
    }
}

/// Tiny GET helper without pulling reqwest into the browser crate.
async fn reqwest_like_get(url: &str) -> Result<Vec<Value>> {
    let host = url::Url::parse(url).map_err(|e| Error::Browser(format!("url: {e}")))?;
    let port = host.port_or_known_default().unwrap_or(80);
    let path = host.path().to_string();
    let mut stream = tokio::net::TcpStream::connect((host.host_str().unwrap_or("127.0.0.1"), port))
        .await
        .map_err(|e| Error::Browser(format!("tcp: {e}")))?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        host.host_str().unwrap_or("localhost")
    );
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream.write_all(request.as_bytes()).await.map_err(|e| Error::Browser(format!("write: {e}")))?;
    let mut body = Vec::new();
    stream.read_to_end(&mut body).await.map_err(|e| Error::Browser(format!("read: {e}")))?;
    let body = String::from_utf8_lossy(&body);
    let start = body.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    serde_json::from_str(&body[start..]).map_err(|e| Error::Browser(format!("json: {e}")))
}

#[async_trait]
impl BrowserControl for CdpBrowser {
    async fn navigate(&self, url: &str) -> Result<String> {
        let result = self.call("Page.navigate", json!({ "url": url })).await?;
        Ok(result.to_string())
    }

    async fn evaluate(&self, expression: &str) -> Result<String> {
        let result = self
            .call("Runtime.evaluate", json!({ "expression": expression, "returnByValue": true }))
            .await?;
        if let Some(value) = result.get("result").and_then(|r| r.get("value")) {
            Ok(value.to_string())
        } else {
            Ok(result.to_string())
        }
    }

    async fn screenshot(&self) -> Result<String> {
        let result = self.call("Page.captureScreenshot", json!({})).await?;
        result
            .get("data")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Browser("screenshot failed".into()))
    }

    async fn open(&self, url: &str) -> Result<()> {
        self.navigate(url).await.map(|_| ())
    }
}

// ---------------------------------------------------------------------------
// Remote protocol (Android WebView bridge)
// ---------------------------------------------------------------------------

/// Connect to the Android app's WebView bridge: a WebSocket speaking the JSON
/// protocol `{"id":n,"method":...,"params":{...}}` with
/// `{"id":n,"ok":bool,"output":"..."}` replies.
pub struct RemoteBrowser {
    ws: Arc<Mutex<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>>,
    next_id: AtomicU64,
}

impl RemoteBrowser {
    pub async fn connect(ws_url: &str) -> Result<Self> {
        let (ws, _) = tokio_tungstenite::connect_async(ws_url)
            .await
            .map_err(|e| Error::Browser(format!("ws connect: {e}")))?;
        Ok(Self {
            ws: Arc::new(Mutex::new(ws)),
            next_id: AtomicU64::new(1),
        })
    }

    async fn call(&self, method: &str, params: Value) -> Result<String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let msg = json!({ "id": id, "method": method, "params": params });
        let mut ws = self.ws.lock().await;
        ws.send(WsMessage::Text(msg.to_string().into()))
            .await
            .map_err(|e| Error::Browser(format!("send: {e}")))?;
        loop {
            let reply = ws
                .next()
                .await
                .ok_or_else(|| Error::Browser("websocket closed".into()))?
                .map_err(|e| Error::Browser(format!("recv: {e}")))?;
            if let WsMessage::Text(text) = reply {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    if value.get("id").and_then(|v| v.as_u64()) == Some(id) {
                        let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                        if !ok {
                            let err = value
                                .get("error")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown error");
                            return Err(Error::Browser(err.to_string()));
                        }
                        return Ok(value
                            .get("output")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string());
                    }
                }
            }
        }
    }
}

#[async_trait]
impl BrowserControl for RemoteBrowser {
    async fn navigate(&self, url: &str) -> Result<String> {
        self.call("navigate", json!({ "url": url })).await
    }

    async fn evaluate(&self, expression: &str) -> Result<String> {
        self.call("evaluate", json!({ "expression": expression })).await
    }

    async fn screenshot(&self) -> Result<String> {
        self.call("screenshot", json!({})).await
    }

    async fn open(&self, url: &str) -> Result<()> {
        self.call("open", json!({ "url": url })).await?;
        Ok(())
    }
}
