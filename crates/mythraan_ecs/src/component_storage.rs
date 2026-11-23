use std::any::Any;

use crate::sparse_set::SparseIndex;
use crate::{Entity, SparseSet};

pub(crate) trait ComponentStorage: Any + Send + Sync {
    fn remove_entity(&mut self, entity: Entity);
    fn query_parts(&mut self) -> (SparseIndex<'_>, &mut (dyn Any + Send + Sync));
}

impl<T: Send + Sync + 'static> ComponentStorage for SparseSet<T> {
    fn remove_entity(&mut self, entity: Entity) {
        drop(self.remove(entity));
    }

    fn query_parts(&mut self) -> (SparseIndex<'_>, &mut (dyn Any + Send + Sync)) {
        let (index, values) = self.query_parts();
        (index, values)
    }
}
