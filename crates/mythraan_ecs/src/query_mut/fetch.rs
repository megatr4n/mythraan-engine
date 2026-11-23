use std::any::TypeId;

use super::{Access, Columns, Request, Requests};
use crate::Entity;
use crate::sparse_set::SparseIndex;

pub trait Fetch {
    type State<'world>;
    type Item<'row>;
    fn requests() -> Requests;
    fn prepare<'world>(columns: &mut Columns<'world>) -> Option<Self::State<'world>>;
    fn get<'row>(state: &'row mut Self::State<'_>, entity: Entity) -> Option<Self::Item<'row>>;
}

pub trait Field {
    type State<'world>;
    type Item<'row>;
    fn request() -> Request;
    fn bind<'world>(columns: &mut Columns<'world>) -> Option<Self::State<'world>>;
    fn item<'row>(state: &'row mut Self::State<'_>, entity: Entity) -> Option<Self::Item<'row>>;
}

fn shared<'world, T: 'static>(
    columns: &Columns<'world>,
) -> Option<(SparseIndex<'world>, &'world [T])> {
    let column = columns
        .iter()
        .flatten()
        .find(|column| column.id == TypeId::of::<T>())?;
    Some((
        column.index,
        column.shared?.downcast_ref::<Vec<T>>()?.as_slice(),
    ))
}

impl<T: Send + Sync + 'static> Field for &T {
    type State<'world> = (SparseIndex<'world>, &'world [T]);
    type Item<'row> = &'row T;
    fn request() -> Request {
        Request::of::<T>(Access::Read)
    }
    fn bind<'world>(columns: &mut Columns<'world>) -> Option<Self::State<'world>> {
        shared::<T>(columns)
    }
    fn item<'row>(state: &'row mut Self::State<'_>, entity: Entity) -> Option<Self::Item<'row>> {
        state.1.get(state.0.position(entity)?)
    }
}

impl<T: Send + Sync + 'static> Field for Option<&T> {
    type State<'world> = Option<(SparseIndex<'world>, &'world [T])>;
    type Item<'row> = Option<&'row T>;
    fn request() -> Request {
        Request::of::<T>(Access::Optional)
    }
    fn bind<'world>(columns: &mut Columns<'world>) -> Option<Self::State<'world>> {
        Some(shared::<T>(columns))
    }
    fn item<'row>(state: &'row mut Self::State<'_>, entity: Entity) -> Option<Self::Item<'row>> {
        Some(state.and_then(|(index, values)| values.get(index.position(entity)?)))
    }
}

impl<T: Send + Sync + 'static> Field for &mut T {
    type State<'world> = (SparseIndex<'world>, &'world mut [T]);
    type Item<'row> = &'row mut T;
    fn request() -> Request {
        Request::of::<T>(Access::Write)
    }
    fn bind<'world>(columns: &mut Columns<'world>) -> Option<Self::State<'world>> {
        let column = columns
            .iter_mut()
            .flatten()
            .find(|column| column.id == TypeId::of::<T>())?;
        let values = column.exclusive.take()?.downcast_mut::<Vec<T>>()?;
        Some((column.index, values.as_mut_slice()))
    }
    fn item<'row>(state: &'row mut Self::State<'_>, entity: Entity) -> Option<Self::Item<'row>> {
        state.1.get_mut(state.0.position(entity)?)
    }
}

impl<T: Field> Fetch for T {
    type State<'world> = T::State<'world>;
    type Item<'row> = T::Item<'row>;
    fn requests() -> Requests {
        [Some(T::request()), None, None, None]
    }
    fn prepare<'world>(columns: &mut Columns<'world>) -> Option<Self::State<'world>> {
        T::bind(columns)
    }
    fn get<'row>(state: &'row mut Self::State<'_>, entity: Entity) -> Option<Self::Item<'row>> {
        T::item(state, entity)
    }
}

macro_rules! tuple {
    ($($field:ident => $state:ident),+) => {
        impl<$($field: Field),+> Fetch for ($($field,)+) {
            type State<'world> = ($($field::State<'world>,)+);
            type Item<'row> = ($($field::Item<'row>,)+);
            fn requests() -> Requests {
                let mut requests: Requests = [const { None }; 4];
                for (slot, request) in requests.iter_mut().zip([$($field::request()),+]) {
                    *slot = Some(request);
                }
                requests
            }
            fn prepare<'world>(columns: &mut Columns<'world>) -> Option<Self::State<'world>> {
                Some(($($field::bind(columns)?,)+))
            }
            fn get<'row>(state: &'row mut Self::State<'_>, entity: Entity) -> Option<Self::Item<'row>> {
                let ($($state,)+) = state;
                Some(($($field::item($state, entity)?,)+))
            }
        }
    };
}

tuple!(A => a);
tuple!(A => a, B => b);
tuple!(A => a, B => b, C => c);
tuple!(A => a, B => b, C => c, D => d);
