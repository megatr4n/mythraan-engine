use crate::{Entity, EntityAllocError, EntityAllocator};

/// Owns the entities in a simulation.
///
/// Use entity identifiers with the world that issued them.
#[derive(Default)]
pub struct World {
    entities: EntityAllocator,
}

impl World {
    /// Creates an empty world.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an entity with no components.
    ///
    /// # Errors
    ///
    /// Returns an error if the allocator cannot issue another identifier.
    pub fn spawn(&mut self) -> Result<Entity, EntityAllocError> {
        self.entities.spawn()
    }

    /// Returns whether this entity identifier is live in the world.
    #[must_use]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }
}
