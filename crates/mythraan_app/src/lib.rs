//! Application lifecycle and frame orchestration for Mythraan.

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
