use anyhow::Result;
use clap::Parser;
use serde_json::json;
use spawn_lite::{
    persist,
    replay::replay_session,
    tools::Tools,
    world::{World, WorldState},
};
use std::fs;
use tokio::sync::broadcast;

#[test]
fn cli_retains_saved_budget_caps_unless_explicitly_changed() -> Result<()> {
    use spawn_lite::{
        budget::{Budget, BudgetLimits},
        config::Cli,
    };
    let dir = tempfile::tempdir()?;
    let run = dir.path().join("bounded");
    fs::create_dir(&run)?;
    let saved = BudgetLimits {
        max_tokens: Some(100),
        max_iterations: Some(2),
        max_cost_usd: Some(0.1),
        input_cost_per_1m: Some(1.0),
        output_cost_per_1m: Some(1.0),
    };
    let mut budget = Budget::open(&run, saved)?;
    budget.reserve(80, 20)?;
    budget.settle(None)?;
    drop(budget);
    let root = dir.path().to_str().unwrap();
    let cli = Cli::try_parse_from([
        "spawn-lite",
        "--mock-llm",
        "--saves-dir",
        root,
        "--run-id",
        "bounded",
    ])?;
    let limits = cli.budget_limits()?;
    assert_eq!(limits.max_tokens, Some(100));
    assert_eq!(limits.max_iterations, Some(2));
    assert_eq!(limits.max_cost_usd, Some(0.1));
    assert!(Budget::open(&run, limits)?.snapshot().exhausted);

    let cli = Cli::try_parse_from([
        "spawn-lite",
        "--mock-llm",
        "--saves-dir",
        root,
        "--run-id",
        "bounded",
        "--max-tokens",
        "200",
    ])?;
    let budget = Budget::open(&run, cli.budget_limits()?)?;
    assert_eq!(budget.snapshot().total_tokens, 100);
    assert_eq!(budget.snapshot().limits.max_tokens, Some(200));
    assert!(!budget.snapshot().exhausted);
    Ok(())
}

#[tokio::test]
async fn stale_repair_cannot_recreate_a_deleted_mod() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let (notices, _) = broadcast::channel(32);
    let tools = Tools::start(dir.path(), 42, 10, notices)?;
    assert_eq!(
        tools
            .execute("write_mod", json!({"name":"clock", "source":WORKING}))
            .await["ok"],
        true
    );
    assert_eq!(
        tools.execute("delete_mod", json!({"name":"clock"})).await["ok"],
        true
    );
    let stale = tools
        .execute_repair(
            "write_mod",
            json!({"name":"clock", "source":WORKING}),
            "clock",
        )
        .await;
    assert_eq!(stale["ok"], false, "{stale}");
    assert_eq!(stale["error"]["code"], "REPAIR_TARGET_REMOVED");
    assert_eq!(
        tools.execute("list_mods", json!({})).await["data"],
        json!([])
    );
    assert!(!dir.path().join("mods/clock.rhai").exists());
    tools.shutdown().await?;
    Ok(())
}

const WORKING: &str = r#"
    fn tick_clock() {
        let clock = query("clock")[0];
        set(clock.id, "ticks", clock.props.ticks + 1);
        emit("chime", #{ticks: clock.props.ticks + 1});
    }
    fn init() {
        spawn("clock", #{ticks: 0});
        on("tick", "tick_clock");
    }
"#;

#[tokio::test]
async fn invalid_manual_edit_preserves_working_source_after_restart() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let (notices, _) = broadcast::channel(32);
    let tools = Tools::start(dir.path(), 42, 10, notices.clone())?;
    let accepted = tools
        .execute("write_mod", json!({"name":"clock", "source":WORKING}))
        .await;
    assert_eq!(accepted["ok"], true, "{accepted}");
    let file = dir.path().join("mods/clock.rhai");
    let invalid = "fn init( {";
    fs::write(&file, invalid)?;
    let reload = tools.execute("reload_mods", json!({})).await;
    assert_eq!(
        reload["data"]["clock.rhai"]["status"], "rejected",
        "{reload}"
    );
    tools.shutdown().await?;

    let tools = Tools::start(dir.path(), 99, 10, notices)?;
    let source = tools.execute("read_mod", json!({"name":"clock"})).await;
    assert_eq!(source["data"]["source"], WORKING);
    assert_eq!(source["data"]["candidate"], invalid);
    let state = tools.execute("run_ticks", json!({"n": 4})).await;
    assert_eq!(state["ok"], true, "{state}");
    assert!(
        state["data"]["entities"][0]["props"]["ticks"]
            .as_u64()
            .unwrap()
            >= 4
    );
    assert!(state["data"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["name"] == "chime"));
    tools.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn recorded_session_reproduces_saved_entities_mods_and_events() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let (notices, _) = broadcast::channel(32);
    let tools = Tools::start(dir.path(), 42, 10, notices)?;
    let unknown = tools
        .execute("run_ticks", json!({"n":5,"unexpected":true}))
        .await;
    assert_eq!(unknown["ok"], false, "{unknown}");
    let traversal = tools
        .execute("write_mod", json!({"name":"../clock","source":WORKING}))
        .await;
    assert_eq!(traversal["ok"], false, "{traversal}");
    let plant = tools
        .execute(
            "spawn_entity",
            json!({"kind":"fern","props_json":{"size":4}}),
        )
        .await;
    assert_eq!(plant["data"]["id"], 1, "{plant}");
    assert_eq!(
        tools
            .execute("write_mod", json!({"name":"clock","source":WORKING}))
            .await["ok"],
        true
    );
    assert_eq!(
        tools.execute("run_ticks", json!({"n":120})).await["ok"],
        true
    );
    tools.shutdown().await?;

    let replay_dir = fs::read_dir(dir.path().join("replay"))?
        .next()
        .unwrap()?
        .path();
    let state = persist::read_json::<WorldState>(&dir.path().join("world.json"))?.unwrap();
    let expected = World::restore(state)?.snapshot();
    assert_eq!(replay_session(&replay_dir)?, expected);
    assert_eq!(replay_session(&replay_dir)?, expected);
    Ok(())
}

#[test]
fn corrupt_world_snapshot_is_preserved_and_startup_fails() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let world = dir.path().join("world.json");
    fs::write(&world, "broken json")?;
    let (notices, _) = broadcast::channel(32);
    assert!(Tools::start(dir.path(), 42, 10, notices).is_err());
    assert_eq!(fs::read_to_string(world)?, "broken json");
    Ok(())
}
