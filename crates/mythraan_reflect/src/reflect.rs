use std::any::Any;

use crate::FieldInfo;

/// Describes the fields of a Rust type without requiring a value.
pub trait Reflect: Any {
    /// Returns the field metadata in declaration order.
    fn fields() -> &'static [FieldInfo]
    where
        Self: Sized;
}
