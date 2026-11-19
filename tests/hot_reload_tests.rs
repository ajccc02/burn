use serde_json::json;
use spawn_lite::world::World;

#[test]
fn rejected_candidate_rolls_back_and_survives_restart() {
    let mut world = World::new(1);
    let working = r#"
        spawn("merchant", #{ gold: 1 });
        on("tick", "profit");
        fn profit() { let m = query("merchant")[0]; set(m.id, "gold", m.props.gold + 1); }
    "#;
    let rejected = r#"spawn("duplicate", #{}); on("tick", "missing");"#;
    world.write_mod("merchant.rhai", working).unwrap();
    assert!(world.write_mod("merchant", rejected).is_err());
    assert!(world.query("duplicate").is_empty());
    assert_eq!(world.recent_errors(1)[0].category, "initialization");
    let data = serde_json::to_vec(&world.export()).unwrap();
    let mut world = World::restore(serde_json::from_slice(&data).unwrap()).unwrap();
    assert_eq!(
        world.read_mod("merchant").unwrap().candidate.as_deref(),
        Some(rejected)
    );
    assert_eq!(
        world.read_mod("merchant").unwrap().source.as_deref(),
        Some(working)
    );
    world.run_ticks(2).unwrap();
    assert_eq!(world.query("merchant")[0].props["gold"], 3);
    assert!(world.query("duplicate").is_empty());
}

#[test]
fn validated_replacement_cancels_old_timers_and_can_be_restored() {
    let mut world = World::new(1);
    let first = r#"set_timer(2, "first"); fn first() { emit("first", #{}); }"#;
    let second = r#"set_timer(2, "second"); fn second() { emit("second", #{}); }"#;
    world.write_mod("timer", first).unwrap();
    world.run_ticks(1).unwrap();
    world.write_mod("timer", second).unwrap();
    world.run_ticks(2).unwrap();
    assert_eq!(
        world
            .snapshot()
            .events
            .iter()
            .map(|event| event.name.as_str())
            .collect::<Vec<_>>(),
        ["second"]
    );
    world.restore_mod("timer").unwrap();
    world.run_ticks(2).unwrap();
    assert_eq!(
        world
            .snapshot()
            .events
            .iter()
            .map(|event| event.name.as_str())
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    assert!(world.delete_mod("timer").unwrap());
    assert!(!world.delete_mod("timer").unwrap());
}

#[test]
fn infinite_callback_is_disabled_without_losing_healthy_mods() {
    let mut world = World::new(1);
    world
        .write_mod(
            "fault",
            r#"
        on("tick", "broken");
        on("tick", "healthy");
        fn broken() {
            spawn("rolled-back", #{});
            loop {}
        }
        fn healthy() {}
    "#,
        )
        .unwrap();
    world
        .write_mod(
            "healthy",
            r#"
        let id = spawn("watcher", #{ ticks: 0 });
        on("tick", "watch");
        fn watch() { let w = query("watcher")[0]; set(w.id, "ticks", w.props.ticks + 1); }
    "#,
        )
        .unwrap();
    world.run_ticks(5).unwrap();
    assert_eq!(world.query("watcher")[0].props["ticks"], 5);
    assert!(world.query("rolled-back").is_empty());
    let errors = world.recent_errors(5);
    assert_eq!(errors.len(), 3);
    assert_eq!(errors[0].callback.as_deref(), Some("broken"));
    assert!(errors[0].disabled);
    assert_eq!(world.list_mods()[0].status, "disabled");
    let repaired = r#"
        on("tick", "broken");
        fn broken() { log("repaired"); }
    "#;
    world.write_mod("fault", repaired).unwrap();
    world.run_ticks(1).unwrap();
    assert_eq!(world.list_mods()[0].status, "loaded");
    assert!(world.recent_errors(5).iter().all(|error| error.resolved));
}

#[test]
fn runtime_diagnostic_uses_the_rhai_source_line() {
    let mut world = World::new(1);
    let source = "on(\"tick\", \"broken\");\nfn broken() {\n    missing_function();\n}";
    world.write_mod("bad_line", source).unwrap();
    world.run_ticks(1).unwrap();
    let errors = world.recent_errors(1);
    assert_eq!(errors[0].line, Some(3));
    assert_eq!(
        errors[0].repro_snippet.as_deref(),
        Some("    missing_function();")
    );
}

#[test]
fn removing_a_faulty_callback_or_mod_resolves_its_obsolete_diagnostics() {
    let mut world = World::new(1);
    let source = r#"on("tick", "broken"); fn broken() { missing_function(); }"#;
    world.write_mod("faulty", source).unwrap();
    world.run_ticks(1).unwrap();
    assert!(!world.recent_errors(1)[0].resolved);

    world
        .write_mod(
            "faulty",
            r#"on("tick", "replacement"); fn replacement() {}"#,
        )
        .unwrap();
    assert!(world.recent_errors(1)[0].resolved);

    world.write_mod("faulty", source).unwrap();
    world.run_ticks(1).unwrap();
    assert!(!world.recent_errors(1)[0].resolved);
    world.delete_mod("faulty").unwrap();
    assert!(world.recent_errors(10).iter().all(|error| error.resolved));
    let saved = serde_json::to_vec(&world.export()).unwrap();
    let restored = World::restore(serde_json::from_slice(&saved).unwrap()).unwrap();
    assert!(restored
        .recent_errors(10)
        .iter()
        .all(|error| error.resolved));
}

#[test]
fn periodic_events_survive_save_load_at_original_ticks() {
    let mut world = World::new(1);
    world
        .write_mod(
            "volcano",
            r#"
        set_timer(60, "erupt");
        fn erupt(event) {
            emit("eruption", #{ at: event.tick });
            log("volcano erupted");
            set_timer(60, "erupt");
        }
    "#,
        )
        .unwrap();
    world.run_ticks(76).unwrap();
    let data = serde_json::to_vec(&world.export()).unwrap();
    let mut world = World::restore(serde_json::from_slice(&data).unwrap()).unwrap();
    world.run_ticks(104).unwrap();
    let eruptions: Vec<_> = world
        .snapshot()
        .events
        .into_iter()
        .filter(|event| event.name == "eruption")
        .collect();
    assert_eq!(
        eruptions.iter().map(|event| event.tick).collect::<Vec<_>>(),
        [60, 120, 180]
    );
    assert!(eruptions
        .iter()
        .all(|event| event.data["at"] == json!(event.tick)));
}

#[test]
fn path_validation_rejects_traversal_and_reserved_device_names() {
    let mut world = World::new(1);
    for name in [
        "../out",
        "..\\out",
        "out/bin",
        "x.rhai.rhai",
        "con.rhai",
        "a:stream",
        "Lava",
    ] {
        assert!(world.write_mod(name, "let x = 1;").is_err(), "{name}");
    }
    assert!(world.list_mods().is_empty());
}
