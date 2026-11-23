//! Event queues through the public ECS API.

use std::sync::Arc;

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
