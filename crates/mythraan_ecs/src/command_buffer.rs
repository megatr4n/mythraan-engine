use crate::{CommandError, Entity, World, WorldInsertError};

type Insert = dyn FnOnce(&mut World, Entity) -> Result<(), WorldInsertError> + Send + Sync;

enum Command {
    Despawn(Entity),
    Insert(Entity, Box<Insert>),
    Remove(Entity, fn(&mut World, Entity)),
}

impl Command {
    fn apply(self, world: &mut World) -> Result<(), CommandError> {
        match self {
            Self::Despawn(entity) => {
                world.despawn(entity);
            }
            Self::Insert(entity, insert) => insert(world, entity)?,
            Self::Remove(entity, remove) => remove(world, entity),
        }
        Ok(())
    }
}

/// A failed command and its position in the flushed queue.
#[derive(Debug)]
pub struct CommandFailure {
    /// Zero-based command index in the flush.
    pub index: usize,
    /// Reason the command failed.
    pub error: CommandError,
}

/// The outcome of applying a batch of structural commands.
#[derive(Debug, Default)]
#[must_use = "inspect the report for failed structural commands"]
pub struct FlushReport {
    failures: Vec<CommandFailure>,
}

impl FlushReport {
    /// Returns command failures in submission order.
    #[must_use]
    pub fn failures(&self) -> &[CommandFailure] {
        &self.failures
    }
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

    /// Queues insertion or replacement of a component.
    ///
    /// The buffer owns the value until it is applied or discarded.
    pub fn insert<T: Send + Sync + 'static>(&mut self, entity: Entity, component: T) {
        self.commands.push(Command::Insert(
            entity,
            Box::new(move |world, entity| world.insert(entity, component).map(drop)),
        ));
    }

    /// Queues removal of a component, if it is present at flush time.
    pub fn remove<T: Send + Sync + 'static>(&mut self, entity: Entity) {
        self.commands.push(Command::Remove(entity, |world, entity| {
            drop(world.remove::<T>(entity));
        }));
    }

    /// Applies all queued changes, retaining the queue's capacity for reuse.
    ///
    /// Failed commands are reported and later commands are still attempted.
    /// The queue is emptied, and completed changes stay applied. Use entity
    /// identifiers from the world being flushed.
    pub fn flush(&mut self, world: &mut World) -> FlushReport {
        let mut report = FlushReport::default();
        for (index, command) in self.commands.drain(..).enumerate() {
            if let Err(error) = command.apply(world) {
                report.failures.push(CommandFailure { index, error });
            }
        }
        report
    }
}
