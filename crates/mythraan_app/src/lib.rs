//! Application lifecycle and frame orchestration for Mythraan.
//!
//! Enable the `reflect` feature to register component types from plugins
//! and edit their named fields through the application's world.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod app;
mod error;
mod stage;
mod time;

pub mod prelude;

pub use app::{App, Plugin};
pub use error::AppError;
pub use stage::Stage;
pub use time::Time;
