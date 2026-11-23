use std::collections::{BTreeMap, BTreeSet};

use crate::ScheduleConfigError;

/// Explicit ordering constraints applied with [`crate::Schedule::configure_order`].
///
/// ```
/// use mythraan_ecs::prelude::{ResMut, Schedule, SystemAccess, SystemOrder, World};
///
/// let mut schedule = Schedule::new();
/// let access = SystemAccess::of::<ResMut<'_, u32>>();
/// let finish = schedule.add_system(access.clone(), |mut scope, _| {
///     if let Some(mut value) = scope.resources.get_mut::<u32>()? {
///         *value += 1;
///     }
///     Ok(())
/// });
/// let reset = schedule.add_system(access, |mut scope, _| {
///     if let Some(mut value) = scope.resources.get_mut::<u32>()? {
///         *value = 10;
///     }
///     Ok(())
/// });
/// let mut order = SystemOrder::new();
/// schedule.configure_order(order.set("prepare", [reset]).before("prepare", finish))?;
/// let mut world = World::new();
/// world.resources_mut().insert(0_u32);
/// assert!(schedule.run(&mut world)?.is_empty());
/// assert_eq!(world.resources().get::<u32>(), Some(&11));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Default)]
pub struct SystemOrder {
    sets: BTreeMap<&'static str, Vec<usize>>,
    edges: Vec<(OrderTarget, OrderTarget)>,
}

/// A registered system or a named set of systems in a [`SystemOrder`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderTarget {
    /// A registration index returned when adding a system.
    System(usize),
    /// A name declared with [`SystemOrder::set`].
    Set(&'static str),
}

impl From<usize> for OrderTarget {
    fn from(system: usize) -> Self {
        Self::System(system)
    }
}

impl From<&'static str> for OrderTarget {
    fn from(name: &'static str) -> Self {
        Self::Set(name)
    }
}

impl SystemOrder {
    /// Creates an empty order.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Defines or replaces a set of registration indices. A system may join several sets.
    pub fn set(
        &mut self,
        name: &'static str,
        systems: impl IntoIterator<Item = usize>,
    ) -> &mut Self {
        self.sets.insert(name, systems.into_iter().collect());
        self
    }

    /// Requires every system in the first target to finish before any in the second starts.
    pub fn before(
        &mut self,
        first: impl Into<OrderTarget>,
        second: impl Into<OrderTarget>,
    ) -> &mut Self {
        self.edges.push((first.into(), second.into()));
        self
    }

    /// Requires every system in the first target to start after all in the second finish.
    pub fn after(
        &mut self,
        first: impl Into<OrderTarget>,
        second: impl Into<OrderTarget>,
    ) -> &mut Self {
        self.before(second, first)
    }

    pub(super) fn compile(&self, count: usize) -> Result<OrderPlan, ScheduleConfigError> {
        let mut predecessors = vec![Vec::new(); count];
        for &member in self.sets.values().flatten() {
            validate_system(member, count)?;
        }
        for (first, second) in &self.edges {
            let first = self.members(first, count)?;
            for &system in self.members(second, count)? {
                if let Some(list) = predecessors.get_mut(system) {
                    list.extend_from_slice(first);
                }
            }
        }
        for list in &mut predecessors {
            list.sort_unstable();
            list.dedup();
        }
        let indices = topological_order(&predecessors)?;
        Ok(OrderPlan {
            indices,
            predecessors,
        })
    }

    fn members<'a>(
        &'a self,
        target: &'a OrderTarget,
        count: usize,
    ) -> Result<&'a [usize], ScheduleConfigError> {
        match target {
            OrderTarget::System(index) => {
                validate_system(*index, count)?;
                Ok(std::slice::from_ref(index))
            }
            OrderTarget::Set(name) => self
                .sets
                .get(name)
                .map(Vec::as_slice)
                .ok_or(ScheduleConfigError::UnknownSet(name)),
        }
    }
}

pub(super) struct OrderPlan {
    pub(super) indices: Vec<usize>,
    pub(super) predecessors: Vec<Vec<usize>>,
}

fn validate_system(index: usize, count: usize) -> Result<(), ScheduleConfigError> {
    if index < count {
        Ok(())
    } else {
        Err(ScheduleConfigError::UnknownSystem(index))
    }
}

fn topological_order(predecessors: &[Vec<usize>]) -> Result<Vec<usize>, ScheduleConfigError> {
    let count = predecessors.len();
    let mut successors = vec![Vec::new(); count];
    let mut remaining = Vec::with_capacity(count);
    let mut ready = BTreeSet::new();
    for (system, dependencies) in predecessors.iter().enumerate() {
        remaining.push(dependencies.len());
        if dependencies.is_empty() {
            ready.insert(system);
        }
        for &dependency in dependencies {
            if let Some(outgoing) = successors.get_mut(dependency) {
                outgoing.push(system);
            }
        }
    }
    let mut order = Vec::with_capacity(count);
    while let Some(system) = ready.pop_first() {
        order.push(system);
        for &next in successors.get(system).into_iter().flatten() {
            if let Some(left) = remaining.get_mut(next) {
                *left -= 1;
                if *left == 0 {
                    ready.insert(next);
                }
            }
        }
    }
    if order.len() == count {
        Ok(order)
    } else {
        Err(ScheduleConfigError::Cycle)
    }
}
