use std::any::Any;

use crate::FieldInfo;

/// Describes a Rust type without requiring a value.
pub trait Reflect: Any {
    /// Returns the field metadata in declaration order.
    fn fields() -> &'static [FieldInfo]
    where
        Self: Sized;

    /// Returns the unit variant names of a reflected enum in declaration order.
    fn variants() -> &'static [&'static str]
    where
        Self: Sized,
    {
        &[]
    }
}

/// Provides erased access to the named fields of a reflected value.
///
/// The derive supplies this access for named fields with aligned storage.
/// Field attributes describe metadata and do not restrict access.
pub trait ReflectFields: Reflect {
    /// Borrows a field by its declared name, if exposed by the type.
    fn field(&self, _: &str) -> Option<&dyn Any> {
        None
    }

    /// Mutably borrows a field by its declared name, if exposed by the type.
    fn field_mut(&mut self, _: &str) -> Option<&mut dyn Any> {
        None
    }
}
