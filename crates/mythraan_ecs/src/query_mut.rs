use std::any::{Any, TypeId, type_name};
use std::collections::BTreeMap;

use crate::component_storage::ComponentStorage;
use crate::sparse_set::SparseIndex;
use crate::{Entity, EntityAllocator, QueryError};

mod fetch;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Optional,
    Write,
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

type Columns<'world> = [Option<Column<'world>>; 4];
type Requests = [Option<Request>; 4];

/// A supported data shape for a query that borrows the world exclusively.
///
/// Use `&T`, `&mut T`, `Option<&T>`, or tuples of up to four of these forms.
pub trait WriteQuery: fetch::Fetch {}
impl<Q: fetch::Fetch> WriteQuery for Q {}

/// Visits each matching entity with the component borrows requested by the query.
pub struct QueryMut<'world, Q: WriteQuery> {
    entities: &'world EntityAllocator,
    candidates: Option<&'world [Entity]>,
    state: Option<Q::State<'world>>,
}

impl<'world, Q: WriteQuery> QueryMut<'world, Q> {
    pub(crate) fn new(
        entities: &'world EntityAllocator,
        storages: &'world mut BTreeMap<TypeId, Box<dyn ComponentStorage>>,
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

        let mut columns: Columns<'world> = [const { None }; 4];
        for (&id, storage) in storages.iter_mut() {
            let Some(request) = requests.iter().flatten().find(|request| request.id == id) else {
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

        let candidates = requests
            .iter()
            .flatten()
            .filter(|request| request.access != Access::Optional)
            .map(|request| index_of(&columns, request.id).entities())
            .min_by_key(|source| source.len());
        let state = Q::prepare(&mut columns);
        Ok(Self {
            entities,
            candidates,
            state,
        })
    }

    /// Calls `visit` with the requested components of every matching entity.
    pub fn for_each(mut self, mut visit: impl for<'row> FnMut(Entity, Q::Item<'row>)) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let visit_entity = |entity| {
            if let Some(item) = Q::get(state, entity) {
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
