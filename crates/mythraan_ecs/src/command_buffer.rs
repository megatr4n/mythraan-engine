use std::sync::Arc;

use crate::{CommandError, Entity, World, WorldInsertError};

type Insert = dyn FnOnce(&mut World, Entity) -> Result<(), WorldInsertError> + Send + Sync;

enum Command {
    Spawn,
    Despawn(Target),
    Insert(Target, Box<Insert>),
    Remove(Target, fn(&mut World, Entity)),
}

impl Command {
    fn apply(
        self,
        world: &mut World,
        spawned: &mut Vec<Option<Entity>>,
    ) -> Result<(), CommandError> {
        match self {
            Self::Spawn => {
                let result = world.spawn();
                spawned.push(result.as_ref().ok().copied());
                result?;
            }
            Self::Despawn(target) => {
                world.despawn(target.resolve(spawned)?);
            }
            Self::Insert(target, insert) => insert(world, target.resolve(spawned)?)?,
            Self::Remove(target, remove) => remove(world, target.resolve(spawned)?),
        }
        Ok(())
    }
}

enum Target {
    Entity(Entity),
    Deferred(usize),
    ForeignPlaceholder,
}

impl Target {
    fn resolve(self, spawned: &[Option<Entity>]) -> Result<Entity, CommandError> {
        match self {
            Self::Entity(entity) => Ok(entity),
            Self::Deferred(index) => spawned
                .get(index)
                .copied()
                .flatten()
                .ok_or(CommandError::UnresolvedPlaceholder),
            Self::ForeignPlaceholder => Err(CommandError::ForeignPlaceholder),
        }
    }
}

/// A placeholder for an entity queued by [`CommandBuffer::spawn`].
///
/// The placeholder belongs to one batch in the buffer that issued it.
/// Pass it by reference when queuing another command in that batch.
/// Obtain its entity identifier from that batch's [`FlushReport`].
#[derive(Clone, Debug)]
pub struct DeferredEntity {
    batch: Arc<()>,
    index: usize,
}

impl DeferredEntity {
    fn belongs_to(&self, batch: Option<&Arc<()>>) -> bool {
        batch.is_some_and(|batch| Arc::ptr_eq(batch, &self.batch))
    }
}

/// The entity addressed by a command.
#[derive(Clone, Copy, Debug)]
pub enum CommandTarget<'entity> {
    /// An existing entity from the world being flushed.
    Entity(Entity),
    /// An entity queued for creation in the current batch.
    Deferred(&'entity DeferredEntity),
}

impl From<Entity> for CommandTarget<'_> {
    fn from(entity: Entity) -> Self {
        Self::Entity(entity)
    }
}

impl<'entity> From<&'entity DeferredEntity> for CommandTarget<'entity> {
    fn from(entity: &'entity DeferredEntity) -> Self {
        Self::Deferred(entity)
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
#[derive(Debug)]
#[must_use = "inspect the report for failed structural commands"]
pub struct FlushReport {
    batch: Option<Arc<()>>,
    spawned: Vec<Option<Entity>>,
    failures: Vec<CommandFailure>,
}

impl FlushReport {
    /// Returns the identifier issued for a successful spawn from this batch.
    ///
    /// A later command in the batch may have already despawned the entity.
    #[must_use]
    pub fn resolve(&self, entity: &DeferredEntity) -> Option<Entity> {
        if entity.belongs_to(self.batch.as_ref()) {
            self.spawned.get(entity.index).copied().flatten()
        } else {
            None
        }
    }

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
/// Commands can target an existing entity or a placeholder returned by this
/// buffer in the current batch. Invalid placeholders are reported at flush.
///
/// ```
/// use mythraan_ecs::prelude::{CommandBuffer, World};
///
/// let mut world = World::new();
/// let source = world.spawn()?;
/// world.insert(source, String::from("example"))?;
///
/// let mut commands = CommandBuffer::new();
/// let mut copies = Vec::new();
/// for (entity, name) in world.query::<&String>() {
///     let copy = commands.spawn();
///     commands.insert(&copy, name.clone());
///     commands.despawn(entity);
///     copies.push(copy);
/// }
///
/// let report = commands.flush(&mut world);
/// assert!(report.failures().is_empty());
/// assert!(!world.is_alive(source));
/// let name = copies.first()
///     .and_then(|copy| report.resolve(copy))
///     .and_then(|id| world.get::<String>(id));
/// assert_eq!(name.map(String::as_str), Some("example"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Default)]
pub struct CommandBuffer {
    commands: Vec<Command>,
    batch: Option<Arc<()>>,
    spawn_count: usize,
}

/// A system's queue for submitting deferred structural changes.
pub struct Commands<'queue>(&'queue mut CommandBuffer);

impl<'queue> Commands<'queue> {
    /// Borrows the command queue assigned to the system.
    #[must_use]
    pub fn new(buffer: &'queue mut CommandBuffer) -> Self {
        Self(buffer)
    }

    /// Queues creation of an entity and returns its placeholder.
    pub fn spawn(&mut self) -> DeferredEntity {
        self.0.spawn()
    }

    /// Queues removal of an entity and its components.
    pub fn despawn<'entity>(&mut self, entity: impl Into<CommandTarget<'entity>>) {
        self.0.despawn(entity);
    }

    /// Queues insertion or replacement of a component.
    pub fn insert<'entity, T: Send + Sync + 'static>(
        &mut self,
        entity: impl Into<CommandTarget<'entity>>,
        component: T,
    ) {
        self.0.insert(entity, component);
    }

    /// Queues removal of a component.
    pub fn remove<'entity, T: Send + Sync + 'static>(
        &mut self,
        entity: impl Into<CommandTarget<'entity>>,
    ) {
        self.0.remove::<T>(entity);
    }
}

impl CommandBuffer {
    /// Creates an empty command buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Queues creation of an empty entity and returns its placeholder.
    pub fn spawn(&mut self) -> DeferredEntity {
        let batch = self.batch.get_or_insert_with(|| Arc::new(()));
        let entity = DeferredEntity {
            batch: Arc::clone(batch),
            index: self.spawn_count,
        };
        self.spawn_count += 1;
        self.commands.push(Command::Spawn);
        entity
    }

    /// Queues removal of an entity and its components.
    ///
    /// An entity that is already dead at flush time is ignored.
    pub fn despawn<'entity>(&mut self, entity: impl Into<CommandTarget<'entity>>) {
        let target = self.target(entity.into());
        self.commands.push(Command::Despawn(target));
    }

    /// Queues insertion or replacement of a component.
    ///
    /// The buffer owns the value until it is applied or discarded.
    pub fn insert<'entity, T: Send + Sync + 'static>(
        &mut self,
        entity: impl Into<CommandTarget<'entity>>,
        component: T,
    ) {
        let target = self.target(entity.into());
        self.commands.push(Command::Insert(
            target,
            Box::new(move |world, entity| world.insert(entity, component).map(drop)),
        ));
    }

    /// Queues removal of a component, if it is present at flush time.
    pub fn remove<'entity, T: Send + Sync + 'static>(
        &mut self,
        entity: impl Into<CommandTarget<'entity>>,
    ) {
        let target = self.target(entity.into());
        self.commands.push(Command::Remove(target, |world, entity| {
            drop(world.remove::<T>(entity));
        }));
    }

    /// Applies all queued changes, retaining the queue's capacity for reuse.
    ///
    /// Failed commands are reported and later commands are still attempted.
    /// The queue is emptied, and completed changes stay applied. Use entity
    /// identifiers from the world being flushed.
    /// If a component destructor unwinds, the remaining commands are discarded.
    pub fn flush(&mut self, world: &mut World) -> FlushReport {
        let mut report = FlushReport {
            batch: self.batch.take(),
            spawned: Vec::with_capacity(std::mem::take(&mut self.spawn_count)),
            failures: Vec::new(),
        };
        for (index, command) in self.commands.drain(..).enumerate() {
            if let Err(error) = command.apply(world, &mut report.spawned) {
                report.failures.push(CommandFailure { index, error });
            }
        }
        report
    }

    fn target(&self, target: CommandTarget<'_>) -> Target {
        match target {
            CommandTarget::Entity(entity) => Target::Entity(entity),
            CommandTarget::Deferred(entity) if entity.belongs_to(self.batch.as_ref()) => {
                Target::Deferred(entity.index)
            }
            CommandTarget::Deferred(_) => Target::ForeignPlaceholder,
        }
    }
}
