use std::any::TypeId;
use std::collections::BTreeMap;

use crate::{AccessSet, SystemAccess};

/// Orders conflicting systems by their registration positions.
///
/// Vertices are the indices of the supplied access sets. A path orders each
/// conflicting pair, so an edge may be omitted when other edges enforce it.
/// Component and resource access are tracked independently. Deferred queues
/// are for structural changes at the scheduler's sync point.
///
/// ```
/// use mythraan_ecs::prelude::{ConflictGraph, Query, QueryMut, ResMut, SystemAccess};
///
/// let graph = ConflictGraph::new(&[
///     SystemAccess::of::<QueryMut<'_, &mut u32>>(),
///     SystemAccess::of::<Query<'_, &u32>>(),
///     SystemAccess::of::<ResMut<'_, u32>>(),
/// ]);
///
/// assert_eq!(graph.predecessors(1), Some([0].as_slice()));
/// assert_eq!(graph.batches().collect::<Vec<_>>(), [&[0, 2][..], &[1]]);
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct ConflictGraph {
    predecessors: Vec<Vec<usize>>,
    batches: Vec<Vec<usize>>,
}

impl ConflictGraph {
    /// Builds the graph from access sets in registration order.
    #[must_use]
    pub fn new(systems: &[SystemAccess]) -> Self {
        Self::in_order(systems, 0..systems.len(), Vec::new())
    }

    pub(crate) fn in_order(
        systems: &[SystemAccess],
        order: impl Iterator<Item = usize> + Clone,
        mut predecessors: Vec<Vec<usize>>,
    ) -> Self {
        let mut components = BTreeMap::new();
        let mut resources = BTreeMap::new();
        predecessors.resize_with(systems.len(), Vec::new);

        for system in order.clone() {
            let (Some(access), Some(dependencies)) =
                (systems.get(system), predecessors.get_mut(system))
            else {
                continue;
            };
            record_access(access.components(), &mut components, system, dependencies);
            record_access(access.resources(), &mut resources, system, dependencies);
            dependencies.sort_unstable();
            dependencies.dedup();
        }
        let batches = build_batches(&predecessors, order);
        Self {
            predecessors,
            batches,
        }
    }

    /// Returns direct predecessors in registration order, or `None` for an unknown index.
    #[must_use]
    pub fn predecessors(&self, system: usize) -> Option<&[usize]> {
        self.predecessors.get(system).map(Vec::as_slice)
    }

    /// Returns compatible batches in dependency order, placing each system
    /// in the earliest batch after its prerequisites.
    ///
    /// Within each batch, systems appear in registration order.
    pub fn batches(&self) -> impl ExactSizeIterator<Item = &[usize]> {
        self.batches.iter().map(Vec::as_slice)
    }
}

fn build_batches(
    predecessors: &[Vec<usize>],
    order: impl Iterator<Item = usize>,
) -> Vec<Vec<usize>> {
    let mut levels = vec![0; predecessors.len()];
    for system in order {
        let Some(dependencies) = predecessors.get(system) else {
            continue;
        };
        let level = dependencies
            .iter()
            .filter_map(|&dependency| levels.get(dependency).copied())
            .max()
            .map_or(0, |previous| previous + 1);
        if let Some(slot) = levels.get_mut(system) {
            *slot = level;
        }
    }
    let mut batches: Vec<Vec<usize>> = Vec::new();
    for (system, level) in levels.into_iter().enumerate() {
        if level >= batches.len() {
            batches.resize_with(level + 1, Vec::new);
        }
        if let Some(batch) = batches.get_mut(level) {
            batch.push(system);
        }
    }
    batches
}

#[derive(Default)]
struct StorageHistory {
    writer: Option<usize>,
    readers: Vec<usize>,
}

fn record_access(
    access: &AccessSet,
    history: &mut BTreeMap<TypeId, StorageHistory>,
    system: usize,
    dependencies: &mut Vec<usize>,
) {
    for &id in access.reads() {
        let entry = history.entry(id).or_default();
        dependencies.extend(entry.writer);
        entry.readers.push(system);
    }
    for &id in access.writes() {
        let entry = history.entry(id).or_default();
        if entry.readers.is_empty() {
            dependencies.extend(entry.writer);
        } else {
            // The intervening readers already depend on the previous writer.
            dependencies.append(&mut entry.readers);
        }
        entry.writer = Some(system);
    }
}

#[cfg(test)]
mod tests {
    use super::ConflictGraph;
    use crate::{Res, ResMut, SystemAccess};

    #[test]
    fn conflict_edges_follow_the_configured_topological_order() {
        let systems = [
            SystemAccess::of::<ResMut<'_, u32>>(),
            SystemAccess::of::<Res<'_, u32>>(),
            SystemAccess::of::<ResMut<'_, u32>>(),
            SystemAccess::of::<()>(),
        ];
        let graph = ConflictGraph::in_order(
            &systems,
            [2, 0, 1, 3].into_iter(),
            vec![vec![2], vec![], vec![], vec![]],
        );
        assert_eq!(graph.predecessors(0), Some([2].as_slice()));
        assert_eq!(graph.predecessors(1), Some([0].as_slice()));
        assert_eq!(
            graph.batches().collect::<Vec<_>>(),
            [&[2, 3][..], &[0], &[1]]
        );
    }
}
