use serde_json::{json, Value};
use spawn_lite::{
    agent::{AgentHandle, AgentSettings, AgentStatus},
    budget::BudgetLimits,
    llm::{
        LlmClient, LlmError, LlmFuture, LlmReply, LlmRequest, MockLlm, ScriptedLlm, TokenUsage,
        ToolCall,
    },
    tools::Tools,
};
use std::{fs, sync::Arc, time::Duration};
use tokio::{
    sync::{broadcast, Notify},
    time::{sleep, timeout},
};

fn settings() -> AgentSettings {
    AgentSettings {
        cooldown: Duration::from_millis(4),
        ..AgentSettings::default()
    }
}

async fn notice(rx: &mut broadcast::Receiver<String>, needle: &str) -> anyhow::Result<String> {
    Ok(timeout(Duration::from_secs(10), async {
        loop {
            let next = rx.recv().await?;
            if next.contains(needle) {
                return Ok::<_, broadcast::error::RecvError>(next);
            }
        }
    })
    .await??)
}

fn called(name: &str, arguments: Value) -> LlmReply {
    LlmReply {
        calls: vec![ToolCall {
            name: name.to_owned(),
            arguments,
        }],
        usage: Some(TokenUsage {
            input: 20,
            output: 5,
        }),
        text: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mock_creates_trading_merchant_repairs_error_and_restores_world_and_budget(
) -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let (send, mut rx) = broadcast::channel(64);
    let tools = Tools::start(dir.path(), 42, 10, send.clone())?;
    let agent = AgentHandle::start(
        tools.clone(),
        Some(Arc::new(MockLlm)),
        dir.path(),
        BudgetLimits::default(),
        settings(),
        send.clone(),
    )?;
    let task = agent
        .submit("Add a wandering merchant that trades with nearby characters.".to_owned())
        .await?;
    notice(&mut rx, &format!("Task #{task} verified.")).await?;
    let durable: Value = serde_json::from_str(&fs::read_to_string(dir.path().join("world.json"))?)?;
    assert!(durable["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entity| {
            entity["kind"] == "merchant" && entity["props"]["trades"].as_u64().unwrap_or(0) > 0
        }));
    assert!(durable["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["name"] == "trade"));
    let observed = tools.observation().await?;
    let merchant = observed["world"]["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entity| entity["kind"] == "merchant")
        .unwrap();
    let merchant_id = merchant["id"].as_u64().unwrap();
    assert!(merchant["props"]["trades"].as_u64().unwrap() > 0);
    assert!(observed["world"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["name"] == "trade"));
    assert!(dir.path().join("mods/merchant.rhai").exists());
    let before_step = tools.execute("run_ticks", json!({"n":1})).await;
    let after_step = tools.execute("run_ticks", json!({"n":1})).await;
    let position = |result: &Value| {
        result["data"]["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entity| entity["id"] == merchant_id)
            .unwrap()["props"]["x"]
            .as_i64()
            .unwrap()
    };
    assert_ne!(position(&before_step), position(&after_step));

    let broken = "on(\"tick\", \"merchant_tick\"); fn merchant_tick() { missing_operation(); }";
    let result = tools
        .execute("write_mod", json!({"name":"merchant","source":broken}))
        .await;
    assert_eq!(result["ok"], true, "{result}");
    tools.execute("run_ticks", json!({"n":4})).await;
    let diagnostics = tools.observation().await?;
    assert!(diagnostics["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["mod_name"] == "merchant"
            && entry["category"] == "runtime"
            && entry["resolved"] == false));
    notice(&mut rx, "queued to repair merchant").await?;
    let verification = notice(&mut rx, "I repaired the merchant mod").await?;
    assert!(verification.contains("verified"));
    let repaired = tools.observation().await?;
    assert!(repaired["errors"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["mod_name"] == "merchant")
        .all(|entry| entry["resolved"] == true));
    assert!(repaired["world"]["mods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["name"] == "merchant" && entry["active"] == true));
    agent.shutdown().await?;
    tools.shutdown().await?;
    let usage = agent.budget();

    let tools = Tools::start(dir.path(), 999, 10, send.clone())?;
    let restarted = AgentHandle::start(
        tools.clone(),
        Some(Arc::new(MockLlm)),
        dir.path(),
        BudgetLimits::default(),
        settings(),
        send,
    )?;
    let restored = tools.observation().await?;
    let merchants: Vec<_> = restored["world"]["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entity| entity["kind"] == "merchant")
        .collect();
    assert_eq!(merchants.len(), 1);
    assert_eq!(merchants[0]["id"], merchant_id);
    assert!(
        merchants[0]["props"]["trades"].as_u64().unwrap()
            >= merchant["props"]["trades"].as_u64().unwrap()
    );
    assert_eq!(restarted.budget().total_tokens, usage.total_tokens);
    assert_eq!(restarted.budget().requests, usage.requests);
    assert_eq!(restarted.status().pending, 0);
    restarted.shutdown().await?;
    tools.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn volcano_recurs_at_sixty_ticks_across_restart() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let (send, mut rx) = broadcast::channel(64);
    let tools = Tools::start(dir.path(), 2, 10, send.clone())?;
    let agent = AgentHandle::start(
        tools.clone(),
        Some(Arc::new(MockLlm)),
        dir.path(),
        BudgetLimits::default(),
        settings(),
        send.clone(),
    )?;
    let task = agent
        .submit("Create a volcano that erupts every 60 ticks.".to_owned())
        .await?;
    notice(&mut rx, &format!("Task #{task} verified.")).await?;
    agent.shutdown().await?;
    tools.shutdown().await?;
    let restored = Tools::start(dir.path(), 2, 10, send)?;
    let result = restored.execute("run_ticks", json!({"n":120})).await;
    assert_eq!(result["ok"], true, "{result}");
    let ticks: Vec<_> = result["data"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["name"] == "eruption")
        .map(|event| event["tick"].as_u64().unwrap())
        .collect();
    assert!(ticks.len() >= 4, "{ticks:?}");
    assert!(
        ticks.windows(2).all(|pair| pair[1] - pair[0] == 60),
        "{ticks:?}"
    );
    assert!(restored.observation().await?["logs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["message"]
            .as_str()
            .is_some_and(|message| message.to_ascii_lowercase().contains("erupt"))));
    restored.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_and_transient_replies_recover_without_unverified_messages() -> anyhow::Result<()>
{
    let dir = tempfile::tempdir()?;
    let (send, mut rx) = broadcast::channel(64);
    let tools = Tools::start(dir.path(), 0, 10, send.clone())?;
    let client = ScriptedLlm::new([
        Err(LlmError::temporary("transient test timeout")),
        Ok(LlmReply::default()),
        Ok(called(
            "spawn_entity",
            json!({"kind":"test_item","props_json":{"name":"Pear"}}),
        )),
        Ok(called(
            "send_message_to_player",
            json!({"text":"FORGED COMPLETION"}),
        )),
        Ok(called(
            "verify_task",
            json!({"checks":[{"kind":"entity","entity_kind":"test_item","min_count":1}],"message":"TOO EARLY"}),
        )),
        Ok(called("run_ticks", json!({"n":2}))),
        Ok(called(
            "verify_task",
            json!({"checks":[{"kind":"entity","entity_kind":"test_item","min_count":1,"props":{"name":"Pear"}}],"message":"Verified a pear."}),
        )),
    ]);
    let agent = AgentHandle::start(
        tools.clone(),
        Some(Arc::new(client)),
        dir.path(),
        BudgetLimits::default(),
        settings(),
        send,
    )?;
    agent
        .submit("Create a test item named Pear.".to_owned())
        .await?;
    timeout(Duration::from_secs(10), async {
        loop {
            let message = rx.recv().await?;
            assert!(!message.contains("FORGED COMPLETION"));
            assert!(!message.contains("TOO EARLY"));
            if message.contains("Verified a pear.") {
                return Ok::<_, broadcast::error::RecvError>(());
            }
        }
    })
    .await??;
    agent.shutdown().await?;
    let text = fs::read_to_string(dir.path().join("agent/journal.jsonl"))?;
    assert!(text.contains("VERIFICATION_REQUIRED"));
    assert!(text.contains("TICKS_REQUIRED"));
    assert_eq!(
        text.lines()
            .filter(|line| line.contains("model_error"))
            .count(),
        2
    );
    assert!(tools.observation().await?["world"]["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entity| entity["props"]["name"] == "Pear"));
    tools.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exhausted_agent_leaves_world_running_and_cannot_resume_after_restart() -> anyhow::Result<()>
{
    let dir = tempfile::tempdir()?;
    let (send, mut rx) = broadcast::channel(64);
    let tools = Tools::start(dir.path(), 3, 10, send.clone())?;
    let limits = BudgetLimits {
        max_iterations: Some(1),
        ..BudgetLimits::default()
    };
    let agent = AgentHandle::start(
        tools.clone(),
        Some(Arc::new(MockLlm)),
        dir.path(),
        limits.clone(),
        settings(),
        send.clone(),
    )?;
    agent.submit("Add a merchant.".to_owned()).await?;
    notice(&mut rx, "cannot cover another request").await?;
    assert_eq!(agent.status().status, AgentStatus::PausedBudget);
    assert!(agent.resume().await.is_err());
    let first = tools.observation().await?["world"]["tick"]
        .as_u64()
        .unwrap();
    sleep(Duration::from_millis(220)).await;
    assert!(
        tools.observation().await?["world"]["tick"]
            .as_u64()
            .unwrap()
            > first
    );
    agent.shutdown().await?;
    tools.shutdown().await?;

    let tools = Tools::start(dir.path(), 3, 10, send.clone())?;
    let agent = AgentHandle::start(
        tools.clone(),
        Some(Arc::new(MockLlm)),
        dir.path(),
        limits,
        settings(),
        send,
    )?;
    assert_eq!(agent.status().status, AgentStatus::PausedBudget);
    assert_eq!(agent.status().pending, 1);
    assert_eq!(agent.budget().requests, 1);
    assert!(agent.resume().await.is_err());
    agent.shutdown().await?;
    tools.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stops_repairing_after_bounded_failed_attempts() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let (send, mut rx) = broadcast::channel(64);
    let tools = Tools::start(dir.path(), 4, 10, send.clone())?;
    tools
        .execute(
            "write_mod",
            json!({"name":"broken","source":"on(\"tick\",\"broken\"); fn broken() { unknown(); }"}),
        )
        .await;
    tools.execute("run_ticks", json!({"n":4})).await;
    let client = ScriptedLlm::new([
        Ok(called("read_mod", json!({"name":"broken"}))),
        Ok(called(
            "write_mod",
            json!({"name":"broken","source":"let ;"}),
        )),
        Ok(called(
            "write_mod",
            json!({"name":"broken","source":"fn {"}),
        )),
        Ok(called(
            "write_mod",
            json!({"name":"broken","source":"fn ready() {}"}),
        )),
    ]);
    let agent = AgentHandle::start(
        tools.clone(),
        Some(Arc::new(client)),
        dir.path(),
        BudgetLimits::default(),
        AgentSettings {
            max_repair_attempts: 2,
            ..settings()
        },
        send,
    )?;
    notice(&mut rx, "Repair attempts reached the configured limit").await?;
    sleep(Duration::from_millis(550)).await;
    assert_eq!(agent.status().pending, 0);
    assert!(tools.observation().await?["world"]["mods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["name"] == "broken" && entry["active"] == false));
    agent.shutdown().await?;
    let journal = fs::read_to_string(dir.path().join("agent/journal.jsonl"))?;
    assert_eq!(
        journal
            .lines()
            .filter(|line| line.contains("tool_intent") && line.contains("write_mod"))
            .count(),
        2
    );
    tools.shutdown().await?;
    Ok(())
}

struct DelayedRepair {
    requested: Arc<Notify>,
    release: Arc<Notify>,
}

impl LlmClient for DelayedRepair {
    fn complete<'a>(&'a self, _request: &'a LlmRequest) -> LlmFuture<'a> {
        Box::pin(async move {
            self.requested.notify_one();
            self.release.notified().await;
            Ok(called(
                "write_mod",
                json!({"name":"broken","source":"fn repaired() {}"}),
            ))
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deliberate_deletion_wins_over_an_in_flight_repair() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let (send, mut rx) = broadcast::channel(64);
    let tools = Tools::start(dir.path(), 5, 10, send.clone())?;
    let broken = tools.execute("write_mod", json!({"name":"broken", "source":"on(\"tick\", \"broken\"); fn broken() { undefined(); }"})).await;
    assert_eq!(broken["ok"], true, "{broken}");
    tools.execute("run_ticks", json!({"n":4})).await;
    let requested = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let agent = AgentHandle::start(
        tools.clone(),
        Some(Arc::new(DelayedRepair {
            requested: requested.clone(),
            release: release.clone(),
        })),
        dir.path(),
        BudgetLimits::default(),
        settings(),
        send,
    )?;
    timeout(Duration::from_secs(5), requested.notified()).await?;
    let removed = tools.execute("delete_mod", json!({"name":"broken"})).await;
    assert_eq!(removed["ok"], true, "{removed}");
    release.notify_one();
    notice(&mut rx, "closed because its mod was removed").await?;
    assert_eq!(agent.status().pending, 0);
    let world = tools.observation().await?;
    assert!(!world["world"]["mods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["name"] == "broken"));
    assert!(!dir.path().join("mods/broken.rhai").exists());
    agent.shutdown().await?;
    tools.shutdown().await?;
    Ok(())
}
