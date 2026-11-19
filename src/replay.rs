use crate::{
    entity::Properties,
    persist,
    world::{World, WorldSnapshot, WorldState},
};
use anyhow::{anyhow, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

const MAX_RECORD_BYTES: u64 = 512 * 1024;
const MAX_TICK_GAP: u64 = 1_000_000;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum WorldAction {
    WriteMod {
        name: String,
        source: String,
        accepted: bool,
    },
    DeleteMod {
        name: String,
    },
    RestoreMod {
        name: String,
        accepted: bool,
    },
    Spawn {
        kind: String,
        props: Properties,
        id: u64,
    },
    Advance {
        ticks: u64,
    },
    Checkpoint,
}

#[derive(Debug, Serialize, Deserialize)]
struct Record {
    tick: u64,
    action: WorldAction,
}

pub(crate) struct Recorder {
    dir: PathBuf,
}

impl Recorder {
    pub fn start(root: &Path, world: &World) -> Result<Self> {
        let replay = root.join("replay");
        fs::create_dir_all(&replay)?;
        let dir = tempfile::Builder::new()
            .prefix("session-")
            .tempdir_in(&replay)?;
        persist::write_json(&dir.path().join("start.json"), &world.export())?;
        File::create_new(dir.path().join("actions.jsonl"))?;
        let dir = dir.keep();
        tracing::info!(directory = %dir.display(), "recording deterministic replay");
        Ok(Self { dir })
    }

    pub fn append(&self, tick: u64, action: WorldAction) -> Result<()> {
        persist::append_json_line(&self.dir.join("actions.jsonl"), &Record { tick, action })
    }
}

/// Replay a recorded session from its original state without starting the app or model.
pub fn replay_session(dir: &Path) -> Result<WorldSnapshot> {
    let start = persist::read_json::<WorldState>(&dir.join("start.json"))?
        .ok_or_else(|| anyhow!("replay start.json is missing"))?;
    let mut world = World::restore(start)?;
    let mut actions = BufReader::new(File::open(dir.join("actions.jsonl"))?);
    let mut line = String::new();
    for n in 1_u64.. {
        line.clear();
        if (&mut actions)
            .take(MAX_RECORD_BYTES + 1)
            .read_line(&mut line)?
            == 0
        {
            break;
        }
        ensure!(
            line.len() as u64 <= MAX_RECORD_BYTES && line.ends_with('\n'),
            "replay record {n} exceeds the limit or is incomplete"
        );
        let Record { tick, action } =
            serde_json::from_str(&line).with_context(|| format!("reading replay record {n}"))?;
        let advance = tick
            .checked_sub(world.tick())
            .ok_or_else(|| anyhow!("replay record {n} moves backwards in time"))?;
        ensure!(
            advance <= MAX_TICK_GAP,
            "replay record {n} skips too many ticks"
        );
        world.run_ticks(advance)?;
        let equivalent = match action {
            WorldAction::WriteMod {
                name,
                source,
                accepted,
            } => world.write_mod(&name, &source).is_ok() == accepted,
            WorldAction::DeleteMod { name } => world.delete_mod(&name)?,
            WorldAction::RestoreMod { name, accepted } => {
                world.restore_mod(&name).is_ok() == accepted
            }
            WorldAction::Spawn { kind, props, id } => world.spawn(&kind, props)? == id,
            WorldAction::Advance { ticks } => {
                ensure!(
                    (1..=120).contains(&ticks),
                    "replay record {n} has an invalid explicit tick count"
                );
                world.run_ticks(ticks)?;
                true
            }
            WorldAction::Checkpoint => true,
        };
        ensure!(equivalent, "replay diverged at record {n}, tick {tick}");
    }
    Ok(world.snapshot())
}
