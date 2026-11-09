//! Empirical verification of the findings (tests 2-5 run inside a runtime).
//! Run: cargo run -p app-core --example verify_bugs

use app_core::{AgentInput, OpenBot};
use openbot_core::ids::TurnId;
use openbot_core::models::{BusyMessageMode, Turn, TurnStatus};
use std::sync::Arc;

fn main() {
    let dir = std::env::temp_dir().join("arkabot-bug-verify");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // ---- TEST 1: startup path outside a runtime ----
    println!("== TEST 1: tokio::spawn outside runtime (FFI startup path) ==");
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let app = OpenBot::new(dir.clone()).unwrap();
        app.start_routine_scheduler();
    }));
    println!(
        "RESULT 1: {}",
        if r.is_ok() { "no panic (OK)" } else { "PANIC CONFIRMED (startup crash)" }
    );

    // Fresh app (no scheduler) for the rest.
    let dir2 = std::env::temp_dir().join("arkabot-bug-verify2");
    let _ = std::fs::remove_dir_all(&dir2);
    std::fs::create_dir_all(&dir2).unwrap();
    let app = OpenBot::new(dir2).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    verify_rest(&app, &rt);
}

fn verify_rest(app: &Arc<OpenBot>, rt: &tokio::runtime::Runtime) {
    // ---- TEST 2: duplicate user message ----
    println!("== TEST 2: duplicate user message ==");
    let agents = app.list_agents().unwrap();
    let agent = agents.first().cloned().unwrap();
    let send = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.send_message(&agent.id, "halo duplikat".into(), Vec::new(), None)
    }));
    match send {
        Ok(Ok(_)) => {
            // wait for the spawned turn to finish
            rt.block_on(async {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            });
            let msgs = app.agent_messages(&agent.id, 100).unwrap();
            let matching: Vec<_> = msgs
                .iter()
                .filter(|m| m.text.contains("halo duplikat"))
                .collect();
            for m in &matching {
                println!("   row: sender={:?} text={:?}", m.sender, m.text.chars().take(60).collect::<String>());
            }
            let user_rows = matching
                .iter()
                .filter(|m| matches!(m.sender, openbot_core::models::Sender::User))
                .count();
            println!("RESULT 2: user-sent 'halo duplikat' rows = {user_rows} (1=ok, 2=duplicate confirmed); total matching = {}", matching.len());
        }
        Ok(Err(e)) => println!("RESULT 2: send_message error: {e}"),
        Err(_) => println!("RESULT 2: send_message PANICKED"),
    }

    // ---- TEST 3: busy_mode enum roundtrip ----
    println!("== TEST 3: busy_mode serialization roundtrip ==");
    let mut input = AgentInput::default();
    input.name = "Enum Test".into();
    input.busy_mode = BusyMessageMode::Steer;
    let a2 = app.create_agent(input).unwrap();
    let reloaded = app.agent(&a2.id).unwrap().unwrap();
    println!(
        "RESULT 3: wrote {:?}, read back {:?} (Steer=ok, Queue=bug)",
        BusyMessageMode::Steer, reloaded.busy_mode
    );

    // ---- TEST 4: workspace path traversal ----
    println!("== TEST 4: workspace path traversal ==");
    let ws = app.engine().workspace();
    ws.ensure_root(&a2.id).unwrap();
    let esc = ws.write(&a2.id, "../../ESCAPED.txt", "pwned");
    match esc {
        Ok(_) => {
            let outside = dir_escape_target();
            let exists = std::fs::metadata(&outside).is_ok();
            println!(
                "RESULT 4: traversal write {} (target {})",
                if exists { "CONFIRMED — escaped!" } else { "not visible at expected path" },
                outside.display()
            );
        }
        Err(e) => println!("RESULT 4: blocked: {e} (OK)"),
    }

    // ---- TEST 6: Sender serialization roundtrip ----
    println!("== TEST 6: Sender::Agent serialization roundtrip ==");
    let agent_sender = openbot_core::models::Sender::Agent { agent_id: a2.id.clone() };
    match serde_json::to_string(&agent_sender) {
        Ok(v) => println!("RESULT 6: serialized = {v}"),
        Err(e) => println!("RESULT 6: serialize FAILED: {e}"),
    }
    let stored = serde_json::to_string(&agent_sender).unwrap_or_else(|_| "null".into());
    let back: openbot_core::models::Sender =
        serde_json::from_str(&stored).unwrap_or_default();
    println!("RESULT 6: read back = {back:?} (Agent = ok, User = bug)");

    // ---- TEST 7: ProviderKind serde exactly as the Kotlin SettingsScreen sends it ----
    println!("== TEST 7: ProviderKind serde (strings sent by SettingsScreen.kt) ==");
    for s in ["simulated", "ollama", "lmstudio", "openai-compatible"] {
        let json = format!("\"{s}\"");
        let parsed: Result<openbot_core::models::ProviderKind, _> = serde_json::from_str(&json);
        match parsed {
            Ok(v) => println!("RESULT 7: \"{s}\" -> ok ({:?}), as_str={:?}", v, v.as_str()),
            Err(e) => println!("RESULT 7: \"{s}\" -> PARSE FAILED: {e}"),
        }
    }
    // The exact payload SettingsScreen builds (minus unrelated fields).
    let exact = r#"{"provider":"lmstudio","base_url":"http://10.0.2.2:11434","api_key":"","model":"qwen3","temperature":0.7,"theme":"system","busy_mode":"queue","demo_mode":false}"#;
    match serde_json::from_str::<openbot_core::models::AppSettings>(exact) {
        Ok(_) => println!("RESULT 7: full SettingsScreen payload -> ok"),
        Err(e) => println!("RESULT 7: full SettingsScreen payload -> PARSE FAILED: {e}"),
    }

    // ---- TEST 8: settings save must not block on the network (FFI path) ----
    println!("== TEST 8: FFI update_settings path (sync save + background check) ==");
    let mut blocked_settings = openbot_core::models::AppSettings::default();
    blocked_settings.provider = openbot_core::models::ProviderKind::Ollama;
    blocked_settings.base_url = "http://10.255.255.1:11434".into(); // non-routable: hangs until timeout
    blocked_settings.demo_mode = false;
    let (tx, rx) = std::sync::mpsc::channel();
    {
        // Mirrors the fixed FFI update_settings: sync local save, check spawned.
        let app2 = app.clone();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let handle_settings = blocked_settings.clone();
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let saved = app2.engine().update_settings(handle_settings);
            let elapsed = start.elapsed();
            // Background check, exactly like the FFI spawn (fire and forget).
            let app3 = app2.clone();
            rt.spawn(async move { let _ = app3.check_provider().await; });
            let _ = tx.send(saved.map(|_| elapsed));
        });
    }
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(elapsed)) => println!("RESULT 8: save returned in {elapsed:?} — main thread never blocks (OK)"),
        Ok(Err(e)) => println!("RESULT 8: save error: {e}"),
        Err(_) => println!("RESULT 8: save STILL RUNNING after 5s — main thread would be frozen (bug confirmed)"),
    }

    // ---- TEST 9: exact AgentEditScreen create/update payload ----
    println!("== TEST 9: AgentEditScreen payload (create new agent) ==");
    let edit_payload = r##"{"name":"Agent Baru","avatar":"🤖","avatar_color":"#7AA2F7","instructions":"","model":"","temperature":0.7,"description":"","provider":"simulated","max_tool_iterations":8,"compaction_messages":60,"busy_mode":"queue","capabilities":["chat","files","task_queue","notifications"]}"##;
    match serde_json::from_str::<AgentInput>(edit_payload) {
        Ok(input) => {
            println!("RESULT 9: AgentInput parse ok ({:?})", input.name);
            let created = app.create_agent(input);
            match created {
                Ok(a) => println!("RESULT 9: create_agent ok -> {}", a.id),
                Err(e) => println!("RESULT 9: create_agent FAILED: {e}"),
            }
        }
        Err(e) => println!("RESULT 9: AgentInput PARSE FAILED: {e}"),
    }

    // ---- TEST 5: turn status roundtrip ----
    println!("== TEST 5: TurnStatus roundtrip ==");
    let turn = Turn {
        id: TurnId::new(),
        agent_id: a2.id.clone(),
        status: TurnStatus::Completed,
        started_at: chrono::Utc::now(),
        ended_at: Some(chrono::Utc::now()),
        error: None,
        prompt: "x".into(),
    };
    app.engine().turns().create(&turn).unwrap();
    app.engine().turns().finish(&turn.id, TurnStatus::Completed, None).unwrap();
    let back = app.engine().turns().get(&turn.id).unwrap().unwrap();
    println!(
        "RESULT 5: wrote Completed, read back {:?} (Completed=ok, Running=bug)",
        back.status
    );
}

fn dir_escape_target() -> std::path::PathBuf {
    // workspace root is <dir2>/agents/<id>/workspace; "../../ESCAPED.txt" -> <dir2>/agents/ESCAPED.txt
    std::env::temp_dir().join("arkabot-bug-verify2").join("agents").join("ESCAPED.txt")
}
