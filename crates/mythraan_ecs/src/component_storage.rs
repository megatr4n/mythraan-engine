use std::any::Any;

use crate::{Entity, SparseSet};

pub(crate) trait ComponentStorage: Any + Send + Sync {
    fn remove_entity(&mut self, entity: Entity);
}

impl<T: Send + Sync + 'static> ComponentStorage for SparseSet<T> {
    fn remove_entity(&mut self, entity: Entity) {
        drop(self.remove(entity));
    }
}
