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

    fn visit_dense(
        _state: &mut Self::State<'_>,
        _source: &[Entity],
        _all_rows: bool,
        _matches: &mut impl FnMut(Entity) -> bool,
        _visit: &mut impl for<'row> FnMut(Entity, Self::Item<'row>),
    ) -> bool {
        false
    }
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

    fn visit_dense(
        _state: &mut Self::State<'_>,
        _source: &[Entity],
        _all_rows: bool,
        _matches: &mut impl FnMut(Entity) -> bool,
        _visit: &mut impl for<'row> FnMut(Entity, Self::Item<'row>),
    ) -> bool {
        false
    }
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

struct ChangedPrefix<'a> {
    stamps: &'a [ComponentTicks],
    tick: u64,
    yielded: usize,
}

impl Drop for ChangedPrefix<'_> {
    fn drop(&mut self) {
        for stamp in self.stamps.iter().take(self.yielded) {
            stamp.mark_changed(self.tick);
        }
    }
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

    fn visit_dense(
        state: &mut Self::State<'_>,
        source: &[Entity],
        all_rows: bool,
        matches: &mut impl FnMut(Entity) -> bool,
        visit: &mut impl for<'row> FnMut(Entity, Self::Item<'row>),
    ) -> bool {
        // The selected driver must share this column's dense row order.
        if !std::ptr::eq(source, state.index.entities())
            || source.len() != state.values.len()
            || source.len() != state.ticks.len()
        {
            return false;
        }

        let tick = state.tick;
        if all_rows {
            let mut prefix = ChangedPrefix {
                stamps: state.ticks,
                tick,
                yielded: 0,
            };
            for (entity, value) in source.iter().copied().zip(&mut *state.values) {
                // Count this row before calling the visitor so unwinding stamps it too.
                prefix.yielded += 1;
                visit(entity, value);
            }
            return true;
        }

        for ((entity, value), stamp) in source
            .iter()
            .copied()
            .zip(&mut *state.values)
            .zip(state.ticks)
        {
            if matches(entity) {
                stamp.mark_changed(tick);
                visit(entity, value);
            }
        }
        true
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

    fn visit_dense(
        state: &mut Self::State<'_>,
        source: &[Entity],
        all_rows: bool,
        matches: &mut impl FnMut(Entity) -> bool,
        visit: &mut impl for<'row> FnMut(Entity, Self::Item<'row>),
    ) -> bool {
        T::visit_dense(state, source, all_rows, matches, visit)
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
