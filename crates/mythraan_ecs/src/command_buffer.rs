use crate::{Entity, World};

enum Command {
    Despawn(Entity),
    Remove(Entity, fn(&mut World, Entity)),
}

/// Queues structural changes for a world and applies them in submission order.
///
/// The buffer can be filled while a query borrows the world. Applying the
/// changes requires exclusive access to that world.
#[derive(Default)]
pub struct CommandBuffer {
    commands: Vec<Command>,
}

impl CommandBuffer {
    /// Creates an empty command buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues removal of an entity and its components.
    ///
    /// An entity that is already dead at flush time is ignored.
    pub fn despawn(&mut self, entity: Entity) {
        self.commands.push(Command::Despawn(entity));
    }

    /// Queues removal of a component, if it is present at flush time.
    pub fn remove<T: Send + Sync + 'static>(&mut self, entity: Entity) {
        self.commands.push(Command::Remove(entity, |world, entity| {
            drop(world.remove::<T>(entity));
        }));
    }

    /// Applies all queued changes, retaining the queue's capacity for reuse.
    pub fn flush(&mut self, world: &mut World) {
        for command in self.commands.drain(..) {
            match command {
                Command::Despawn(entity) => {
                    world.despawn(entity);
                }
                Command::Remove(entity, remove) => remove(world, entity),
            }
        }
    }
}
