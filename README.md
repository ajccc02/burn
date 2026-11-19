# Spawn-lite

Spawn-lite runs a persistent, headless game world. A player describes a change in chat, and the agent can inspect the world, write a sandboxed Rhai mod, advance the simulation, and verify the resulting behavior. The world keeps running while the agent waits for the model, pauses, or exhausts its budget.

## Build and run

Install [rustup](https://rustup.rs/) and a native linker. On Windows, the standard MSVC Rust host requires the Visual Studio C++ Build Tools and Windows SDK. The repository pins Rust 1.95.0.

```sh
cargo build --locked
cargo run --locked -- --mock-llm --run-id demo --seed 42 --tick-rate 10
```

The mock runs offline and supports two replayable examples. Enter `Add a wandering merchant that trades with nearby characters.` and wait for the verified reply. Then enter `Create a volcano that erupts every 60 ticks.` The merchant changes position and exchanges bread for gold. The volcano increments its eruption count and emits an `eruption` event every 60 ticks. Use `/world` to inspect these changes. `/quit` saves the world and agent before exiting.

Run `cargo run --locked` to open the default save. With `LLM_BASE_URL` or `LLM_MODEL` unset, the world runs and the agent reports `PAUSED_CONFIG`. Messages can remain queued for a later configured session.

If a Windows GNU build reports a `dlltool` error from a path containing spaces, set `CARGO_TARGET_DIR` to a writable absolute directory with no spaces before running Cargo.

## Connect a model

Copy `.env.example` to `.env` and set the base URL, model, and API key if required by the provider. Existing process environment variables take precedence. For a local OpenAI-compatible service, the configuration may look like this.

```dotenv
LLM_BASE_URL=http://localhost:1234/v1
LLM_MODEL=your-model-name
LLM_API_KEY=
LLM_MODE=tools
LLM_TOKEN_LIMIT_FIELD=max_completion_tokens
```

The client requests `<base URL>/chat/completions`. Set `LLM_MODE=json` for models that return actions as JSON text. The action envelope is `{"calls":[{"name":"list_mods","arguments":{}}],"text":null}`. Set `LLM_TOKEN_LIMIT_FIELD=max_tokens` for a server that expects the older parameter. Use an endpoint whose context window supports your configured context and output limits.

## Terminal commands

| Command | Behavior |
| --- | --- |
| `/status` | Show the world tick and current agent state |
| `/pause` | Pause the agent while the world continues |
| `/resume` | Resume the agent if its configuration and budget allow |
| `/save` | Write a world checkpoint |
| `/errors` | Read recent structured diagnostics |
| `/mods` | List mods and their activation status |
| `/budget` | Show usage, pricing, and remaining limits |
| `/world` | Inspect entities, events, and mod status |
| `/reload` | Load changed `.rhai` files from the current save |
| `/help` | List commands |
| `/quit` | Save state and exit |

For an optional WebSocket interface, run `cargo run --locked --features http -- --http`. Connect a local client to `ws://127.0.0.1:3210/ws`. Send one player input per UTF-8 text frame. The server sends JSON text frames containing `type` (`reply` or `notice`) and `text`. The listener binds to loopback and accepts up to eight clients for the shared player session. Keep stdin open while using the server.

## Write and reload mods

Save a `.rhai` file under `saves/<run-id>/mods/` and enter `/reload`. The examples under [examples/mods](examples/mods/) demonstrate initialization, events, and timers. A candidate compiles and initializes in a staged transaction. If validation fails, the current version keeps running and the agent can read the rejected source and its diagnostic. Replacing a mod replaces its subscriptions and timers. Use `restore_mod` through the agent to activate the previous working version. Deleting a mod removes its callbacks and leaves its existing entities available for inspection.

```rhai
fn count_tick() {
    let clocks = query("clock");
    if clocks.len > 0 {
        let clock = clocks[0];
        set(clock.id, "ticks", clock.props.ticks + 1);
    }
}

fn init() {
    if query("clock").len == 0 {
        spawn("clock", #{ ticks: 0 });
    }
    on("tick", "count_tick");
}
```

After three consecutive failures in a callback, Spawn-lite disables the responsible mod and reports the error. A configured agent can repair, reload, and test the mod within the repair limit. In mock mode, automatic repairs apply to the bundled examples.

## Budgets and saves

Set `--max-tokens`, `--max-iterations`, `--max-cost-usd`, `--cooldown-ms`, `--max-context-tokens`, `--max-output-tokens`, or `--max-repair-attempts` on startup. `--help` lists the context and queue limits. New runs default to 250,000 tokens and 250 model iterations. Saved spending, token, and iteration caps apply when their flags are omitted on subsequent launches. Set both `LLM_INPUT_COST_PER_1M` and `LLM_OUTPUT_COST_PER_1M` when applying a USD cap to a real model. Provider token usage is preferred. If usage is missing, Spawn-lite charges the reservation, which includes a conservative estimate of input and the allowed output. Change limits on the next launch with the same run ID to retain consumed usage and apply the new limits. `/resume` observes the hard caps.

Each run lives in `saves/<run-id>/` by default. Use `--saves-dir` to choose another root. Reopening the same run restores the tick, entities and IDs, RNG, mod versions, active timers, queued tasks, and budget. The seed applies when a save is first created. The application saves on mod changes, every ten seconds, and at clean shutdown. A damaged snapshot produces a startup error so the original remains available for recovery. Keep a backup of the run directory before repairing save files manually.

The startup log prints the replay directory for the current session. Replay applies recorded edits and tick advancement to the starting snapshot and prints the final world state.

```sh
cargo run --locked -- --replay saves/demo/replay/session-REPLACE_WITH_ID
```

## Verify and troubleshoot

```sh
cargo fmt --check
cargo check --locked --all-features
cargo test --locked
cargo test --locked --all-features
```

The tests use scripted mock replies and a local HTTP test server. If the model returns an authorization error, check the key, model ID, and chat endpoint. If the model reports an invalid request, check the token-limit parameter and whether your endpoint supports structured tools. Use `/errors` for script positions and `/budget` for the reason a new request cannot be reserved. Use `RUST_LOG=spawn_lite=debug` for detailed application logs.

The development machine for this project used a temporary Rust installation. To use that installation from PowerShell, run the following commands in this project directory. A standard rustup installation also supports all commands above.

```powershell
$SpawnTooling = Join-Path $env:LOCALAPPDATA 'Temp/spawn-lite-rust-195'
$env:CARGO_HOME = Join-Path $SpawnTooling 'cargo'
$env:RUSTUP_HOME = Join-Path $SpawnTooling 'rustup'
$env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA 'Temp/spawn-lite-build'
$env:Path = (Join-Path $env:CARGO_HOME 'bin') + ';' + $env:Path
cargo run --locked -- --mock-llm
```
