use mythraan_math::transform::{GlobalTransform, Transform};

use super::World;
use crate::WorldInsertError;

impl World {
    /// Updates the world-space transforms derived from local transforms.
    ///
    /// Call at a sync point after editing local transforms and parent links.
    /// Entities without a local transform forward their parent's transform.
    ///
    /// # Errors
    /// Returns an error if a derived component cannot be stored.
    pub fn propagate_transforms(&mut self) -> Result<(), WorldInsertError> {
        let mut pending = Vec::new();
        let standalone = self
            .storage::<Transform>()
            .into_iter()
            .flat_map(|storage| storage.entities().iter().copied())
            .filter(|&entity| self.parent(entity).is_none() && self.children(entity).is_none());
        pending.extend(
            self.hierarchy
                .roots()
                .chain(standalone)
                .map(|entity| (entity, GlobalTransform::IDENTITY)),
        );

        while let Some((entity, parent)) = pending.pop() {
            let mut global = parent;
            if let Some(&local) = self.get::<Transform>(entity) {
                global = parent.compose(local.into());
                if self.get::<GlobalTransform>(entity) != Some(&global) {
                    self.insert(entity, global)?;
                }
            }
            if let Some(children) = self.children(entity) {
                pending.extend(children.as_slice().iter().map(|&child| (child, global)));
            }
        }
        Ok(())
    }
}
