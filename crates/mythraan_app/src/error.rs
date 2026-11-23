use mythraan_ecs::{ChangeTickError, ScheduleError, SystemFlush, WorldInsertError};
use thiserror::Error;

use crate::Stage;

/// A frame stopped before all of its stages could complete.
#[derive(Debug, Error)]
pub enum AppError {
    /// The world reached the end of its change tick counter.
    #[error(transparent)]
    ChangeTick(#[from] ChangeTickError),
    /// A stage's schedule failed.
    #[error("schedule failed in {stage:?}")]
    Schedule {
        /// The stage that was running.
        stage: Stage,
        /// The underlying schedule error.
        #[source]
        source: ScheduleError,
    },
    /// A stage finished with failed deferred commands.
    #[error("deferred commands failed in {stage:?}")]
    Commands {
        /// The stage whose commands were applied.
        stage: Stage,
        /// All command reports from the stage, in registration order.
        reports: Vec<SystemFlush>,
    },
    /// Derived world transforms could not be updated after late systems ran.
    #[error("transform propagation failed")]
    Transforms(#[from] WorldInsertError),
}
