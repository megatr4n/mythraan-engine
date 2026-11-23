//! Standard frame order and application integration.

use mythraan_app::prelude::Stage;

#[test]
fn stage_order_follows_the_frame_contract() {
    assert_eq!(
        Stage::ORDER,
        [
            Stage::First,
            Stage::PreUpdate,
            Stage::FixedUpdate,
            Stage::Update,
            Stage::PostUpdate,
            Stage::Render,
            Stage::Last,
        ]
    );
}
