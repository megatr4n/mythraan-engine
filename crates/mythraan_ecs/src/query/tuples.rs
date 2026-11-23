use super::sealed::Fetch;
use crate::{Entity, SparseSet, World};

macro_rules! read_tuple {
    ($($component:ident => $storage:ident),+ $(,)?) => {
        impl<$($component: Send + Sync + 'static),+> Fetch for ($(&$component,)+) {
            type Item<'world> = ($(&'world $component,)+);
            type State<'world> = (($(&'world SparseSet<$component>,)+), &'world [Entity]);

            fn prepare(world: &World) -> Option<Self::State<'_>> {
                $(let $storage = world.storage::<$component>()?;)+
                let candidates = [$($storage.entities()),+]
                    .into_iter()
                    .min_by_key(|entities| entities.len())?;
                Some((($($storage,)+), candidates))
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
                Some((entity, ($($storage.get(entity)?,)+)))
            }
        }
    };
}

read_tuple!(A => a, B => b);
read_tuple!(A => a, B => b, C => c);
read_tuple!(A => a, B => b, C => c, D => d);
