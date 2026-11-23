use std::mem;

use rayon::prelude::*;

use crate::{
    CommandBuffer, Commands, ConflictGraph, FlushReport, ScheduleError, ScopeError, SystemAccess,
    SystemScope, World,
};

type Callback = dyn for<'world, 'queue> FnMut(
        SystemScope<'world>,
        Option<Commands<'queue>>,
    ) -> Result<(), ScopeError>
    + Send
    + Sync;

struct RegisteredSystem {
    index: usize,
    batch: usize,
    callback: Box<Callback>,
    commands: Option<CommandBuffer>,
}

impl RegisteredSystem {
    fn execute(&mut self, scope: SystemScope<'_>) -> Result<Option<CommandBuffer>, ScheduleError> {
        let mut commands = self.commands.as_mut().map(mem::take);
        (self.callback)(scope, commands.as_mut().map(Commands::new)).map_err(|source| {
            ScheduleError::System {
                system: self.index,
                source,
            }
        })?;
        Ok(commands)
    }
}

/// The commands applied for one system at the schedule's sync point.
#[derive(Debug)]
pub struct SystemFlush {
    /// The registration index returned by [`Schedule::add_system`].
    pub system: usize,
    /// Resolved spawns and failed commands from this system's queue.
    pub report: FlushReport,
}

/// Runs systems in conflict-free batches using the current Rayon pool.
///
/// Conflicting systems run in registration order. Queued structural changes
/// are applied after all batches finish successfully.
#[derive(Default)]
pub struct Schedule {
    accesses: Vec<SystemAccess>,
    systems: Vec<RegisteredSystem>,
    batches_dirty: bool,
}

impl Schedule {
    /// Creates an empty schedule.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a system and returns its zero-based registration index.
    ///
    /// Declare storage with [`SystemAccess::of`]. The callback receives a
    /// command queue only when the declaration includes [`Commands`].
    pub fn add_system(
        &mut self,
        access: SystemAccess,
        callback: impl for<'world, 'queue> FnMut(
            SystemScope<'world>,
            Option<Commands<'queue>>,
        ) -> Result<(), ScopeError>
        + Send
        + Sync
        + 'static,
    ) -> usize {
        let index = self.accesses.len();
        let commands = access.uses_commands().then(CommandBuffer::new);
        self.accesses.push(access);
        self.systems.push(RegisteredSystem {
            index,
            batch: 0,
            callback: Box::new(callback),
            commands,
        });
        self.batches_dirty = true;
        index
    }

    /// Runs the schedule and applies nonempty queues in registration order.
    ///
    /// Systems in later batches can see earlier component and resource
    /// writes. Structural commands become visible at the end of the run.
    ///
    /// # Errors
    ///
    /// If a system returns an access error, the first error in batch then
    /// registration order is returned. Pending commands are discarded.
    /// Direct writes already made by systems stay applied.
    pub fn run(&mut self, world: &mut World) -> Result<Vec<SystemFlush>, ScheduleError> {
        self.prepare_batches();
        let accesses = &self.accesses;
        let mut pending = Vec::new();
        let mut completed = Vec::<Result<_, ScheduleError>>::new();
        for batch in self.systems.chunk_by_mut(|a, b| a.batch == b.batch) {
            let declared = batch.iter().filter_map(|system| accesses.get(system.index));
            let scopes = world.scope_batch(declared)?;
            batch
                .par_iter_mut()
                .zip(scopes)
                .map(|(system, scope)| {
                    let commands = system.execute(scope)?;
                    Ok((system, commands))
                })
                .collect_into_vec(&mut completed);
            for result in completed.drain(..) {
                let (system, commands) = result?;
                if let Some(commands) = commands {
                    pending.push((system, commands));
                }
            }
        }

        pending.sort_unstable_by_key(|(system, _)| system.index);
        let mut flushed = Vec::new();
        for (system, mut commands) in pending {
            if !commands.is_empty() {
                flushed.push(SystemFlush {
                    system: system.index,
                    report: commands.flush(world),
                });
            }
            system.commands = Some(commands);
        }
        Ok(flushed)
    }

    fn prepare_batches(&mut self) {
        if !self.batches_dirty {
            return;
        }
        let graph = ConflictGraph::new(&self.accesses);
        self.systems.sort_unstable_by_key(|system| system.index);
        for (number, batch) in graph.batches().enumerate() {
            for &index in batch {
                if let Some(system) = self.systems.get_mut(index) {
                    system.batch = number;
                }
            }
        }
        self.systems
            .sort_unstable_by_key(|system| (system.batch, system.index));
        self.batches_dirty = false;
    }
}
