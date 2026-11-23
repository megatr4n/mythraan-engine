use std::any::TypeId;

use super::{Access, Columns, Request, Requests, requests};
use crate::Entity;
use crate::change_detection::ComponentTicks;
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
    fn item<'row>(
        state: &'row mut Self::State<'_>,
        entity: Entity,
    ) -> Option<RowItem<'row, Self::Item<'row>>>;
}

pub struct RowItem<'row, T> {
    value: T,
    stamp: Option<(&'row ComponentTicks, u64)>,
}

impl<T> RowItem<'_, T> {
    fn shared(value: T) -> Self {
        Self { value, stamp: None }
    }

    fn into_item(self) -> T {
        if let Some((ticks, tick)) = self.stamp {
            ticks.mark_changed(tick);
        }
        self.value
    }
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
    fn item<'row>(
        state: &'row mut Self::State<'_>,
        entity: Entity,
    ) -> Option<RowItem<'row, Self::Item<'row>>> {
        state.1.get(state.0.position(entity)?).map(RowItem::shared)
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
    fn item<'row>(
        state: &'row mut Self::State<'_>,
        entity: Entity,
    ) -> Option<RowItem<'row, Self::Item<'row>>> {
        let value = state.and_then(|(index, values)| values.get(index.position(entity)?));
        Some(RowItem::shared(value))
    }
}

pub struct WriteState<'world, T> {
    index: SparseIndex<'world>,
    values: &'world mut [T],
    ticks: &'world [ComponentTicks],
    tick: u64,
}

impl<T: Send + Sync + 'static> Field for &mut T {
    type State<'world> = WriteState<'world, T>;
    type Item<'row> = &'row mut T;
    fn request() -> Request {
        Request::of::<T>(Access::Write)
    }
    fn bind<'world>(columns: &mut Columns<'world>) -> Option<Self::State<'world>> {
        let column = columns
            .iter_mut()
            .flatten()
            .find(|column| column.id == TypeId::of::<T>())?;
        let (values, tick) = column.exclusive.take()?;
        let values = values.downcast_mut::<Vec<T>>()?;
        Some(WriteState {
            index: column.index,
            values: values.as_mut_slice(),
            ticks: column.ticks,
            tick,
        })
    }
    fn item<'row>(
        state: &'row mut Self::State<'_>,
        entity: Entity,
    ) -> Option<RowItem<'row, Self::Item<'row>>> {
        let position = state.index.position(entity)?;
        Some(RowItem {
            value: state.values.get_mut(position)?,
            stamp: state.ticks.get(position).map(|ticks| (ticks, state.tick)),
        })
    }
}

impl<T: Field> Fetch for T {
    type State<'world> = T::State<'world>;
    type Item<'row> = T::Item<'row>;
    fn requests() -> Requests {
        requests([T::request()])
    }
    fn prepare<'world>(columns: &mut Columns<'world>) -> Option<Self::State<'world>> {
        T::bind(columns)
    }
    fn get<'row>(state: &'row mut Self::State<'_>, entity: Entity) -> Option<Self::Item<'row>> {
        Some(T::item(state, entity)?.into_item())
    }
}

macro_rules! tuple {
    ($($field:ident => $state:ident),+) => {
        impl<$($field: Field),+> Fetch for ($($field,)+) {
            type State<'world> = ($($field::State<'world>,)+);
            type Item<'row> = ($($field::Item<'row>,)+);
            fn requests() -> Requests {
                requests([$($field::request()),+])
            }
            fn prepare<'world>(columns: &mut Columns<'world>) -> Option<Self::State<'world>> {
                Some(($($field::bind(columns)?,)+))
            }
            fn get<'row>(state: &'row mut Self::State<'_>, entity: Entity) -> Option<Self::Item<'row>> {
                let ($($state,)+) = state;
                $(let $state = $field::item($state, entity)?;)+
                Some(($($state.into_item(),)+))
            }
        }
    };
}

tuple!(A => a);
tuple!(A => a, B => b);
tuple!(A => a, B => b, C => c);
tuple!(A => a, B => b, C => c, D => d);
