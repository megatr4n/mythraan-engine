use std::any::TypeId;

use super::sealed::Fetch;
use crate::{Entity, SparseSet, SystemAccess, World};

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
                $(let $storage = if state.2 == $index {
                    $storage.values().get(position)?
                } else {
                    $storage.get(entity)?
                };)+
                Some((entity, ($($storage,)+)))
            }
        }
    };
}

read_tuple!(A => a @ 0);
read_tuple!(A => a @ 0, B => b @ 1);
read_tuple!(A => a @ 0, B => b @ 1, C => c @ 2);
read_tuple!(A => a @ 0, B => b @ 1, C => c @ 2, D => d @ 3);
