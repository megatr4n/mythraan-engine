use std::mem;

use rayon::prelude::*;

use crate::{
    CommandBuffer, Commands, ConflictGraph, FlushReport, ScheduleConfigError, ScheduleError,
    ScopeError, SystemAccess, SystemScope, World,
};

mod order;
use order::OrderPlan;
pub use order::{OrderTarget, SystemOrder};

type Callback = dyn for<'world, 'queue> FnMut(
        SystemScope<'world>,
        Option<Commands<'queue>>,
    ) -> Result<(), ScopeError>
    + Send
    + Sync;

type RunCondition = dyn FnMut(&World) -> bool + Send + Sync;

struct RegisteredSystem {
    index: usize,
    batch: usize,
    callback: Box<Callback>,
    commands: Option<CommandBuffer>,
    condition: Option<Box<RunCondition>>,
    enabled: bool,
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
/// Conflicting systems default to registration order. Queued structural changes
/// are applied after all batches finish successfully.
///
/// ```
/// use mythraan_ecs::prelude::{Commands, ResMut, Schedule, SystemAccess, World};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// # rayon::ThreadPoolBuilder::new().num_threads(2)
/// #     .build_scoped(|thread| thread.run(), |pool| pool.install(|| {
/// let mut world = World::new();
/// world.resources_mut().insert(0_u32);
/// let mut schedule = Schedule::new();
/// schedule.add_system(
///     SystemAccess::of::<(ResMut<'_, u32>, Commands<'_>)>(),
///     |mut scope, commands| {
///         if let Some(mut count) = scope.resources.get_mut::<u32>()? {
///             *count += 1;
///         }
///         if let Some(mut commands) = commands {
///             let spawned = commands.spawn();
///             commands.insert(&spawned, true);
///         }
///         Ok(())
///     },
/// );
/// let reports = schedule.run(&mut world)?;
/// assert_eq!(world.resources().get::<u32>(), Some(&1));
/// assert_eq!(world.query::<&bool>().count(), 1);
/// assert!(reports.iter().all(|entry| entry.report.failures().is_empty()));
/// # Ok(())
/// # }))?
/// # }
/// ```
#[derive(Default)]
pub struct Schedule {
    accesses: Vec<SystemAccess>,
    systems: Vec<RegisteredSystem>,
    batches_dirty: bool,
    order: Option<OrderPlan>,
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
            condition: None,
            enabled: true,
        });
        self.batches_dirty = true;
        index
    }

    /// Replaces the explicit ordering constraints for this schedule.
    ///
    /// Systems registered later are unconstrained until configured again.
    /// The explicit order takes precedence over default ordering of conflicting systems.
    /// Deferred command queues are still flushed in registration order.
    ///
    /// # Errors
    ///
    /// Unknown systems, unknown sets or a cycle leave the previous ordering in effect.
    pub fn configure_order(&mut self, order: &SystemOrder) -> Result<(), ScheduleConfigError> {
        self.order = Some(order.compile(self.systems.len())?);
        self.batches_dirty = true;
        Ok(())
    }

    /// Sets or replaces a system's run condition.
    ///
    /// Conditions read the world before any system in their batch runs.
    /// A skipped system keeps its ordering constraints.
    ///
    /// # Errors
    ///
    /// Returns an error if the system index is unknown.
    pub fn run_if(
        &mut self,
        index: usize,
        condition: impl FnMut(&World) -> bool + Send + Sync + 'static,
    ) -> Result<(), ScheduleConfigError> {
        let system = self
            .systems
            .iter_mut()
            .find(|system| system.index == index)
            .ok_or(ScheduleConfigError::UnknownSystem(index))?;
        system.condition = Some(Box::new(condition));
        Ok(())
    }

    /// Runs the schedule and applies nonempty queues in registration order.
    ///
    /// Systems in later batches can see earlier component and resource
    /// writes. Structural commands become visible at the end of the run.
    /// Command failures are recorded in the returned reports.
    /// The caller controls the change tick through [`World::advance_change_tick`].
    ///
    /// # Errors
    ///
    /// If a system returns an access error, the first error in batch then
    /// registration order is returned. Pending commands are discarded.
    /// Direct writes already made by systems stay applied.
    ///
    /// # Panics
    ///
    /// A panic in a system or its run condition propagates before any queued commands are applied.
    pub fn run(&mut self, world: &mut World) -> Result<Vec<SystemFlush>, ScheduleError> {
        self.prepare_batches();
        let accesses = &self.accesses;
        let empty_access = SystemAccess::default();
        let mut pending = Vec::new();
        let mut completed = Vec::<Result<_, ScheduleError>>::new();
        for batch in self.systems.chunk_by_mut(|a, b| a.batch == b.batch) {
            for system in batch.iter_mut() {
                if let Some(condition) = system.condition.as_mut() {
                    system.enabled = condition(world);
                }
            }
            let declared = batch.iter().filter_map(|system| {
                if system.enabled {
                    accesses.get(system.index)
                } else {
                    Some(&empty_access)
                }
            });
            let scopes = world.scope_batch(declared)?;
            batch
                .par_iter_mut()
                .zip(scopes)
                .map(|(system, scope)| {
                    let commands = if system.enabled {
                        system.execute(scope)?
                    } else {
                        None
                    };
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
        let graph = match &self.order {
            Some(plan) => ConflictGraph::in_order(
                &self.accesses,
                plan.indices
                    .iter()
                    .copied()
                    .chain(plan.indices.len()..self.accesses.len()),
                plan.predecessors.clone(),
            ),
            None => ConflictGraph::new(&self.accesses),
        };
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
