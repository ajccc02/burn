use serde_json::json;
use spawn_lite::world::World;

#[test]
fn game_api_delivers_copied_event_and_entity_maps() {
    let mut world = World::new(7);
    world
        .write_mod(
            "growth",
            r#"
        let id = spawn("tree", #{ age: 0, history: [(), #{ healthy: true }] });
        let tree = get(id);
        tree.props.age = 99;
        on("rain", "grow");
        set_timer(2, "rainstorm");
        fn rainstorm() { emit("rain", #{ amount: 2 }); }
        fn grow(event) {
            let tree = query("tree")[0];
            set(tree.id, "age", tree.props.age + event.amount);
            despawn(spawn("drop", #{}));
            log("growth recorded");
        }
    "#,
        )
        .unwrap();
    assert_eq!(world.query("tree")[0].props["age"], 0);
    assert_eq!(
        world.query("tree")[0].props["history"],
        json!([null, {"healthy": true}])
    );
    world.run_ticks(2).unwrap();
    assert_eq!(world.query("tree")[0].props["age"], 2);
    assert!(world.query("drop").is_empty());
    assert_eq!(world.snapshot().events[0].name, "rain");
    assert_eq!(
        world.recent_logs(Some("info"), 1)[0].message,
        "growth recorded"
    );
    assert!(world.recent_errors(10).is_empty());
}

#[test]
fn sandbox_rejects_external_access_and_dynamic_evaluation() {
    for source in [
        r#"read_file("example.txt");"#,
        r#"getenv("LLM_API_KEY");"#,
        r#"http_get("https://example.invalid");"#,
        r#"run_command("example");"#,
        r#"eval("40 + 2");"#,
        r#"import "external" as module;"#,
    ] {
        let mut world = World::new(1);
        assert!(world.write_mod("restricted", source).is_err(), "{source}");
        assert_eq!(world.list_mods()[0].status, "rejected");
        assert_eq!(world.recent_errors(1)[0].mod_name, "restricted");
    }
}

#[test]
fn sandbox_enforces_operation_and_recursion_limits() {
    for source in [
        "loop { let a = 1; }",
        "fn recurse() { 1 + recurse() } recurse();",
    ] {
        let mut world = World::new(3);
        assert!(world.write_mod("bounded", source).is_err(), "{source}");
        assert_eq!(world.recent_errors(1)[0].category, "initialization");
    }
}

#[test]
fn sandbox_enforces_string_and_array_limits() {
    let oversized_string = format!("let text = \"{}\"; log(text);", "x".repeat(4097));
    let oversized_array = format!(
        "let items = [0{}]; log(\"unreachable\");",
        ",0".repeat(1024)
    );
    for source in [oversized_string, oversized_array] {
        let mut world = World::new(3);
        assert!(world.write_mod("bounded", &source).is_err());
        assert!(world.recent_logs(None, 10).is_empty());
    }
}

#[test]
fn recursive_events_defer_and_allow_ticks_to_continue() {
    let mut world = World::new(3);
    world
        .write_mod(
            "echo",
            r#"
                on("echo", "again");
                emit("echo", #{});
                fn again() { emit("echo", #{}); }
            "#,
        )
        .unwrap();
    world.run_ticks(1).unwrap();
    let initial = world.snapshot().events.len();
    assert!((2..=100).contains(&initial));
    world.run_ticks(1).unwrap();
    assert_eq!(world.tick(), 2);
    assert!(world.snapshot().events.len() > initial);
    assert!(world.recent_errors(1).is_empty());
}
