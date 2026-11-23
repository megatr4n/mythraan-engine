//! Conflict-aware execution through the public schedule API.

use mythraan_ecs::prelude::{Res, ResMut, Schedule, SystemAccess, World};

#[test]
fn conflicting_systems_see_writes_in_registration_order() {
    let mut world = World::new();
    world.resources_mut().insert(1_u32);
    let mut schedule = Schedule::new();
    schedule.add_system(
        SystemAccess::of::<ResMut<'_, u32>>(),
        |mut scope, commands| {
            assert!(commands.is_none());
            *scope.resources.get_mut::<u32>()?.unwrap() += 2;
            Ok(())
        },
    );
    schedule.add_system(SystemAccess::of::<Res<'_, u32>>(), |scope, _| {
        assert_eq!(*scope.resources.get::<u32>()?.unwrap(), 3);
        Ok(())
    });
    schedule.add_system(SystemAccess::of::<ResMut<'_, u32>>(), |mut scope, _| {
        *scope.resources.get_mut::<u32>()?.unwrap() *= 2;
        Ok(())
    });

    assert!(schedule.run(&mut world).unwrap().is_empty());
    assert_eq!(world.resources().get::<u32>(), Some(&6));
}
