//! Native stack regression tests for discarding deferred components.

// Miri tests command behavior, but does not execute the native stack layout.
#![cfg(not(miri))]

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use mythraan_ecs::{CommandBuffer, CommandError, World};

static DROP_MARKER: AtomicUsize = AtomicUsize::new(0);

struct Payload<const N: usize>([u8; N]);

impl<const N: usize> Drop for Payload<N> {
    #[inline(never)]
    fn drop(&mut self) {
        black_box(&self.0);
        let marker = 0_u8;
        DROP_MARKER.store(black_box(&marker) as *const u8 as usize, Ordering::Relaxed);
    }
}

#[inline(never)]
fn measure_discard(commands: CommandBuffer, world: Option<&mut World>) -> usize {
    let marker = 0_u8;
    let start = black_box(&marker) as *const u8 as usize;
    DROP_MARKER.store(0, Ordering::Relaxed);

    let mut commands = black_box(commands);
    if let Some(world) = world {
        let report = commands.flush(world);
        assert_eq!(report.failures().len(), 1);
        assert!(matches!(
            report.failures()[0].error,
            CommandError::ForeignPlaceholder
        ));
    }
    drop(commands);

    let end = DROP_MARKER.load(Ordering::Relaxed);
    assert_ne!(end, 0, "component destructor did not run");
    black_box(&marker);
    start.abs_diff(end)
}

fn stack_distance<const N: usize>(world: Option<&mut World>) -> usize {
    let foreign = CommandBuffer::new().spawn();
    let mut commands = CommandBuffer::new();
    commands.insert(&foreign, Payload([0; N]));
    measure_discard(commands, world)
}

#[test]
fn discarding_a_component_uses_less_than_512_bytes_of_extra_stack() {
    let mut world = World::new();
    for (path, small, large) in [
        (
            "buffer drop",
            stack_distance::<64>(None),
            stack_distance::<4096>(None),
        ),
        (
            "foreign target",
            stack_distance::<64>(Some(&mut world)),
            stack_distance::<4096>(Some(&mut world)),
        ),
    ] {
        let growth = large.saturating_sub(small);
        println!(
            "{path}: {small} B with 64 B payload, {large} B with 4096 B payload, {growth} B growth"
        );
        assert!(growth < 512, "{path}: stack use grew by {growth} bytes");
    }
}
