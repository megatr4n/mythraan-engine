//! Runtime type and field metadata for the Mythraan engine.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod prelude;

mod error;
mod reflect;
mod type_info;
mod type_registry;

pub use error::RegisterTypeError;
pub use reflect::Reflect;
pub use type_info::{FieldInfo, TypeInfo};
pub use type_registry::TypeRegistry;
