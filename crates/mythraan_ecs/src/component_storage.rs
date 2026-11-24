use std::any::Any;

use crate::change_detection::ComponentTicks;
use crate::sparse_set::SparseIndex;
use crate::{Entity, SparseSet};

pub(crate) trait ComponentStorage: Any + Send + Sync {
    fn set_membership_id(&mut self, id: u32);
    fn remove_entity(&mut self, entity: Entity);
    fn query_parts_ref(&self) -> (SparseIndex<'_>, &[ComponentTicks], &(dyn Any + Send + Sync));
    fn query_parts(
        &mut self,
    ) -> (
        SparseIndex<'_>,
        &[ComponentTicks],
        &mut (dyn Any + Send + Sync),
    );
}

impl<T: Send + Sync + 'static> ComponentStorage for SparseSet<T> {
    fn set_membership_id(&mut self, id: u32) {
        self.membership_id = id;
    }

    fn remove_entity(&mut self, entity: Entity) {
        drop(self.remove(entity));
    }

    fn query_parts_ref(&self) -> (SparseIndex<'_>, &[ComponentTicks], &(dyn Any + Send + Sync)) {
        let (index, ticks, values) = self.query_parts_ref();
        (index, ticks, values)
    }

    fn query_parts(
        &mut self,
    ) -> (
        SparseIndex<'_>,
        &[ComponentTicks],
        &mut (dyn Any + Send + Sync),
    ) {
        let (index, ticks, values) = self.query_parts();
        (index, ticks, values)
    }
}
