use std::iter::FusedIterator;
use std::ops::Range;

use crate::{Entity, World};

mod tuples;

mod sealed {
    use crate::{Entity, World};

    pub trait Fetch {
        type Item<'world>;
        type State<'world>;

        fn prepare(world: &World) -> Option<Self::State<'_>>;
        fn len(state: &Self::State<'_>) -> usize;
        fn row<'world>(
            state: &Self::State<'world>,
            position: usize,
        ) -> Option<(Entity, Self::Item<'world>)>;
    }
}

/// A supported shape of a read-only component query.
pub trait ReadQuery: sealed::Fetch {}

impl<Q: sealed::Fetch> ReadQuery for Q {}

/// An iterator over matching entity identifiers and shared component references.
///
/// Iteration order is unspecified.
pub struct Query<'world, Q: ReadQuery> {
    state: Option<Q::State<'world>>,
    candidates: Range<usize>,
}

impl<'world, Q: ReadQuery> Query<'world, Q> {
    pub(crate) fn new(world: &'world World) -> Self {
        let state = Q::prepare(world);
        let len = state.as_ref().map_or(0, Q::len);
        Self {
            state,
            candidates: 0..len,
        }
    }
}

impl<'world, Q: ReadQuery> Iterator for Query<'world, Q> {
    type Item = (Entity, Q::Item<'world>);

    fn next(&mut self) -> Option<Self::Item> {
        let state = self.state.as_ref()?;
        self.candidates.find_map(|position| Q::row(state, position))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.candidates.len()))
    }
}

impl<Q: ReadQuery> FusedIterator for Query<'_, Q> {}

impl<T: Send + Sync + 'static> sealed::Fetch for &T {
    type Item<'world> = &'world T;
    type State<'world> = (&'world [Entity], &'world [T]);

    fn prepare(world: &World) -> Option<Self::State<'_>> {
        let storage = world.storage::<T>()?;
        Some((storage.entities(), storage.values()))
    }

    fn len(state: &Self::State<'_>) -> usize {
        state.0.len()
    }

    fn row<'world>(
        state: &Self::State<'world>,
        position: usize,
    ) -> Option<(Entity, Self::Item<'world>)> {
        Some((*state.0.get(position)?, state.1.get(position)?))
    }
}
