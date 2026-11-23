use std::any::TypeId;
use std::collections::BTreeMap;

use crate::{AccessSet, SystemAccess};

/// Orders conflicting systems by their registration positions.
///
/// Vertices are the indices of the supplied access sets. A path orders each
/// conflicting pair, so an edge may be omitted when other edges enforce it.
#[derive(Debug, PartialEq, Eq)]
pub struct ConflictGraph {
    predecessors: Vec<Vec<usize>>,
}

impl ConflictGraph {
    /// Builds the graph from access sets in registration order.
    #[must_use]
    pub fn new(systems: &[SystemAccess]) -> Self {
        let mut components = BTreeMap::new();
        let mut resources = BTreeMap::new();
        let mut predecessors = Vec::with_capacity(systems.len());

        for (system, access) in systems.iter().enumerate() {
            let mut dependencies = Vec::new();
            record_access(
                access.components(),
                &mut components,
                system,
                &mut dependencies,
            );
            record_access(
                access.resources(),
                &mut resources,
                system,
                &mut dependencies,
            );
            dependencies.sort_unstable();
            dependencies.dedup();
            predecessors.push(dependencies);
        }
        Self { predecessors }
    }

    /// Returns direct predecessors in registration order, or `None` for an unknown index.
    #[must_use]
    pub fn predecessors(&self, system: usize) -> Option<&[usize]> {
        self.predecessors.get(system).map(Vec::as_slice)
    }
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
