use std::any::TypeId;
use std::collections::BTreeSet;

use crate::query_mut::{record_data_access, record_filter_access};
use crate::{
    Commands, FilteredQuery, Query, QueryFilter, QueryMut, ReadQuery, Res, ResMut, WriteQuery,
};

/// The types accessed in one kind of ECS storage.
///
/// Types in the write set also permit reads of their values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessSet {
    reads: BTreeSet<TypeId>,
    writes: BTreeSet<TypeId>,
}

impl AccessSet {
    /// Returns the types accessed through shared borrows or query filters.
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
///
/// Component and resource types occupy separate sets. Use [`FilteredQuery`]
/// for declarations that include filters. [`QueryMut`] contributes its data
/// columns when used as a parameter type.
///
/// ```
/// use std::any::TypeId;
/// use mythraan_ecs::prelude::{Commands, QueryMut, Res, SystemAccess};
///
/// struct Position;
/// struct Velocity;
/// struct FrameTime;
///
/// type Movement<'world> = (
///     QueryMut<'world, (&'world mut Position, &'world Velocity)>,
///     Res<'world, FrameTime>,
///     Commands<'world>,
/// );
///
/// let access = SystemAccess::of::<Movement<'_>>();
/// assert!(access.components().writes().contains(&TypeId::of::<Position>()));
/// assert!(access.components().reads().contains(&TypeId::of::<Velocity>()));
/// assert!(access.resources().reads().contains(&TypeId::of::<FrameTime>()));
/// assert!(access.uses_commands());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemAccess {
    components: AccessSet,
    resources: AccessSet,
    deferred_commands: bool,
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

    /// Returns whether the system requests a queue for changes at a sync point.
    #[must_use]
    pub fn uses_commands(&self) -> bool {
        self.deferred_commands
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
///
/// Parameter lists can be tuples of up to eight parameters. Tuple groups can
/// be nested, and `()` declares a system with no parameters.
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

impl<Q: WriteQuery, F: QueryFilter> sealed::Param for FilteredQuery<'_, Q, F> {
    fn record_access(access: &mut SystemAccess) {
        record_data_access::<Q>(access);
        record_filter_access::<F>(access);
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

impl sealed::Param for Commands<'_> {
    fn record_access(access: &mut SystemAccess) {
        access.deferred_commands = true;
    }
}

impl sealed::Param for () {
    fn record_access(_: &mut SystemAccess) {}
}

macro_rules! parameter_tuple {
    ($($param:ident),+) => {
        impl<$($param: SystemParam),+> sealed::Param for ($($param,)+) {
            fn record_access(access: &mut SystemAccess) {
                $($param::record_access(access);)+
            }
        }
    };
}

parameter_tuple!(A);
parameter_tuple!(A, B);
parameter_tuple!(A, B, C);
parameter_tuple!(A, B, C, D);
parameter_tuple!(A, B, C, D, E);
parameter_tuple!(A, B, C, D, E, F);
parameter_tuple!(A, B, C, D, E, F, G);
parameter_tuple!(A, B, C, D, E, F, G, H);
