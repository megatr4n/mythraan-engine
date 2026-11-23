use std::any::{TypeId, type_name};

use crate::Reflect;

/// The name and Rust type of a reflected field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldInfo {
    name: &'static str,
    type_id: TypeId,
}

impl FieldInfo {
    /// Describes a field with the given name and Rust type.
    #[must_use]
    pub const fn new<T: 'static>(name: &'static str) -> Self {
        Self {
            name,
            type_id: TypeId::of::<T>(),
        }
    }

    /// Returns the field's name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the Rust type identifier of the field's value.
    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }
}

/// Runtime metadata for one reflected Rust type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeInfo {
    name: &'static str,
    type_id: TypeId,
    fields: &'static [FieldInfo],
}

impl TypeInfo {
    /// Gets the type's identity from Rust and its fields from its reflection implementation.
    #[must_use]
    pub fn of<T: Reflect>() -> Self {
        Self {
            name: type_name::<T>(),
            type_id: TypeId::of::<T>(),
            fields: T::fields(),
        }
    }

    /// Returns the Rust type name used for lookup in this build.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the type's Rust identifier.
    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns field metadata in declaration order.
    #[must_use]
    pub const fn fields(&self) -> &'static [FieldInfo] {
        self.fields
    }
}
