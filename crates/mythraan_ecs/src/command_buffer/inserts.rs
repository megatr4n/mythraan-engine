use std::any::{Any, TypeId};
use std::collections::{BTreeMap, VecDeque};

use crate::{Entity, World, WorldInsertError};

trait Column: Any + Send + Sync {
    fn apply_next(&mut self, world: &mut World, entity: Entity) -> Result<(), WorldInsertError>;
    fn discard_next(&mut self);
}

impl<T: Send + Sync + 'static> Column for VecDeque<T> {
    fn apply_next(&mut self, world: &mut World, entity: Entity) -> Result<(), WorldInsertError> {
        if let Some(component) = self.pop_front() {
            drop(world.insert(entity, component)?);
        }
        Ok(())
    }

    fn discard_next(&mut self) {
        drop(self.pop_front());
    }
}

/// Holds one typed FIFO per component type. Command headers carry the column
/// index and consume one value in submission order, including on rejection.
#[derive(Default)]
pub(super) struct Inserts {
    indices: BTreeMap<TypeId, usize>,
    columns: Vec<Box<dyn Column>>,
}

impl Inserts {
    pub(super) fn push<T: Send + Sync + 'static>(&mut self, component: T) -> usize {
        let id = TypeId::of::<T>();
        if let Some(&index) = self.indices.get(&id)
            && let Some(column) = self.columns.get_mut(index)
            && let Some(values) = (column.as_mut() as &mut dyn Any).downcast_mut::<VecDeque<T>>()
        {
            values.push_back(component);
            return index;
        }

        let index = self.columns.len();
        self.columns.push(Box::new(VecDeque::from([component])));
        self.indices.insert(id, index);
        index
    }

    pub(super) fn apply_next(
        &mut self,
        index: usize,
        world: &mut World,
        entity: Entity,
    ) -> Result<(), WorldInsertError> {
        if let Some(column) = self.columns.get_mut(index) {
            column.apply_next(world, entity)?;
        }
        Ok(())
    }

    pub(super) fn discard_next(&mut self, index: usize) {
        if let Some(column) = self.columns.get_mut(index) {
            column.discard_next();
        }
    }
}
