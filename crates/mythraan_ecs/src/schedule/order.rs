use std::collections::BTreeSet;

use crate::ScheduleConfigError;

/// Explicit ordering constraints applied with [`crate::Schedule::configure_order`].
#[derive(Default)]
pub struct SystemOrder {
    edges: Vec<(usize, usize)>,
}

impl SystemOrder {
    /// Creates an empty order.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requires the first system to finish before the second starts.
    pub fn before(&mut self, first: usize, second: usize) -> &mut Self {
        self.edges.push((first, second));
        self
    }

    /// Requires the first system to start after the second finishes.
    pub fn after(&mut self, first: usize, second: usize) -> &mut Self {
        self.before(second, first)
    }

    pub(super) fn compile(&self, count: usize) -> Result<OrderPlan, ScheduleConfigError> {
        let mut predecessors = vec![Vec::new(); count];
        for &(first, second) in &self.edges {
            validate_system(first, count)?;
            let list = predecessors
                .get_mut(second)
                .ok_or(ScheduleConfigError::UnknownSystem(second))?;
            list.push(first);
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
