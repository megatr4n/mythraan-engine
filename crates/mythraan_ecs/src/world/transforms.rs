use std::any::TypeId;
use std::mem;

use mythraan_math::transform::{GlobalTransform, Transform};

use super::World;
use crate::{Entity, WorldInsertError};

type Pending = (Entity, GlobalTransform, bool);

#[derive(Default)]
pub(super) struct TransformPropagation {
    pending: Vec<Pending>,
    last_tick: u64,
    locals_dirty: bool,
    pub(super) rebuild: bool,
}

impl TransformPropagation {
    pub(super) fn invalidate(&mut self, id: TypeId) {
        self.locals_dirty |= id == TypeId::of::<Transform>();
        self.rebuild |= id == TypeId::of::<GlobalTransform>();
    }
}

impl World {
    /// Updates the world-space transforms derived from local transforms.
    ///
    /// Call at a sync point after editing local transforms and parent links.
    /// Entities without a local transform forward their parent's transform.
    ///
    /// # Errors
    /// Returns an error if a derived component cannot be stored.
    pub fn propagate_transforms(&mut self) -> Result<(), WorldInsertError> {
        if !self.transforms.locals_dirty
            && !self.transforms.rebuild
            && self.hierarchy.dirty_subtrees.is_empty()
        {
            return Ok(());
        }
        let mut pending = mem::take(&mut self.transforms.pending);
        let result = self.update_dirty_transforms(&mut pending);
        pending.clear();
        self.transforms.pending = pending;
        if result.is_ok() {
            self.transforms.last_tick = self.change_tick();
            self.transforms.locals_dirty = false;
            self.transforms.rebuild = false;
            self.hierarchy.dirty_subtrees.clear();
        }
        result
    }

    fn update_dirty_transforms(
        &mut self,
        pending: &mut Vec<Pending>,
    ) -> Result<(), WorldInsertError> {
        let rebuild = self.transforms.rebuild;
        let checkpoint = self.transforms.last_tick;
        let standalone = self
            .storage::<Transform>()
            .into_iter()
            .flat_map(|storage| storage.entities().iter().copied())
            .filter(|&entity| self.parent(entity).is_none() && self.children(entity).is_none());
        pending.extend(
            self.hierarchy
                .roots()
                .chain(standalone)
                .map(|entity| (entity, GlobalTransform::IDENTITY, rebuild)),
        );

        while let Some((entity, parent, mut dirty)) = pending.pop() {
            dirty |= self.hierarchy.dirty_subtrees.contains(&entity);
            let mut global = parent;
            if let Some((&local, tick)) = self
                .storage::<Transform>()
                .and_then(|storage| storage.get_with_change_tick(entity))
            {
                let cached = self.get::<GlobalTransform>(entity).copied();
                // Include the last pass's tick because several passes can share one tick.
                if let Some(cached) = cached
                    && !dirty
                    && tick < checkpoint
                {
                    global = cached;
                } else {
                    global = parent.compose(local.into());
                    if cached != Some(global) {
                        self.insert(entity, global)?;
                        dirty = true;
                    }
                }
            }
            if let Some(children) = self.children(entity) {
                pending.extend(
                    children
                        .as_slice()
                        .iter()
                        .map(|&child| (child, global, dirty)),
                );
            }
        }
        Ok(())
    }
}
