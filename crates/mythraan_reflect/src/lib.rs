//! Runtime type and field metadata for the Mythraan engine.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod prelude;

mod reflect;
mod type_info;

pub use reflect::Reflect;
pub use type_info::{FieldInfo, TypeInfo};
