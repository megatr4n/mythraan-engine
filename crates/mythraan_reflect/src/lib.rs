//! Runtime type and field metadata for the Mythraan engine.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate self as mythraan_reflect;

pub mod prelude;

mod error;
mod reflect;
mod type_info;
mod type_registry;

pub use error::RegisterTypeError;
pub use mythraan_reflect_derive::Reflect;
pub use reflect::Reflect;
pub use type_info::{FieldInfo, FieldRange, TypeInfo};
pub use type_registry::TypeRegistry;
