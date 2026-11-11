//! Integration test: the HTTP browser bridge transport.

use browser::{BrowserControl, HttpBrowser};
use std::process::{Child, Command, Stdio};

/// A bridge stand-in that answers on `port`.
struct FakeBridge {
    child: Child,
    port: u16,
}

impl FakeBridge {
    fn start(tag: &str) -> Option<Self> {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fake_bridge.py");
        if !script.is_file() {
            eprintln!("skipping: {} is missing", script.display());
            return None;
        }
        // Pick a free port by asking the OS for one, then release it.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .ok()?
            .local_addr()
            .ok()?
            .port();
        let child = Command::new("python3")
            .arg(&script)
            .arg(port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        // Wait until the server accepts connections instead of guessing.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Some(Self { child, port })
    }
}

impl Drop for FakeBridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
async fn drives_a_bridge_over_http() {
    let Some(bridge) = FakeBridge::start("http") else {
        return;
    };
    let browser = HttpBrowser::new(&format!("http://127.0.0.1:{}", bridge.port));

    let out = browser.navigate("https://example.com").await.unwrap();
    assert_eq!(out, "loaded https://example.com");

    let out = browser.evaluate("1+1").await.unwrap();
    assert_eq!(out, "42");

    let out = browser.screenshot().await.unwrap();
    assert_eq!(out, "iVBORw0KGgo=");

    browser.open("https://example.com").await.unwrap();
}
