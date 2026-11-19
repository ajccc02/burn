use serde_json::{json, Map};
use spawn_lite::world::World;

#[test]
fn entity_operations_preserve_id_order_and_snapshots_are_copies() {
    let mut world = World::new(42);
    let first = world.spawn("villager", Map::new()).unwrap();
    let second = world.spawn("villager", Map::new()).unwrap();
    let other = world.spawn("house", Map::new()).unwrap();
    assert_eq!(first, 1);
    assert_eq!(
        world
            .query("villager")
            .iter()
            .map(|entity| entity.id)
            .collect::<Vec<_>>(),
        [first, second]
    );
    assert!(world.set(first, "name", json!("Ada")).unwrap());
    let mut detached = world.get(first).unwrap();
    detached.props.insert("name".into(), json!("changed"));
    assert_eq!(world.get(first).unwrap().props["name"], "Ada");
    assert!(!world.set(1000, "name", json!("missing")).unwrap());
    assert!(world.despawn(second));
    assert!(!world.despawn(second));
    assert_eq!(world.query("villager").len(), 1);
    assert_eq!(world.query("house")[0].id, other);
    assert!(world.spawn("house", Map::new()).unwrap() > other);
}

#[test]
fn save_restore_preserves_rng_progress_and_equivalent_replay() {
    let mut original = World::new(42);
    original.spawn("player", Map::new()).unwrap();
    original.run_ticks(127).unwrap();
    let data = serde_json::to_string(&original.export()).unwrap();
    let mut replay = World::restore(serde_json::from_str(&data).unwrap()).unwrap();
    assert_eq!(original.snapshot(), replay.snapshot());
    for world in [&mut original, &mut replay] {
        world.spawn("house", Map::new()).unwrap();
        world.run_ticks(45).unwrap();
        world.set(1, "home", json!(2)).unwrap();
        world.run_ticks(140).unwrap();
    }
    assert_eq!(original.snapshot(), replay.snapshot());
    let ambient: Vec<_> = replay
        .snapshot()
        .events
        .into_iter()
        .filter(|event| event.name == "ambient")
        .map(|event| event.tick)
        .collect();
    assert_eq!(ambient, [100, 200, 300]);
}

#[test]
fn restore_rejects_duplicate_entity_ids() {
    let mut world = World::new(1);
    world.spawn("tree", Map::new()).unwrap();
    let mut saved = world.export();
    saved.entities.push(saved.entities[0].clone());
    assert!(World::restore(saved).is_err());
}
