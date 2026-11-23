use std::time::Duration;

use mythraan_ecs::prelude::Stages;
use mythraan_ecs::{Schedule, World};

use crate::time::FIXED_HZ;
use crate::{AppError, Stage, Time};

const STEP_UNITS: u128 = 1_000_000_000;

/// Registers a subsystem's resources and systems with an application.
pub trait Plugin {
    /// Configures the application at the point where the plugin is added.
    fn build(&self, app: &mut App);
}

/// Owns an ECS world and its standard frame schedules.
pub struct App {
    world: World,
    stages: Stages<Stage>,
    time: Time,
    // Count nanoseconds scaled by the fixed rate to avoid rounding each step.
    fixed_credit: u128,
    max_substeps: u32,
}

impl Default for App {
    fn default() -> Self {
        let mut world = World::new();
        world.resources_mut().insert(Time::default());

        let mut stages = Stages::new();
        for stage in Stage::ORDER {
            stages.insert(stage, Schedule::new());
        }
        Self {
            world,
            stages,
            time: Time::default(),
            fixed_credit: 0,
            max_substeps: 8,
        }
    }
}

impl App {
    /// Creates an application with empty standard schedules and a zeroed clock.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a plugin immediately and returns the application for chaining.
    pub fn add_plugin(&mut self, plugin: impl Plugin) -> &mut Self {
        plugin.build(self);
        self
    }

    /// Inserts or replaces a resource and returns the application for chaining.
    pub fn insert_resource<T: Send + Sync + 'static>(&mut self, resource: T) -> &mut Self {
        self.world.resources_mut().insert(resource);
        self
    }

    /// Borrows the application world.
    #[must_use]
    pub const fn world(&self) -> &World {
        &self.world
    }

    /// Mutably borrows the application world.
    #[must_use]
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Looks up the schedule for registration or configuration.
    #[must_use]
    pub fn schedule_mut(&mut self, stage: Stage) -> Option<&mut Schedule> {
        self.stages.get_mut(&stage)
    }

    /// Sets the maximum number of fixed steps per frame. The default is eight.
    ///
    /// Excess accumulated time is dropped. Zero disables fixed updates.
    pub fn set_max_substeps(&mut self, max_substeps: u32) -> &mut Self {
        self.max_substeps = max_substeps;
        self
    }

    /// Advances one frame using the supplied elapsed time.
    ///
    /// The world change tick advances once per frame. Derived transforms
    /// are propagated after the post-update schedule and before render.
    ///
    /// # Errors
    ///
    /// A change tick, schedule, deferred command, or transform failure stops
    /// the frame. Changes already applied remain in the world.
    pub fn update(&mut self, delta: Duration) -> Result<(), AppError> {
        self.world.advance_change_tick()?;
        self.time.frame = self.time.frame.saturating_add(1);
        self.time.elapsed = self.time.elapsed.saturating_add(delta);
        self.time.delta = delta;
        self.fixed_credit = (self.fixed_credit + delta.as_nanos() * u128::from(FIXED_HZ))
            .min(u128::from(self.max_substeps) * STEP_UNITS);
        self.time.interpolation_alpha = (self.fixed_credit % STEP_UNITS) as f64 / STEP_UNITS as f64;
        self.publish_time();

        for stage in Stage::ORDER {
            if stage == Stage::FixedUpdate {
                self.run_fixed_updates(delta)?;
            } else {
                self.run_stage(stage)?;
            }
            if stage == Stage::PostUpdate {
                self.world.propagate_transforms()?;
            }
        }
        Ok(())
    }

    fn run_fixed_updates(&mut self, frame_delta: Duration) -> Result<(), AppError> {
        if self.fixed_credit < STEP_UNITS {
            return Ok(());
        }

        self.time.delta = Time::FIXED_STEP;
        let mut result = Ok(());
        while self.fixed_credit >= STEP_UNITS {
            self.fixed_credit -= STEP_UNITS;
            self.time.fixed_ticks = self.time.fixed_ticks.saturating_add(1);
            self.publish_time();
            result = self.run_stage(Stage::FixedUpdate);
            if result.is_err() {
                break;
            }
        }
        self.time.delta = frame_delta;
        self.publish_time();
        result
    }

    fn run_stage(&mut self, stage: Stage) -> Result<(), AppError> {
        if let Some(schedule) = self.stages.get_mut(&stage) {
            let reports = schedule
                .run(&mut self.world)
                .map_err(|source| AppError::Schedule { stage, source })?;
            if reports
                .iter()
                .any(|entry| !entry.report.failures().is_empty())
            {
                return Err(AppError::Commands { stage, reports });
            }
        }
        Ok(())
    }

    fn publish_time(&mut self) {
        let resources = self.world.resources_mut();
        if let Some(time) = resources.get_mut::<Time>() {
            *time = self.time;
        } else {
            resources.insert(self.time);
        }
    }
}
