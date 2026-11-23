use std::any::TypeId;
use std::collections::BTreeSet;

use crate::{Query, ReadQuery};

/// The types accessed in one kind of ECS storage.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessSet {
    reads: BTreeSet<TypeId>,
}

impl AccessSet {
    /// Returns the types accessed through shared references.
    #[must_use]
    pub fn reads(&self) -> &BTreeSet<TypeId> {
        &self.reads
    }
}

/// The world storage accessed by a system's declared parameters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemAccess {
    components: AccessSet,
}

impl SystemAccess {
    /// Extracts access from parameter types before any world is borrowed.
    #[must_use]
    pub fn of<P: SystemParam>() -> Self {
        let mut access = Self::default();
        P::record_access(&mut access);
        access
    }

    /// Returns the system's component access.
    #[must_use]
    pub fn components(&self) -> &AccessSet {
        &self.components
    }

    pub(crate) fn read_component(&mut self, id: TypeId) {
        self.components.reads.insert(id);
    }
}

mod sealed {
    use super::SystemAccess;

    pub trait Param {
        fn record_access(access: &mut SystemAccess);
    }
}

/// A parameter type whose world access can be extracted before execution.
pub trait SystemParam: sealed::Param {}

impl<P: sealed::Param> SystemParam for P {}

impl<Q: ReadQuery> sealed::Param for Query<'_, Q> {
    fn record_access(access: &mut SystemAccess) {
        Q::record_reads(access);
    }
}
