use std::any::TypeId;
use std::collections::{BTreeMap, TryReserveError};

use crate::Entity;
use crate::component_storage::ComponentStorage;

// Churn measurements used a host with 4 MiB of L3 and about 40 MB of sparse
// slots at 50 types. A larger cache can reduce the benefit of indexing.
pub(super) const MEMBERSHIP_THRESHOLD: usize = 32;
const WORD_BITS: usize = u64::BITS as usize;

#[cfg(test)]
thread_local! {
    pub(super) static FAIL_NEXT_WORD_RESERVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[derive(Default)]
pub(super) struct Membership {
    types: Vec<TypeId>,
    // Each plane stores one word per entity index and covers 64 storage IDs.
    planes: Vec<Vec<u64>>,
    // Despawn preserves the scanning world's TypeId destructor order.
    pending: Vec<(TypeId, u32)>,
}

impl Membership {
    pub(super) fn build(
        storages: &BTreeMap<TypeId, Box<dyn ComponentStorage>>,
        new_type: TypeId,
        entity: Entity,
    ) -> Result<Self, TryReserveError> {
        let mut membership = Self::default();
        membership.types.try_reserve_exact(storages.len() + 1)?;
        membership.pending.try_reserve_exact(storages.len() + 1)?;
        let max_row = storages
            .values()
            .flat_map(|storage| storage.query_parts_ref().0.entities())
            .map(|entity| entity.index())
            .fold(entity.index(), u32::max);
        membership.prepare(max_row as usize, 0)?;
        for (&ty, storage) in storages {
            let id = membership.types.len() as u32;
            membership.types.push(ty);
            for &owner in storage.query_parts_ref().0.entities() {
                membership.add(owner.index() as usize, id);
            }
        }
        membership.types.push(new_type);
        membership.add(entity.index() as usize, storages.len() as u32);
        Ok(membership)
    }

    pub(super) fn next_id(&self) -> usize {
        self.types.len()
    }

    pub(super) fn prepare_type(&mut self, row: usize, id: u32) -> Result<(), TryReserveError> {
        self.types.try_reserve(1)?;
        // Reserve before registration so cleanup never allocates while dropping components.
        self.pending
            .try_reserve(self.types.len() + 1 - self.pending.len())?;
        self.prepare(row, id)
    }

    pub(super) fn register(&mut self, ty: TypeId) {
        self.types.push(ty);
    }

    pub(super) fn prepare(&mut self, row: usize, id: u32) -> Result<(), TryReserveError> {
        let plane = id as usize / WORD_BITS;
        if self.planes.len() <= plane {
            self.planes.try_reserve(plane + 1 - self.planes.len())?;
            self.planes.resize_with(plane + 1, Vec::new);
        }
        let words = &mut self.planes[plane];
        if words.len() <= row {
            let additional = row + 1 - words.len();
            #[cfg(test)]
            let additional = if FAIL_NEXT_WORD_RESERVE.replace(false) {
                usize::MAX
            } else {
                additional
            };
            words.try_reserve(additional)?;
            words.resize(row + 1, 0);
        }
        Ok(())
    }

    pub(super) fn add(&mut self, row: usize, id: u32) {
        self.planes[id as usize / WORD_BITS][row] |= 1 << (id as usize % WORD_BITS);
    }

    pub(super) fn remove(&mut self, row: usize, id: u32) {
        self.planes[id as usize / WORD_BITS][row] &= !(1 << (id as usize % WORD_BITS));
    }

    pub(super) fn despawn(
        &mut self,
        storages: &mut BTreeMap<TypeId, Box<dyn ComponentStorage>>,
        entity: Entity,
    ) {
        let row = entity.index() as usize;
        self.pending.clear();
        for (plane, words) in self.planes.iter().enumerate() {
            let mut bits = words.get(row).copied().unwrap_or(0);
            while bits != 0 {
                let id = plane * WORD_BITS + bits.trailing_zeros() as usize;
                self.pending.push((self.types[id], id as u32));
                bits &= bits - 1;
            }
        }
        self.pending.sort_unstable_by_key(|&(ty, _)| ty);
        for index in 0..self.pending.len() {
            let (ty, id) = self.pending[index];
            // The dense removal precedes the destructor, which can unwind.
            self.remove(row, id);
            if let Some(storage) = storages.get_mut(&ty) {
                storage.remove_entity(entity);
            }
        }
        // Every word is now zero. A recycled slot starts with empty membership.
    }
}

#[cfg(test)]
impl Membership {
    pub(super) fn assert_matches(
        &self,
        storages: &BTreeMap<TypeId, Box<dyn ComponentStorage>>,
        entity: Entity,
    ) {
        assert_eq!(self.types.len(), storages.len());
        for (id, ty) in self.types.iter().enumerate() {
            let word = self.planes[id / WORD_BITS]
                .get(entity.index() as usize)
                .copied()
                .unwrap_or(0);
            let has_bit = word & (1 << (id % WORD_BITS)) != 0;
            assert_eq!(has_bit, storages[ty].query_parts_ref().0.contains(entity));
        }
    }
}
