use super::sealed::Fetch;
use crate::{Entity, SparseSet, World};

impl<A: Send + Sync + 'static, B: Send + Sync + 'static> Fetch for (&A, &B) {
    type Item<'world> = (&'world A, &'world B);
    type State<'world> = (
        (&'world SparseSet<A>, &'world SparseSet<B>),
        &'world [Entity],
    );

    fn prepare(world: &World) -> Option<Self::State<'_>> {
        let a = world.storage::<A>()?;
        let b = world.storage::<B>()?;
        let candidates = [a.entities(), b.entities()]
            .into_iter()
            .min_by_key(|entities| entities.len())?;
        Some(((a, b), candidates))
    }

    fn len(state: &Self::State<'_>) -> usize {
        state.1.len()
    }

    fn row<'world>(
        state: &Self::State<'world>,
        position: usize,
    ) -> Option<(Entity, Self::Item<'world>)> {
        let entity = *state.1.get(position)?;
        let (a, b) = state.0;
        Some((entity, (a.get(entity)?, b.get(entity)?)))
    }
}

impl<A: Send + Sync + 'static, B: Send + Sync + 'static, C: Send + Sync + 'static> Fetch
    for (&A, &B, &C)
{
    type Item<'world> = (&'world A, &'world B, &'world C);
    type State<'world> = (
        (
            &'world SparseSet<A>,
            &'world SparseSet<B>,
            &'world SparseSet<C>,
        ),
        &'world [Entity],
    );

    fn prepare(world: &World) -> Option<Self::State<'_>> {
        let a = world.storage::<A>()?;
        let b = world.storage::<B>()?;
        let c = world.storage::<C>()?;
        let candidates = [a.entities(), b.entities(), c.entities()]
            .into_iter()
            .min_by_key(|entities| entities.len())?;
        Some(((a, b, c), candidates))
    }

    fn len(state: &Self::State<'_>) -> usize {
        state.1.len()
    }

    fn row<'world>(
        state: &Self::State<'world>,
        position: usize,
    ) -> Option<(Entity, Self::Item<'world>)> {
        let entity = *state.1.get(position)?;
        let (a, b, c) = state.0;
        Some((entity, (a.get(entity)?, b.get(entity)?, c.get(entity)?)))
    }
}
