//! Application lifecycle and frame orchestration for Mythraan.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod app;
mod stage;
mod time;

pub mod prelude;

pub use app::{App, Plugin};
pub use stage::Stage;
pub use time::Time;
