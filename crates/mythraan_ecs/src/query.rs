use std::iter::FusedIterator;
use std::ops::Range;

use crate::{Entity, SystemAccess, World};

mod tuples;

mod sealed {
    use std::ops::Range;

    use crate::{Entity, SystemAccess, World};

    pub trait Fetch {
        type Item<'world>;
        type State<'world>;

        fn record_reads(access: &mut SystemAccess);
        fn prepare(world: &World) -> Option<Self::State<'_>>;
        fn len(state: &Self::State<'_>) -> usize;
        fn row<'world>(
            state: &Self::State<'world>,
            position: usize,
        ) -> Option<(Entity, Self::Item<'world>)>;

        fn fold<'world, B>(
            state: &Self::State<'world>,
            candidates: Range<usize>,
            init: B,
            visit: impl FnMut(B, (Entity, Self::Item<'world>)) -> B,
        ) -> B {
            candidates
                .filter_map(|position| Self::row(state, position))
                .fold(init, visit)
        }
    }
}

/// A supported shape of a read-only component query.
///
/// Use `&T` or a tuple of one to four shared component references.
pub trait ReadQuery: sealed::Fetch {}

impl<Q: sealed::Fetch> ReadQuery for Q {}

/// An iterator over matching entity identifiers and shared component references.
///
/// A tuple query visits the smallest requested component storage and yields a
/// row only when the entity has every requested component. Iteration order is
/// unspecified.
///
/// ```
/// use mythraan_ecs::prelude::World;
///
/// struct Position(i32);
/// struct Health(u32);
///
/// let mut world = World::new();
/// let entity = world.spawn()?;
/// world.insert(entity, Position(12))?;
/// world.insert(entity, Health(80))?;
/// let mut query = world.query::<(&Position, &Health)>();
/// let result = query.next().map(|(id, (pos, hp))| (id, pos.0, hp.0));
/// assert_eq!(result, Some((entity, 12, 80)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// A live query borrows the world, so the world cannot be mutated while the
/// query is still in use.
///
/// ```compile_fail,E0502
/// use mythraan_ecs::prelude::World;
///
/// let mut world = World::new();
/// let mut query = world.query::<&u32>();
/// let _ = world.spawn();
/// let _ = query.next();
/// ```
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

    fn fold<B, F>(self, init: B, visit: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let Some(state) = self.state.as_ref() else {
            return init;
        };
        Q::fold(state, self.candidates, init, visit)
    }
}

impl<Q: ReadQuery> FusedIterator for Query<'_, Q> {}

impl<T: Send + Sync + 'static> sealed::Fetch for &T {
    type Item<'world> = &'world T;
    type State<'world> = (&'world [Entity], &'world [T]);

    fn record_reads(access: &mut SystemAccess) {
        access.read_component(std::any::TypeId::of::<T>());
    }

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
