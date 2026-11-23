use mythraan_ecs::prelude::Stages;
use mythraan_ecs::{Schedule, World};

use crate::{Stage, Time};

/// Registers a subsystem's resources and systems with an application.
pub trait Plugin {
    /// Configures the application at the point where the plugin is added.
    fn build(&self, app: &mut App);
}

/// Owns an ECS world and its standard frame schedules.
pub struct App {
    world: World,
    stages: Stages<Stage>,
}

impl Default for App {
    fn default() -> Self {
        let mut world = World::new();
        world.resources_mut().insert(Time::default());

        let mut stages = Stages::new();
        for stage in Stage::ORDER {
            stages.insert(stage, Schedule::new());
        }
        Self { world, stages }
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
}
