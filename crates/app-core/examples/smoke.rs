fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let dir = std::env::temp_dir().join("openbot-smoke-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let app = app_core::OpenBot::new(dir.to_string_lossy().to_string()).unwrap();
        app.start_routine_scheduler();
        let agents = app.list_agents().unwrap();
        println!("agents: {}", agents.len());
        let health = app.check_provider().await.unwrap();
        println!("provider: {:?}", health);
        let mut input = app_core::AgentInput::default();
        input.name = "Test Agent".into();
        input.instructions = "You are a test agent.".into();
        input.model = "llama3.2".into();
        let agent = app.create_agent(input).unwrap();
        println!("created agent: {} ({})", agent.name, agent.id);
        let channels = app.list_channels().unwrap();
        println!("channels: {}", channels.len());
        let settings = app.get_settings().unwrap();
        println!("settings model: {:?}", settings.model);
        println!("SMOKE OK");
    });
}
