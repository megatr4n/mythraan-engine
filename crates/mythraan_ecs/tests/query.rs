//! Read-only query behavior through the public ECS API.

use mythraan_ecs::prelude::{Query, World};

#[derive(Debug, PartialEq, Eq)]
struct Health(u32);

#[test]
fn single_query_reads_components_from_matching_entities() {
    let mut world = World::new();
    let first = world.spawn().unwrap();
    let unrelated = world.spawn().unwrap();
    let second = world.spawn().unwrap();
    world.insert(first, Health(40)).unwrap();
    world.insert(unrelated, 99_u32).unwrap();
    world.insert(second, Health(80)).unwrap();

    let query: Query<'_, &Health> = world.query();
    let mut values: Vec<_> = query.map(|(entity, health)| (entity, health.0)).collect();
    values.sort_unstable_by_key(|(entity, _)| entity.index());
    assert_eq!(values, [(first, 40), (second, 80)]);
}
