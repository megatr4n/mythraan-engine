/// A stage in the application's frame schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Begins the frame.
    First,
    /// Processes input and incoming events.
    PreUpdate,
    /// Runs simulation at a fixed rate, zero or more times per frame.
    FixedUpdate,
    /// Runs the main variable-rate update.
    Update,
    /// Runs late updates before transform propagation.
    PostUpdate,
    /// Runs systems that extract, prepare, and render the frame.
    Render,
    /// Finishes the frame.
    Last,
}

impl Stage {
    /// The order of the frame stages. Fixed update may repeat or be skipped.
    pub const ORDER: [Self; 7] = [
        Self::First,
        Self::PreUpdate,
        Self::FixedUpdate,
        Self::Update,
        Self::PostUpdate,
        Self::Render,
        Self::Last,
    ];
}
