use std::any::TypeId;
use std::collections::BTreeSet;

use crate::query_mut::record_data_access;
use crate::{Query, QueryMut, ReadQuery, Res, ResMut, WriteQuery};

/// The types accessed in one kind of ECS storage.
///
/// Types in the write set also permit reads of their values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessSet {
    reads: BTreeSet<TypeId>,
    writes: BTreeSet<TypeId>,
}

impl AccessSet {
    /// Returns the types accessed only through shared references.
    #[must_use]
    pub fn reads(&self) -> &BTreeSet<TypeId> {
        &self.reads
    }

    /// Returns the types accessed exclusively.
    #[must_use]
    pub fn writes(&self) -> &BTreeSet<TypeId> {
        &self.writes
    }

    fn read(&mut self, id: TypeId) {
        if !self.writes.contains(&id) {
            self.reads.insert(id);
        }
    }

    fn write(&mut self, id: TypeId) {
        self.reads.remove(&id);
        self.writes.insert(id);
    }
}

/// The world storage accessed by a system's declared parameters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemAccess {
    components: AccessSet,
    resources: AccessSet,
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

    /// Returns the system's resource access.
    #[must_use]
    pub fn resources(&self) -> &AccessSet {
        &self.resources
    }

    pub(crate) fn read_component(&mut self, id: TypeId) {
        self.components.read(id);
    }

    pub(crate) fn write_component(&mut self, id: TypeId) {
        self.components.write(id);
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

impl<Q: WriteQuery> sealed::Param for QueryMut<'_, Q> {
    fn record_access(access: &mut SystemAccess) {
        record_data_access::<Q>(access);
    }
}

impl<T: Send + Sync + 'static> sealed::Param for Res<'_, T> {
    fn record_access(access: &mut SystemAccess) {
        access.resources.read(TypeId::of::<T>());
    }
}

impl<T: Send + Sync + 'static> sealed::Param for ResMut<'_, T> {
    fn record_access(access: &mut SystemAccess) {
        access.resources.write(TypeId::of::<T>());
    }
}
