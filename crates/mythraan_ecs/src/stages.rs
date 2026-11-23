use crate::Schedule;

/// A sequence of schedules identified by labels of an application-defined type.
///
/// Stages are visited in insertion order. Each stage has its own schedule
/// and command sync point.
pub struct Stages<L> {
    entries: Vec<(L, Schedule)>,
}

impl<L> Default for Stages<L> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<L: Eq> Stages<L> {
    /// Creates an empty stage collection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a schedule or replaces the schedule in the label's existing position.
    pub fn insert(&mut self, label: L, schedule: Schedule) -> Option<Schedule> {
        if let Some(previous) = self.get_mut(&label) {
            Some(std::mem::replace(previous, schedule))
        } else {
            self.entries.push((label, schedule));
            None
        }
    }

    /// Looks up the schedule for a label.
    #[must_use]
    pub fn get(&self, label: &L) -> Option<&Schedule> {
        self.entries
            .iter()
            .find_map(|(key, schedule)| (key == label).then_some(schedule))
    }

    /// Mutably looks up the schedule for a label.
    pub fn get_mut(&mut self, label: &L) -> Option<&mut Schedule> {
        self.entries
            .iter_mut()
            .find_map(|(key, schedule)| (key == label).then_some(schedule))
    }

    /// Visits the labels and their schedules in insertion order.
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = (&L, &mut Schedule)> {
        self.entries
            .iter_mut()
            .map(|(label, schedule)| (&*label, schedule))
    }
}
