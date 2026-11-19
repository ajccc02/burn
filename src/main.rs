use anyhow::{Context, Result};
use clap::Parser;
use spawn_lite::{agent::AgentHandle, config::Cli, player::Player, tools::Tools};
use tokio::sync::broadcast;

#[tokio::main]
async fn main() -> Result<()> {
    match dotenvy::from_path(".env") {
        Ok(_) => (),
        Err(dotenvy::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(_) => anyhow::bail!("could not load .env. Check its syntax and access permissions"),
    }
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "spawn_lite=info".into()),
        )
        .init();

    if let Some(path) = cli.replay {
        let snapshot = spawn_lite::replay::replay_session(&path)?;
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }

    #[cfg(not(feature = "http"))]
    if cli.http {
        anyhow::bail!("WebSocket support requires cargo run --features http -- --http");
    }

    let llm = match cli.llm() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("LLM configuration could not be loaded: {error}");
            None
        }
    };
    if llm.is_none() {
        println!("LLM configuration is missing. Set LLM_BASE_URL and LLM_MODEL or start with --mock-llm.");
    }
    let limits = cli.budget_limits()?;
    limits.validate()?;
    let run_dir = cli.run_dir();
    let (notices, _) = broadcast::channel(128);
    let tools = Tools::start(&run_dir, cli.seed, cli.tick_rate, notices.clone())?;
    let agent = AgentHandle::start(
        tools.clone(),
        llm,
        &run_dir,
        limits,
        cli.agent_settings(),
        notices.clone(),
    )?;
    let metadata = serde_json::json!({
        "version":1,
        "application_version":env!("CARGO_PKG_VERSION"),
        "mock_llm":cli.mock_llm,
        "tick_rate":cli.tick_rate,
        "seed_for_new_world":cli.seed,
        "budget_limits":agent.budget().limits,
        "cooldown_ms":cli.cooldown_ms,
        "max_context_tokens":cli.max_context_tokens,
        "max_output_tokens":cli.max_output_tokens,
        "max_repair_attempts":cli.max_repair_attempts,
    });
    if let Err(error) = spawn_lite::persist::write_json(&run_dir.join("config.json"), &metadata) {
        let _ = agent.shutdown().await;
        let _ = tools.shutdown().await;
        return Err(error).context("saving run configuration");
    }
    let player = Player::new(agent.clone(), tools.clone(), notices);

    #[cfg(feature = "http")]
    let http = if cli.http {
        match spawn_lite::player::start_http(player.clone(), cli.http_addr).await {
            Ok((address, task)) => {
                println!("WebSocket listening on ws://{address}/ws");
                Some(task)
            }
            Err(error) => {
                let _ = agent.shutdown().await;
                let _ = tools.shutdown().await;
                return Err(error).context("starting WebSocket interface");
            }
        }
    } else {
        None
    };

    let terminal = player.terminal().await;
    #[cfg(feature = "http")]
    if let Some(task) = http {
        task.abort();
    }
    let agent_exit = agent.shutdown().await;
    let world_exit = tools.shutdown().await;
    terminal.context("player interface stopped")?;
    agent_exit.context("saving agent state")?;
    world_exit.context("saving world state")?;
    Ok(())
}
