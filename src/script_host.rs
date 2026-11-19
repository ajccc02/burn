use crate::entity::{
    allocate_id, validate_label, validate_props, EntitySnapshot, EntityStore, Properties,
    MAX_ENTITIES,
};
use anyhow::{anyhow, ensure, Result};
use rhai::{
    packages::{
        ArithmeticPackage, BasicArrayPackage, BasicIteratorPackage, BasicMapPackage,
        LanguageCorePackage, LogicPackage, Package,
    },
    Array, CallFnOptions, Dynamic, Engine, EvalAltResult, ImmutableString, Map, Scope, AST,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

pub(crate) const MAX_SUBSCRIPTIONS: usize = 8;
pub(crate) const MAX_TIMERS: usize = 32;
pub(crate) const MAX_PENDING_EVENTS: usize = 128;
const MAX_API_CALLS: usize = 256;
const MAX_QUERY_BYTES: usize = 256 * 1024;

pub(crate) type SharedEntities = Arc<Mutex<EntityStore>>;
type Slot = Arc<Mutex<Option<Invocation>>>;
type RhaiResult<T> = std::result::Result<T, Box<EvalAltResult>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Subscription {
    pub event: String,
    pub callback: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Timer {
    pub due: u64,
    pub callback: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldEvent {
    pub tick: u64,
    pub name: String,
    pub data: Properties,
}

#[derive(Debug)]
pub(crate) struct ScriptFailure {
    pub category: &'static str,
    pub line: Option<usize>,
    pub message: String,
}

pub(crate) struct Program {
    ast: AST,
    functions: BTreeMap<String, BTreeSet<usize>>,
}

impl Program {
    pub fn callback_arity(&self, name: &str) -> Option<usize> {
        callback_arity(&self.functions, name)
    }
}

pub(crate) struct ScriptHost {
    engine: Engine,
    current: Slot,
}

impl ScriptHost {
    pub fn new() -> Self {
        let mut engine = Engine::new_raw();
        engine
            .register_global_module(LanguageCorePackage::new().as_shared_module())
            .register_global_module(ArithmeticPackage::new().as_shared_module())
            .register_global_module(LogicPackage::new().as_shared_module())
            .register_global_module(BasicArrayPackage::new().as_shared_module())
            .register_global_module(BasicMapPackage::new().as_shared_module())
            .register_global_module(BasicIteratorPackage::new().as_shared_module())
            .disable_symbol("eval")
            .disable_symbol("import")
            .disable_symbol("export")
            .set_max_operations(10_000)
            .set_max_call_levels(32)
            .set_max_string_size(4096)
            .set_max_array_size(1024)
            .set_max_map_size(256)
            .set_max_variables(128)
            .set_max_functions(64)
            .set_max_expr_depths(32, 32);
        engine.on_print(|_| {});
        engine.on_debug(|_, _, _| {});

        let current = Slot::default();
        let slot = current.clone();
        // Rhai reserves the word 'spawn'. Bind only the game's two-argument form.
        engine
            .register_custom_syntax(
                ["spawn", "(", "$expr$", ",", "$expr$", ")"],
                false,
                move |context, inputs| {
                    let kind = context.eval_expression_tree(&inputs[0])?;
                    let props = context.eval_expression_tree(&inputs[1])?;
                    with_invocation(&slot, |run| {
                        let kind: String = rhai::serde::from_dynamic(&kind)?;
                        let props = rhai::serde::from_dynamic(&props)?;
                        Ok(Dynamic::from_int(run.spawn(&kind, props)? as i64))
                    })
                    .map_err(|mut error| {
                        error.set_position(inputs[0].position());
                        error
                    })
                },
            )
            .expect("fixed spawn syntax must be valid");
        let slot = current.clone();
        engine.register_fn("despawn", move |id: i64| {
            with_invocation(&slot, |run| Ok(run.despawn(id as u64)))
        });
        let slot = current.clone();
        engine.register_fn("query", move |kind: ImmutableString| {
            with_invocation(&slot, |run| run.query(kind.as_str()))
        });
        let slot = current.clone();
        engine.register_fn("get", move |id: i64| {
            with_invocation(&slot, |run| match run.get(id as u64) {
                Some(entity) => Ok(rhai::serde::to_dynamic(entity)?),
                None => Ok(Dynamic::UNIT),
            })
        });
        let slot = current.clone();
        engine.register_fn(
            "set",
            move |id: i64, key: ImmutableString, value: Dynamic| {
                with_invocation(&slot, |run| {
                    let Some(mut entity) = run.get(id as u64) else {
                        return Ok(false);
                    };
                    entity
                        .props
                        .insert(key.to_string(), rhai::serde::from_dynamic(&value)?);
                    validate_props(&entity.props)?;
                    run.changes.insert(entity.id, Some(entity));
                    Ok(true)
                })
            },
        );
        let slot = current.clone();
        engine.register_fn("set_timer", move |delay: i64, callback: ImmutableString| {
            with_invocation(&slot, |run| run.timer(delay, callback.as_str()))
        });
        let slot = current.clone();
        engine.register_fn(
            "on",
            move |event: ImmutableString, callback: ImmutableString| {
                with_invocation(&slot, |run| {
                    run.subscribe(event.as_str(), callback.as_str())
                })
            },
        );
        let slot = current.clone();
        engine.register_fn("emit", move |event: ImmutableString, data: Map| {
            with_invocation(&slot, |run| {
                let data = rhai::serde::from_dynamic(&Dynamic::from_map(data))?;
                run.emit(event.as_str(), data)
            })
        });
        let slot = current.clone();
        engine.register_fn("log", move |message: ImmutableString| {
            with_invocation(&slot, |run| {
                ensure!(run.logs.len() < 32, "callback log limit reached");
                ensure!(message.len() <= 4096, "log message is too long");
                run.logs.push(message.to_string());
                Ok(())
            })
        });

        Self { engine, current }
    }

    pub fn compile(&self, source: &str) -> std::result::Result<Program, ScriptFailure> {
        let ast = self.engine.compile(source).map_err(|error| ScriptFailure {
            category: "parse",
            line: error.position().line(),
            message: error.to_string(),
        })?;
        let mut functions: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
        for function in ast
            .iter_functions()
            .filter(|function| !function.access.is_private() && function.this_type.is_none())
        {
            functions
                .entry(function.name.to_owned())
                .or_default()
                .insert(function.params.len());
        }
        Ok(Program { ast, functions })
    }

    pub fn initialize(
        &self,
        program: &Program,
        mut invocation: Invocation,
    ) -> std::result::Result<Invocation, ScriptFailure> {
        invocation.functions = program.functions.clone();
        self.run(invocation, "initialization", || {
            self.engine.run_ast(&program.ast)?;
            if program
                .functions
                .get("init")
                .is_some_and(|arities| arities.contains(&0))
            {
                self.call(program, "init", ())
            } else {
                Ok(())
            }
        })
    }

    pub fn callback(
        &self,
        program: &Program,
        callback: &str,
        data: &Properties,
        mut invocation: Invocation,
    ) -> std::result::Result<Invocation, ScriptFailure> {
        invocation.functions = program.functions.clone();
        self.run(invocation, "runtime", || {
            match program.callback_arity(callback) {
                Some(0) => self.call(program, callback, ()),
                Some(1) => self.call(program, callback, (rhai::serde::to_dynamic(data)?,)),
                _ => Err(script_error("missing callback")),
            }
        })
    }

    fn call(&self, program: &Program, name: &str, args: impl rhai::FuncArgs) -> RhaiResult<()> {
        let _ = self.engine.call_fn_with_options::<Dynamic>(
            CallFnOptions::new().eval_ast(false),
            &mut Scope::new(),
            &program.ast,
            name,
            args,
        )?;
        Ok(())
    }

    fn run(
        &self,
        invocation: Invocation,
        category: &'static str,
        execute: impl FnOnce() -> RhaiResult<()>,
    ) -> std::result::Result<Invocation, ScriptFailure> {
        let mut slot = self
            .current
            .lock()
            .expect("simulation script slot poisoned");
        debug_assert!(slot.is_none());
        *slot = Some(invocation);
        drop(slot);
        let result = execute();
        let invocation = self
            .current
            .lock()
            .expect("simulation script slot poisoned")
            .take()
            .expect("script invocation missing");
        result.map(|_| invocation).map_err(|error| ScriptFailure {
            category,
            line: error
                .unwrap_inner()
                .position()
                .line()
                .or_else(|| error.position().line()),
            message: error.to_string(),
        })
    }
}

fn with_invocation<T>(
    slot: &Slot,
    operation: impl FnOnce(&mut Invocation) -> Result<T>,
) -> RhaiResult<T> {
    (|| {
        let mut slot = slot.lock().expect("simulation script slot poisoned");
        let invocation = slot.as_mut().ok_or_else(|| anyhow!("no active script"))?;
        ensure!(
            invocation.api_calls < MAX_API_CALLS,
            "callback API call limit reached"
        );
        invocation.api_calls += 1;
        operation(invocation)
    })()
    .map_err(script_error)
}

fn script_error(message: impl std::fmt::Display) -> Box<EvalAltResult> {
    message.to_string().into()
}

fn callback_arity(functions: &BTreeMap<String, BTreeSet<usize>>, callback: &str) -> Option<usize> {
    let arities = functions.get(callback)?;
    [1, 0].into_iter().find(|arity| arities.contains(arity))
}

pub(crate) struct Invocation {
    base: SharedEntities,
    tick: u64,
    entity_count: usize,
    event_capacity: usize,
    functions: BTreeMap<String, BTreeSet<usize>>,
    api_calls: usize,
    pub next_id: u64,
    pub changes: BTreeMap<u64, Option<EntitySnapshot>>,
    pub subscriptions: Vec<Subscription>,
    pub timers: Vec<Timer>,
    pub events: Vec<WorldEvent>,
    pub logs: Vec<String>,
}

impl Invocation {
    pub fn new(
        base: SharedEntities,
        tick: u64,
        next_id: u64,
        event_capacity: usize,
        subscriptions: Vec<Subscription>,
        timers: Vec<Timer>,
    ) -> Self {
        let entity_count = base.lock().expect("simulation entity store poisoned").len();
        Self {
            base,
            tick,
            entity_count,
            event_capacity,
            functions: BTreeMap::new(),
            api_calls: 0,
            next_id,
            changes: BTreeMap::new(),
            subscriptions,
            timers,
            events: Vec::new(),
            logs: Vec::new(),
        }
    }

    fn spawn(&mut self, kind: &str, props: Properties) -> Result<u64> {
        validate_label(kind, 64)?;
        validate_props(&props)?;
        ensure!(
            self.entity_count < MAX_ENTITIES,
            "world entity limit reached"
        );
        let id = allocate_id(&mut self.next_id)?;
        self.changes.insert(
            id,
            Some(EntitySnapshot {
                id,
                kind: kind.to_owned(),
                props,
            }),
        );
        self.entity_count += 1;
        Ok(id)
    }

    fn despawn(&mut self, id: u64) -> bool {
        if self.get(id).is_some() {
            self.changes.insert(id, None);
            self.entity_count -= 1;
            true
        } else {
            false
        }
    }

    fn get(&self, id: u64) -> Option<EntitySnapshot> {
        match self.changes.get(&id) {
            Some(change) => change.clone(),
            None => self
                .base
                .lock()
                .expect("simulation entity store poisoned")
                .get(id),
        }
    }

    fn query(&self, kind: &str) -> Result<Array> {
        validate_label(kind, 64)?;
        let mut entities: BTreeMap<_, _> = self
            .base
            .lock()
            .expect("simulation entity store poisoned")
            .query(kind)
            .into_iter()
            .map(|entity| (entity.id, entity))
            .collect();
        for (id, change) in &self.changes {
            match change {
                Some(entity) if entity.kind == kind => {
                    entities.insert(*id, entity.clone());
                }
                _ => {
                    entities.remove(id);
                }
            }
        }
        ensure!(
            entities.len() <= 1024,
            "entity query exceeds the array limit"
        );
        let mut size = 0;
        let mut result = Vec::with_capacity(entities.len());
        for entity in entities.into_values() {
            size += serde_json::to_vec(&entity)?.len();
            ensure!(
                size <= MAX_QUERY_BYTES,
                "entity query exceeds the script data limit"
            );
            result.push(rhai::serde::to_dynamic(entity)?);
        }
        Ok(result)
    }

    fn timer(&mut self, delay: i64, callback: &str) -> Result<()> {
        ensure!(delay > 0, "timer delay must be positive");
        self.validate_callback(callback)?;
        ensure!(self.timers.len() < MAX_TIMERS, "mod timer limit reached");
        let due = self
            .tick
            .checked_add(delay as u64)
            .ok_or_else(|| anyhow!("timer tick overflow"))?;
        self.timers.push(Timer {
            due,
            callback: callback.to_owned(),
        });
        Ok(())
    }

    fn subscribe(&mut self, event: &str, callback: &str) -> Result<()> {
        validate_label(event, 64)?;
        self.validate_callback(callback)?;
        let subscription = Subscription {
            event: event.to_owned(),
            callback: callback.to_owned(),
        };
        if !self.subscriptions.contains(&subscription) {
            ensure!(
                self.subscriptions.len() < MAX_SUBSCRIPTIONS,
                "mod subscription limit reached"
            );
            self.subscriptions.push(subscription);
        }
        Ok(())
    }

    fn validate_callback(&self, callback: &str) -> Result<()> {
        ensure!(
            callback_arity(&self.functions, callback).is_some(),
            "callback {callback:?} must be a public function taking zero arguments or one event map"
        );
        Ok(())
    }

    fn emit(&mut self, event: &str, data: Properties) -> Result<()> {
        validate_label(event, 64)?;
        ensure!(event != "tick", "the simulation owns tick events");
        validate_props(&data)?;
        ensure!(
            self.events.len() < self.event_capacity,
            "world event queue is full"
        );
        self.events.push(WorldEvent {
            tick: self.tick,
            name: event.to_owned(),
            data,
        });
        Ok(())
    }
}
