//! Integration test: the ACP client against a fake agent process.

use acp::{AcpClient, ClientHandlers};

/// The fake agent script next to this test file.
fn fake_agent() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fake_agent.py")
}

#[tokio::test]
async fn runs_a_prompt_turn_against_a_real_process() {
    let script = fake_agent();
    if !script.is_file() {
        eprintln!("skipping: {} is missing", script.display());
        return;
    }

    let mut client = AcpClient::start(
        "python3",
        &[script.to_string_lossy().into_owned()],
        None,
        ClientHandlers::allow_all(),
    )
    .await
    .expect("fake agent starts");

    client.initialize().await.expect("initialize succeeds");
    let session = client
        .new_session("/tmp")
        .await
        .expect("session is created");
    assert_eq!(session, "sess_test");

    let outcome = client
        .prompt(&session, "hi there")
        .await
        .expect("prompt succeeds");
    assert_eq!(outcome.text, "Hello from fake agent");
    assert_eq!(outcome.stop_reason, "end_turn");

    client.shutdown().await;
}

#[tokio::test]
async fn reports_a_missing_agent() {
    let err = match AcpClient::start(
        "definitely-not-a-real-command-arkabot",
        &[],
        None,
        ClientHandlers::deny_all(),
    )
    .await
    {
        Ok(_) => panic!("starting a missing command must fail"),
        Err(err) => err,
    };
    assert!(matches!(err, acp::AcpError::Spawn(_)));
}
