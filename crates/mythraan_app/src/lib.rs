//! Application lifecycle and frame orchestration for Mythraan.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod stage;

pub mod prelude;

pub use stage::Stage;
