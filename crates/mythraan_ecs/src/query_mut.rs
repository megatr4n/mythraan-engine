use std::any::{Any, TypeId, type_name};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::ops::Bound::{Included, Unbounded};

use crate::change_detection::ComponentTicks;
use crate::component_storage::ComponentStorage;
use crate::sparse_set::SparseIndex;
use crate::{Entity, EntityAllocator, QueryError, SystemAccess, World};

mod fetch;
mod filters;
pub use filters::{Added, Changed, With, Without};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Optional,
    Write,
    With,
    Without,
    Added,
    Changed,
}

pub struct Request {
    id: TypeId,
    name: &'static str,
    access: Access,
}

impl Request {
    fn of<T: 'static>(access: Access) -> Self {
        Self {
            id: TypeId::of::<T>(),
            name: type_name::<T>(),
            access,
        }
    }
}

pub struct Column<'world> {
    id: TypeId,
    index: SparseIndex<'world>,
    ticks: &'world [ComponentTicks],
    shared: Option<&'world (dyn Any + Send + Sync)>,
    exclusive: Option<(&'world mut (dyn Any + Send + Sync), u64)>,
}

type Columns<'world> = [Option<Column<'world>>; 8];
type Requests = [Option<Request>; 4];

fn requests(input: impl IntoIterator<Item = Request>) -> Requests {
    let mut input = input.into_iter();
    std::array::from_fn(|_| input.next())
}

/// A supported data shape for a query that borrows the world exclusively.
///
/// Use `&T`, `&mut T`, `Option<&T>`, or tuples of up to four of these forms.
pub trait WriteQuery: fetch::Fetch {}
impl<Q: fetch::Fetch> WriteQuery for Q {}

pub(crate) fn record_data_access<Q: WriteQuery>(access: &mut SystemAccess) {
    for request in Q::requests().into_iter().flatten() {
        match request.access {
            Access::Read | Access::Optional => access.read_component(request.id),
            Access::Write => access.write_component(request.id),
            _ => {}
        }
    }
}

/// A component filter or conjunction of up to four component filters.
pub trait QueryFilter: filters::Filter {}
impl<F: filters::Filter> QueryFilter for F {}

pub(crate) fn record_filter_access<F: QueryFilter>(access: &mut SystemAccess) {
    for request in F::requests().into_iter().flatten() {
        access.read_component(request.id);
    }
}

/// An exclusive world query that retains its filter type in system declarations.
pub struct FilteredQuery<'world, Q: WriteQuery, F: QueryFilter> {
    query: QueryMut<'world, Q>,
    filter: PhantomData<fn() -> F>,
}

impl<'world, Q: WriteQuery, F: QueryFilter> FilteredQuery<'world, Q, F> {
    /// Prepares a filtered query for the current tick.
    ///
    /// # Errors
    /// Returns the same access conflicts as [`World::query_filtered_mut`].
    pub fn from_world(world: &'world mut World) -> Result<Self, QueryError> {
        Ok(Self {
            query: world.query_filtered_mut::<Q, F>()?,
            filter: PhantomData,
        })
    }

    /// Prepares a filtered query whose change filters match after `after_tick`.
    ///
    /// # Errors
    /// Returns the same access conflicts as [`World::query_filtered_mut_since`].
    pub fn from_world_since(world: &'world mut World, after_tick: u64) -> Result<Self, QueryError> {
        Ok(Self {
            query: world.query_filtered_mut_since::<Q, F>(after_tick)?,
            filter: PhantomData,
        })
    }

    /// Calls `visit` with the requested components of every matching entity.
    pub fn for_each(self, visit: impl for<'row> FnMut(Entity, Q::Item<'row>)) {
        self.query.for_each(visit);
    }
}

/// Visits each matching entity with the component borrows requested by the query.
pub struct QueryMut<'world, Q: WriteQuery> {
    entities: &'world EntityAllocator,
    candidates: Option<&'world [Entity]>,
    filters: [Option<PreparedFilter<'world>>; 4],
    after_tick: u64,
    state: Option<Q::State<'world>>,
}

impl<'world, Q: WriteQuery> QueryMut<'world, Q> {
    pub(crate) fn new(
        entities: &'world EntityAllocator,
        storages: &'world mut BTreeMap<TypeId, Box<dyn ComponentStorage>>,
        filters: Requests,
        after_tick: u64,
        tick: u64,
    ) -> Result<Self, QueryError> {
        let requests = Q::requests();
        for (position, request) in requests.iter().enumerate() {
            let Some(request) = request else { continue };
            if requests.iter().skip(position + 1).flatten().any(|other| {
                other.id == request.id
                    && (other.access == Access::Write || request.access == Access::Write)
            }) {
                return Err(QueryError {
                    component: request.name,
                });
            }
        }

        let requested = || requests.iter().chain(&filters).flatten();
        let keys = requested().map(|request| request.id);
        let bounds = (
            keys.clone().min().map_or(Unbounded, Included),
            keys.max().map_or(Unbounded, Included),
        );
        let mut columns: Columns<'world> = [const { None }; 8];
        for (&id, storage) in storages.range_mut(bounds) {
            let Some(request) = requested().find(|request| request.id == id) else {
                continue;
            };
            let Some(slot) = columns.iter_mut().find(|column| column.is_none()) else {
                break;
            };
            let (index, ticks, values) = storage.query_parts();
            let (shared, exclusive) = if request.access == Access::Write {
                (None, Some((values, tick)))
            } else {
                (Some(&*values), None)
            };
            *slot = Some(Column {
                id,
                index,
                ticks,
                shared,
                exclusive,
            });
        }

        let candidates = requested()
            .filter(|request| !matches!(request.access, Access::Optional | Access::Without))
            .map(|request| index_of(&columns, request.id).entities())
            .min_by_key(|source| source.len());
        let filters = filters.map(|filter| {
            filter.map(|filter| {
                let (index, ticks) = column_of(&columns, filter.id)
                    .map(|column| (column.index, column.ticks))
                    .unwrap_or_default();
                PreparedFilter {
                    index,
                    ticks,
                    access: filter.access,
                }
            })
        });
        let state = Q::prepare(&mut columns);
        Ok(Self {
            entities,
            candidates,
            filters,
            after_tick,
            state,
        })
    }

    /// Calls `visit` with the requested components of every matching entity.
    ///
    /// Each yielded mutable reference marks its component as changed.
    pub fn for_each(mut self, mut visit: impl for<'row> FnMut(Entity, Q::Item<'row>)) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let end = self
            .filters
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |last| last + 1);
        let filters = self.filters.get(..end).unwrap_or_default();
        let visit_entity = |entity| {
            if filters
                .iter()
                .flatten()
                .all(|filter| filter.matches(entity, self.after_tick))
                && let Some(item) = Q::get(state, entity)
            {
                visit(entity, item);
            }
        };
        match self.candidates {
            Some(source) => source.iter().copied().for_each(visit_entity),
            None => self.entities.live_entities().for_each(visit_entity),
        }
    }
}

struct PreparedFilter<'world> {
    index: SparseIndex<'world>,
    ticks: &'world [ComponentTicks],
    access: Access,
}

impl PreparedFilter<'_> {
    fn matches(&self, entity: Entity, after_tick: u64) -> bool {
        let position = self.index.position(entity);
        match self.access {
            Access::With => position.is_some(),
            Access::Without => position.is_none(),
            Access::Added => position
                .and_then(|position| self.ticks.get(position))
                .is_some_and(|ticks| ticks.added > after_tick),
            Access::Changed => position
                .and_then(|position| self.ticks.get(position))
                .is_some_and(|ticks| ticks.changed() > after_tick),
            _ => false,
        }
    }
}

fn index_of<'world>(columns: &Columns<'world>, id: TypeId) -> SparseIndex<'world> {
    column_of(columns, id).map_or_else(SparseIndex::default, |column| column.index)
}

fn column_of<'a, 'world>(columns: &'a Columns<'world>, id: TypeId) -> Option<&'a Column<'world>> {
    columns.iter().flatten().find(|column| column.id == id)
}
