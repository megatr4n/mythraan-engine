use thiserror::Error;

/// A reflected type could not be added to the registry.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegisterTypeError {
    /// Two distinct Rust types have the same lookup name.
    #[error("a different type is already registered under {name}")]
    NameCollision {
        /// The name already in use.
        name: &'static str,
    },
}
