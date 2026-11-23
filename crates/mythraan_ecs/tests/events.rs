//! Event queues through the public ECS API.

use std::sync::Arc;

use mythraan_ecs::EventReadError;
use mythraan_ecs::prelude::Events;

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
