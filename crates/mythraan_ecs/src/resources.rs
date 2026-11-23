use std::any::{Any, TypeId};
use std::collections::{BTreeMap, btree_map::Entry};
use std::mem;
use std::ops::Deref;

/// Stores one shared value of each Rust type.
#[derive(Default)]
pub struct Resources {
    values: BTreeMap<TypeId, Box<dyn Any + Send + Sync>>,
}

/// A system's shared borrow of the resource of type `T`.
pub struct Res<'world, T>(&'world T);

impl<'world, T: Send + Sync + 'static> Res<'world, T> {
    /// Borrows the resource if it is present.
    #[must_use]
    pub fn from_resources(resources: &'world Resources) -> Option<Self> {
        resources.get().map(Self)
    }
}

impl<T> Deref for Res<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
    }
}

impl Resources {
    /// Creates an empty resource collection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    /// Stores a resource and returns the previous value of the same type, if any.
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) -> Option<T> {
        match self.values.entry(TypeId::of::<T>()) {
            Entry::Vacant(entry) => {
                entry.insert(Box::new(value));
                None
            }
            Entry::Occupied(mut entry) => entry
                .get_mut()
                .downcast_mut::<T>()
                .map(|previous| mem::replace(previous, value)),
        }
    }

    /// Borrows the resource of the requested type if it is present.
    #[must_use]
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.values.get(&TypeId::of::<T>())?.downcast_ref()
    }

    /// Mutably borrows the resource of the requested type if it is present.
    #[must_use]
    pub fn get_mut<T: Send + Sync + 'static>(&mut self) -> Option<&mut T> {
        self.values.get_mut(&TypeId::of::<T>())?.downcast_mut()
    }

    /// Removes and returns the resource of the requested type if it is present.
    pub fn remove<T: Send + Sync + 'static>(&mut self) -> Option<T> {
        let removed = self.values.remove(&TypeId::of::<T>())?;
        removed.downcast::<T>().ok().map(|value| *value)
    }
}
