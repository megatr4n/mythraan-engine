//! Event queues through the public ECS API.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use mythraan_ecs::EventReadError;
use mythraan_ecs::prelude::{Events, World};

#[test]
fn events_expire_after_their_submission_frame_and_the_next_frame() {
    let mut events = Events::default();
    let first = Arc::new(1_u32);
    let first_ref = Arc::downgrade(&first);
    events.writer().send(first).unwrap();
    assert!(first_ref.upgrade().is_some());

    events.advance_frame();
    let second = Arc::new(2_u32);
    let second_ref = Arc::downgrade(&second);
    events.writer().send(second).unwrap();
    assert!(first_ref.upgrade().is_some());
    assert!(second_ref.upgrade().is_some());

    events.advance_frame();
    assert!(first_ref.upgrade().is_none());
    assert!(second_ref.upgrade().is_some());

    events.advance_frame();
    assert!(second_ref.upgrade().is_none());
}

#[test]
fn readers_advance_independently_across_frames() {
    let mut events = Events::new();
    let mut first = events.reader();
    let mut second = events.reader();
    events.writer().send(10).unwrap();
    events.writer().send(20).unwrap();

    assert_eq!(first.read(&events).unwrap().next(), Some(&10));
    assert_eq!(
        second.read(&events).unwrap().copied().collect::<Vec<_>>(),
        [10, 20]
    );

    events.advance_frame();
    events.writer().send(30).unwrap();
    assert_eq!(
        first.read(&events).unwrap().copied().collect::<Vec<_>>(),
        [20, 30]
    );
    assert_eq!(
        second.read(&events).unwrap().copied().collect::<Vec<_>>(),
        [30]
    );
    assert_eq!(first.read(&events).unwrap().next(), None);
}

#[test]
fn late_and_idle_readers_skip_expired_events() {
    let mut events = Events::new();
    let mut idle = events.reader();
    events.writer().send(10).unwrap();
    events.writer().send(20).unwrap();
    events.advance_frame();
    events.writer().send(30).unwrap();
    events.advance_frame();
    events.writer().send(40).unwrap();

    let mut late = events.reader();
    for reader in [&mut idle, &mut late] {
        assert_eq!(
            reader.read(&events).unwrap().copied().collect::<Vec<_>>(),
            [30, 40]
        );
    }
}

#[test]
fn reading_another_queue_leaves_the_cursor_unchanged() {
    let mut first = Events::new();
    let mut second = Events::new();
    let mut reader = first.reader();
    first.writer().send(10).unwrap();
    second.writer().send(20).unwrap();

    assert!(matches!(reader.read(&second), Err(EventReadError)));
    assert_eq!(reader.read(&first).unwrap().next(), Some(&10));
    assert_eq!(reader.read(&first).unwrap().next(), None);
}

#[test]
fn iterator_adaptors_advance_the_cursor_across_both_buffers() {
    let mut events = Events::new();
    let mut reader = events.reader();
    events.writer().send(1).unwrap();
    events.writer().send(2).unwrap();
    events.advance_frame();
    events.writer().send(3).unwrap();
    events.writer().send(4).unwrap();

    assert_eq!(reader.read(&events).unwrap().nth(2), Some(&3));
    assert_eq!(reader.read(&events).unwrap().count(), 1);
    assert_eq!(reader.read(&events).unwrap().next(), None);
    events.writer().send(5).unwrap();
    assert_eq!(reader.read(&events).unwrap().next(), Some(&5));
}

#[test]
fn unwinding_during_expiry_preserves_retained_events() {
    struct DropEvent {
        id: u32,
        panic_on_drop: bool,
    }

    impl Drop for DropEvent {
        fn drop(&mut self) {
            assert!(!self.panic_on_drop, "event destructor unwound");
        }
    }

    let mut events = Events::new();
    let mut reader = events.reader();
    events
        .writer()
        .send(DropEvent {
            id: 1,
            panic_on_drop: true,
        })
        .unwrap();
    events.advance_frame();
    events
        .writer()
        .send(DropEvent {
            id: 2,
            panic_on_drop: false,
        })
        .unwrap();

    assert!(catch_unwind(AssertUnwindSafe(|| events.advance_frame())).is_err());
    assert_eq!(
        reader.read(&events).unwrap().next().map(|event| event.id),
        Some(2)
    );
    assert_eq!(reader.read(&events).unwrap().count(), 0);
    events.advance_frame();
    assert_eq!(events.reader().read(&events).unwrap().count(), 0);
}

#[test]
fn world_resources_share_event_payloads_between_readers() {
    struct HitEvent(u32);

    let mut world = World::new();
    world.resources_mut().insert(Events::<HitEvent>::new());
    let (mut sound, mut damage) = {
        let hits = world.resources().get::<Events<HitEvent>>().unwrap();
        (hits.reader(), hits.reader())
    };

    {
        let hits = world.resources_mut().get_mut::<Events<HitEvent>>().unwrap();
        hits.writer().send(HitEvent(12)).unwrap();
    }

    let hits = world.resources().get::<Events<HitEvent>>().unwrap();
    let for_sound = sound.read(hits).unwrap().next().unwrap();
    assert_eq!(sound.read(hits).unwrap().count(), 0);
    let for_damage = damage.read(hits).unwrap().next().unwrap();
    assert!(std::ptr::eq(for_sound, for_damage));
    assert_eq!(for_damage.0, 12);
}
