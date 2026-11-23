use std::any::{Any, TypeId, type_name};
use std::collections::BTreeMap;

use crate::component_storage::ComponentStorage;
use crate::sparse_set::SparseIndex;
use crate::{Entity, EntityAllocator, QueryError};

mod fetch;
mod filters;
pub use filters::{With, Without};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Optional,
    Write,
    With,
    Without,
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
    shared: Option<&'world (dyn Any + Send + Sync)>,
    exclusive: Option<&'world mut (dyn Any + Send + Sync)>,
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

/// A presence filter or conjunction of up to four presence filters.
pub trait QueryFilter: filters::Filter {}
impl<F: filters::Filter> QueryFilter for F {}

/// Visits each matching entity with the component borrows requested by the query.
pub struct QueryMut<'world, Q: WriteQuery> {
    entities: &'world EntityAllocator,
    candidates: Option<&'world [Entity]>,
    filters: [Option<(SparseIndex<'world>, bool)>; 4],
    state: Option<Q::State<'world>>,
}

impl<'world, Q: WriteQuery> QueryMut<'world, Q> {
    pub(crate) fn new(
        entities: &'world EntityAllocator,
        storages: &'world mut BTreeMap<TypeId, Box<dyn ComponentStorage>>,
        filters: Requests,
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
        let mut columns: Columns<'world> = [const { None }; 8];
        for (&id, storage) in storages.iter_mut() {
            let Some(request) = requested().find(|request| request.id == id) else {
                continue;
            };
            let Some(slot) = columns.iter_mut().find(|column| column.is_none()) else {
                break;
            };
            let (index, values) = storage.query_parts();
            let (shared, exclusive) = if request.access == Access::Write {
                (None, Some(values))
            } else {
                (Some(&*values), None)
            };
            *slot = Some(Column {
                id,
                index,
                shared,
                exclusive,
            });
        }

        let candidates = requested()
            .filter(|request| !matches!(request.access, Access::Optional | Access::Without))
            .map(|request| index_of(&columns, request.id).entities())
            .min_by_key(|source| source.len());
        let filters = filters.map(|filter| {
            filter.map(|filter| (index_of(&columns, filter.id), filter.access == Access::With))
        });
        let state = Q::prepare(&mut columns);
        Ok(Self {
            entities,
            candidates,
            filters,
            state,
        })
    }

    /// Calls `visit` with the requested components of every matching entity.
    pub fn for_each(mut self, mut visit: impl for<'row> FnMut(Entity, Q::Item<'row>)) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let visit_entity = |entity| {
            if self
                .filters
                .iter()
                .flatten()
                .all(|(index, present)| index.position(entity).is_some() == *present)
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

fn index_of<'world>(columns: &Columns<'world>, id: TypeId) -> SparseIndex<'world> {
    columns
        .iter()
        .flatten()
        .find(|column| column.id == id)
        .map_or_else(SparseIndex::default, |column| column.index)
}
