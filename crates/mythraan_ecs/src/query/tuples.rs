use std::any::TypeId;
use std::ops::Range;

use super::sealed::Fetch;
use crate::{Entity, SparseSet, SystemAccess, World};

macro_rules! tuple_values {
    ($entity:ident, $position:ident, $driver:expr; ($($storage:ident @ $index:literal),+)) => {
        ($(if $driver == $index {
            $storage.values().get($position)?
        } else {
            $storage.get($entity)?
        },)+)
    };
}

macro_rules! fold_driver {
    ($driver:expr, $rows:ident, $init:ident, $visit:ident; $columns:tt; $($index:literal),+) => {
        // Each branch reads a fixed dense column for the whole scan.
        match $driver {
            $($index => $rows.filter_map(|(position, entity)| {
                Some((entity, tuple_values!(entity, position, $index; $columns)))
            }).fold($init, $visit),)+
            _ => $init,
        }
    };
}

macro_rules! read_tuple {
    ($($component:ident => $storage:ident @ $index:literal),+ $(,)?) => {
        impl<$($component: Send + Sync + 'static),+> Fetch for ($(&$component,)+) {
            type Item<'world> = ($(&'world $component,)+);
            type State<'world> = (
                ($(&'world SparseSet<$component>,)+),
                &'world [Entity],
                usize,
            );

            fn record_reads(access: &mut SystemAccess) {
                $(access.read_component(TypeId::of::<$component>());)+
            }

            fn prepare(world: &World) -> Option<Self::State<'_>> {
                $(let $storage = world.storage::<$component>()?;)+
                let (driver, candidates) = [$(($index, $storage.entities()),)+]
                    .into_iter()
                    .min_by_key(|(_, entities)| entities.len())?;
                Some((($($storage,)+), candidates, driver))
            }

            fn len(state: &Self::State<'_>) -> usize {
                state.1.len()
            }

            fn row<'world>(
                state: &Self::State<'world>,
                position: usize,
            ) -> Option<(Entity, Self::Item<'world>)> {
                let entity = *state.1.get(position)?;
                let ($($storage,)+) = state.0;
                let values = tuple_values!(entity, position, state.2; ($($storage @ $index),+));
                Some((entity, values))
            }

            fn fold<'world, Acc>(
                state: &Self::State<'world>,
                candidates: Range<usize>,
                init: Acc,
                visit: impl FnMut(Acc, (Entity, Self::Item<'world>)) -> Acc,
            ) -> Acc {
                let start = candidates.start;
                let Some(entities) = state.1.get(candidates) else {
                    return init;
                };
                let ($($storage,)+) = state.0;
                let rows = entities.iter().copied().enumerate()
                    .map(|(offset, entity)| (start + offset, entity));
                fold_driver!(state.2, rows, init, visit; ($($storage @ $index),+); $($index),+)
            }
        }
    };
}

read_tuple!(A => a @ 0);
read_tuple!(A => a @ 0, B => b @ 1);
read_tuple!(A => a @ 0, B => b @ 1, C => c @ 2);
read_tuple!(A => a @ 0, B => b @ 1, C => c @ 2, D => d @ 3);
