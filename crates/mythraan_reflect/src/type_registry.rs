use std::any::TypeId;
use std::collections::{HashMap, hash_map::Entry};

use crate::{Reflect, RegisterTypeError, TypeInfo};

/// An index of reflected types by Rust type name and identifier.
///
/// ```
/// use std::any::{TypeId, type_name};
///
/// use mythraan_reflect::prelude::{FieldInfo, Reflect, TypeRegistry};
///
/// struct Marker;
///
/// impl Reflect for Marker {
///     fn fields() -> &'static [FieldInfo] {
///         &[]
///     }
/// }
///
/// let mut registry = TypeRegistry::new();
/// registry.register::<Marker>()?;
/// let info = registry.get_by_name(type_name::<Marker>());
/// assert_eq!(info, registry.get_by_id(TypeId::of::<Marker>()));
/// # Ok::<(), mythraan_reflect::RegisterTypeError>(())
/// ```
#[derive(Debug, Default)]
pub struct TypeRegistry {
    by_id: HashMap<TypeId, TypeInfo>,
    by_name: HashMap<&'static str, TypeId>,
}

impl TypeRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a type's metadata. Registering the same type again succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`RegisterTypeError::NameCollision`] if the name belongs to
    /// a different type. The existing entry is kept.
    pub fn register<T: Reflect>(&mut self) -> Result<(), RegisterTypeError> {
        let Entry::Vacant(type_slot) = self.by_id.entry(TypeId::of::<T>()) else {
            return Ok(());
        };
        let info = TypeInfo::of::<T>();
        let Entry::Vacant(name_slot) = self.by_name.entry(info.name()) else {
            return Err(RegisterTypeError::NameCollision { name: info.name() });
        };
        name_slot.insert(info.type_id());
        type_slot.insert(info);
        Ok(())
    }

    /// Looks up metadata by the Rust type name returned by [`TypeInfo::name`].
    #[must_use]
    pub fn get_by_name(&self, name: &str) -> Option<&TypeInfo> {
        self.get_by_id(*self.by_name.get(name)?)
    }

    /// Looks up metadata by Rust type identifier.
    #[must_use]
    pub fn get_by_id(&self, type_id: TypeId) -> Option<&TypeInfo> {
        self.by_id.get(&type_id)
    }
}
