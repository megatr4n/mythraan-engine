use std::any::TypeId;

use super::sealed::Fetch;
use crate::{Entity, SparseSet, SystemAccess, World};

macro_rules! read_tuple {
    ($first:ident => $leading:ident $(, $component:ident => $storage:ident)* $(,)?) => {
        impl<$first: Send + Sync + 'static $(, $component: Send + Sync + 'static)*>
            Fetch for (&$first, $(&$component,)*)
        {
            type Item<'world> = (&'world $first, $(&'world $component,)*);
            type State<'world> = (
                (&'world SparseSet<$first>, $(&'world SparseSet<$component>,)*),
                &'world [Entity],
                bool,
            );

            fn record_reads(access: &mut SystemAccess) {
                access.read_component(TypeId::of::<$first>());
                $(access.read_component(TypeId::of::<$component>());)*
            }

            fn prepare(world: &World) -> Option<Self::State<'_>> {
                let $leading = world.storage::<$first>()?;
                $(let $storage = world.storage::<$component>()?;)*
                let (driver, candidates) = [$leading.entities(), $($storage.entities(),)*]
                    .into_iter()
                    .enumerate()
                    .min_by_key(|(_, entities)| entities.len())?;
                Some((($leading, $($storage,)*), candidates, driver == 0))
            }

            fn len(state: &Self::State<'_>) -> usize {
                state.1.len()
            }

            fn row<'world>(
                state: &Self::State<'world>,
                position: usize,
            ) -> Option<(Entity, Self::Item<'world>)> {
                let entity = *state.1.get(position)?;
                let ($leading, $($storage,)*) = state.0;
                let first = if state.2 {
                    $leading.values().get(position)?
                } else {
                    $leading.get(entity)?
                };
                Some((entity, (first, $($storage.get(entity)?,)*)))
            }
        }
    };
}

read_tuple!(A => a);
read_tuple!(A => a, B => b);
read_tuple!(A => a, B => b, C => c);
read_tuple!(A => a, B => b, C => c, D => d);
