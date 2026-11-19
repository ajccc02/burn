# Spawn-lite design

## Ownership and modules

`world` owns the `hecs` entity store, stable numeric IDs, tick counter, seeded ChaCha RNG, mod registry, and event ordering. `entity` validates the JSON-backed properties and maintains the ID-to-`hecs` map. `script_host` compiles and invokes Rhai against copied values and staged game API calls. `error_bus` retains bounded diagnostics that include Rhai source positions when available.

`tools` owns one simulation thread per opened save. A bounded Tokio command queue and one-shot replies serialize player commands, model calls, and explicit tick advancement with the real-time timer. Plain snapshots cross thread boundaries. Requests to the model run on the agent runtime, and filesystem work for world snapshots and mod changes runs on the simulation owner. The save lock prevents two instances from using the same save concurrently. Tick timing uses skipped wakeups under load and gameplay uses simulated ticks.

`config` reads CLI and environment settings. `player` shares one agent and simulation between the terminal and optional loopback WebSocket. `agent` manages task phases, tool dispatch, verification, and repair. `observer` assembles the model context. `llm` implements the OpenAI-compatible transport and the mock using the same action and tool definitions. `budget` and `journal` persist resources and agent progress. `persist` provides atomic critical-file replacement and JSONL append. `replay` records and replays externally submitted world mutations.

## Script lifecycle and security

The Rhai host uses `Engine::new_raw()` with selected language packages and the explicit game API. Its capabilities include reading copied entity snapshots, staging entity changes, registering callbacks, creating timers, emitting events, and writing world logs. CPU operations, call depth, expression/data sizes, API calls, entity count, callback registrations, event dispatch, and timer work have independent bounds. Rhai has neither operating-system nor network capabilities. The tool dispatcher validates names, JSON shapes, bounds, and mod paths before dispatch. The HTTP client keeps credentials in transport headers and accepts responses as data. The player’s model receives world content and scripts as untrusted context and can act only through the tool registry.

Mods may use top-level initialization and an optional zero-argument `init()`. They can register `on("tick", "handler")`, subscribe to custom events, or schedule a one-shot timer with `set_timer(n, "handler")`. Callbacks take zero arguments or one copied event map. Initialization and each callback stage edits, events, timers, and logs. A failed invocation drops its staged effects. Named registrations are resolved to callbacks during validation. Mod ordering uses canonical names and scheduled work retains stable order. The world caps work within each tick and defers queued work when a limit is reached.

Callbacks run in fresh Rhai scopes. Durable mod state belongs in entity properties and the registered timer/subscription schedule. Initialization statements execute when activating a source and the saved schedule restores directly at startup. The host implements the required `spawn(...)` API with Rhai custom syntax because Rhai reserves that identifier.

`write_mod` validates the single-component name and source bounds, stages an editable `.rhai` file, preserves the previous source, and commits only successfully initialized behavior. Rejected candidates are retained alongside their structured errors. `world.json` is the authoritative versioned snapshot, including the current active source, previous version, rejected source, enabled status, and callback schedule. On existing saves the snapshot is restored first. Use `/reload` to apply files edited outside the application. An invalid candidate on disk can therefore survive a restart without replacing the active behavior. Deletion and restoration are explicit tool actions. Changes are serialized with tick processing, and a snapshot-write failure rolls back world state and attempts to restore the editable file.

Runtime errors are tracked by mod and callback. After the third consecutive failure in a callback, the mod is disabled and player and agent can inspect the diagnostic. A successful replacement can re-enable the mod. Source position and reproduction text, when available, come from the failing script. The in-memory log and error windows are bounded, and `errors.jsonl` captures newly observed diagnostics and resolution changes.

## Agent and verification

The runtime maintains a first-in-first-out player task queue and monitors unresolved runtime errors during idle periods. Background work targets known failures and avoids issuing model requests while idle. Each active iteration observes a consistent world, applies the context budget, reserves model usage, gets and validates an action, dispatches it, and journals its outcome. Requests use cooldowns and bounded retry/backoff for temporary transport failures. User pause, budget pause, absent configuration, repair, and recovery are visible states. Pause interrupts the in-flight request and conservatively settles its reservation if usage is unavailable.

The agent has to perform an actual change and advance the world before `verify_task` can mark a changed-world task complete. It submits explicit assertions over entity properties, entity counts, or events. The verifier evaluates the assertions against a fresh snapshot and can test fixed intervals between recorded events. For repairs, it additionally requires an active repaired mod, sufficient post-change ticks, and no unresolved post-change runtime error on affected mods. The model chooses which observable assertions represent the requested behavior. The built-in examples exercise assertions on a trade, merchant movement, and consecutive eruptions. Repair attempts and retries are bounded, with unresolved work surfaced to the player.

Verification first saves the world and returns that same observation in one owner command. Only assertions on this durable state can complete a task. The task completion state is then saved before the player receives its completion message.

## Context allocation

The context starts with the current instruction and task, a compact pending queue and budget status, and the current tick. The rest is admitted in priority order according to entry, byte, and context-token limits.

1. Unresolved diagnostics, preserving useful positions and snippets when space allows.
2. Relevant mod source and rejected candidates, prioritizing mods implicated in the current task or error.
3. Recent tool actions and structured results.
4. Entity counts, notable properties, recent events, and mod status.
5. Older logs and persistent journal history.

The allocator keeps text on UTF-8 boundaries, limits recorded lists, and bounds the assembled JSON by serialized size. The runtime checks the conservative request estimate including the system prompt, tool schemas, and provider framing. If necessary, lower-priority material is trimmed. The configured per-request output allowance is separately bounded. Earlier failures and current task state remain available across restarts through the persisted task queue and journal.

## Resource accounting and restart

`budget.json` records versioned consumption, saved limits, and an in-flight reservation. Startup retains saved caps when the corresponding CLI flag is omitted. Before a call the agent reserves conservatively estimated input, allowable output, one request, and one iteration. The input estimate uses serialized request bytes and a framing allowance in place of provider-specific tokenization. The request advertises the reserved output cap. The agent settles using provider usage when supplied. When it is unavailable, or a request was in flight at the time of a crash, settlement charges the full reservation. For a USD cap both input and output prices are required. A provider must honor the requested output limit for a hard pre-call cost bound. If it reports usage above the reserve, actual usage is charged and caps apply to subsequent calls.

`agent/state.json` persists the task queue, pause state, and compact history. `agent/journal.jsonl` records attempted model/tool actions and their results. `world.json` persists world data, the exact RNG state, pending events, and each mod’s state. Critical snapshots are staged and synced in the same directory before atomic replacement. Loading validates snapshot versions and limits. A clean shutdown checkpoints both components. Across an unexpected process exit, the world recovers to the last checkpoint. Journal intent and world execution are separate writes, so an interrupted action may require inspection after restart.

## Determinism and replay

Stable IDs survive ECS reconstruction. The RNG state is serialized, event targets and timer work have deterministic ordering, and callbacks use tick-relative schedules. Each session can be replayed from `replay/session-*/start.json` and its `actions.jsonl`. The journal records world mutations, accepted and rejected script activations, explicit tick advancement, and checkpoint ticks. Replay fills in the elapsed simulation ticks between recorded actions and reports any changed activation result or entity ID. It reproduces observable entity and event state for the same build and action sequence. Wall-clock scheduling controls when an external action enters the original run, and replay uses its recorded tick. LLM responses, provider failures, live transport timings, and player notices are outside the deterministic world contract.
