//! Debounced filesystem notifications.

use std::collections::{BTreeSet, HashMap};
use std::io;
use std::ops::Bound::{Excluded, Unbounded};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TrySendError};
use std::time::{Duration, Instant};

use notify::event::{ModifyKind, RemoveKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

const QUEUE_CAPACITY: usize = 1024;

/// A change reported for a path in the watched directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileChangeKind {
    /// A path was created.
    Created,
    /// A path was modified or replaced.
    Modified,
    /// A path was removed.
    Removed,
}

/// A debounced filesystem change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChange {
    /// The path relative to the watched directory.
    pub path: PathBuf,
    /// The kind of change.
    pub kind: FileChangeKind,
}

/// A failure to start or continue watching a directory.
#[derive(Debug, Error)]
pub enum WatchError {
    /// The watch root could not be resolved.
    #[error("could not use {path:?} as the watch root ({source})")]
    Root {
        /// The requested root directory.
        path: PathBuf,
        /// The source I/O error.
        #[source]
        source: io::Error,
    },
    /// The platform watcher failed.
    #[error("filesystem watcher failed ({0})")]
    Backend(#[from] notify::Error),
    /// Events may have been lost and the caller should rescan the directory.
    #[error("filesystem watcher lost events and requires a directory rescan")]
    RescanRequired,
}

struct Incoming {
    time: Instant,
    event: notify::Result<Event>,
}

#[derive(Clone, Copy)]
struct PendingChange {
    time: Instant,
    kind: FileChangeKind,
}

/// Watches a directory recursively and groups repeated changes to each path.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    root: PathBuf,
    incoming: Receiver<Incoming>,
    missed: Arc<AtomicBool>,
    quiet_period: Duration,
    pending: HashMap<PathBuf, PendingChange>,
    known: BTreeSet<PathBuf>,
}

impl FileWatcher {
    /// Starts watching an existing directory.
    ///
    /// Changes to the same path are grouped while less than `quiet_period`
    /// elapses between them. A zero duration reports each change separately.
    ///
    /// # Errors
    ///
    /// Returns an error when the root cannot be watched.
    pub fn watch(root: impl AsRef<Path>, quiet_period: Duration) -> Result<Self, WatchError> {
        let root = root.as_ref();
        let absolute = root
            .canonicalize()
            .and_then(|path| {
                if path.metadata()?.is_dir() {
                    Ok(path)
                } else {
                    Err(io::ErrorKind::NotADirectory.into())
                }
            })
            .map_err(|source| WatchError::Root {
                path: root.to_owned(),
                source,
            })?;

        let (sender, incoming) = mpsc::sync_channel(QUEUE_CAPACITY);
        let missed = Arc::new(AtomicBool::new(false));
        let callback_missed = Arc::clone(&missed);
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            let message = Incoming {
                time: Instant::now(),
                event: event.map(normalize_event),
            };
            if matches!(sender.try_send(message), Err(TrySendError::Full(_))) {
                callback_missed.store(true, Ordering::Release);
            }
        })?;
        watcher.watch(&absolute, RecursiveMode::Recursive)?;
        Ok(Self {
            _watcher: watcher,
            root: absolute,
            incoming,
            missed,
            quiet_period,
            pending: HashMap::new(),
            known: BTreeSet::new(),
        })
    }

    /// Returns changes whose quiet period has elapsed without blocking.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend reports a failure or events were lost.
    pub fn poll(&mut self) -> Result<Vec<FileChange>, WatchError> {
        let mut ready = Vec::new();
        let mut error = None;
        for message in self.incoming.try_iter().take(QUEUE_CAPACITY) {
            match message.event {
                Ok(event) if event.need_rescan() => error = Some(WatchError::RescanRequired),
                Ok(event) => {
                    for_each_change(&event, |path, kind| {
                        if let Ok(path) = path.strip_prefix(&self.root)
                            && !path.as_os_str().is_empty()
                        {
                            record_change(
                                &mut self.pending,
                                &mut ready,
                                self.quiet_period,
                                path,
                                PendingChange {
                                    time: message.time,
                                    kind,
                                },
                            );
                        }
                    });
                }
                Err(source) => error = Some(source.into()),
            }
        }

        if self.missed.swap(false, Ordering::AcqRel) {
            error = Some(WatchError::RescanRequired);
        }
        if let Some(error) = error {
            self.pending.clear();
            self.known.clear();
            return Err(error);
        }

        if !self.pending.is_empty() {
            let now = Instant::now();
            ready.extend(
                self.pending
                    .extract_if(|_, change| {
                        now.saturating_duration_since(change.time) >= self.quiet_period
                    })
                    .map(|(path, change)| FileChange {
                        path,
                        kind: change.kind,
                    }),
            );
        }
        for change in &mut ready {
            classify_change(&mut self.known, change);
        }
        Ok(ready)
    }
}

fn classify_change(known: &mut BTreeSet<PathBuf>, change: &mut FileChange) {
    match change.kind {
        FileChangeKind::Created | FileChangeKind::Modified => {
            if known.contains(&change.path) {
                change.kind = FileChangeKind::Modified;
            } else {
                known.insert(change.path.clone());
            }
        }
        FileChangeKind::Removed => {
            known.remove(&change.path);
            let children: Vec<_> = known
                .range::<Path, _>((Excluded(change.path.as_path()), Unbounded))
                .take_while(|path| path.starts_with(&change.path))
                .cloned()
                .collect();
            for path in children {
                known.remove(&path);
            }
        }
    }
}

fn normalize_event(mut event: Event) -> Event {
    let ambiguous_rename = matches!(
        event.kind,
        EventKind::Modify(ModifyKind::Name(RenameMode::Any | RenameMode::Other))
    );
    let macos_change = cfg!(target_os = "macos")
        && matches!(
            event.kind,
            EventKind::Create(_)
                | EventKind::Remove(_)
                | EventKind::Modify(
                    ModifyKind::Any
                        | ModifyKind::Data(_)
                        | ModifyKind::Metadata(_)
                        | ModifyKind::Other
                )
        );
    // Resolve coalesced FSEvents flags on the callback thread.
    if (ambiguous_rename || macos_change)
        && let [path] = event.paths.as_slice()
    {
        match path.symlink_metadata() {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                event.kind = EventKind::Remove(RemoveKind::Any);
            }
            Ok(_) if ambiguous_rename || matches!(event.kind, EventKind::Remove(_)) => {
                event.kind = EventKind::Modify(ModifyKind::Any);
            }
            _ => {}
        }
    }
    event
}

fn for_each_change(event: &Event, mut emit: impl FnMut(&Path, FileChangeKind)) {
    use FileChangeKind::{Created, Modified, Removed};

    let kind = match event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            if let [from, to] = event.paths.as_slice() {
                emit(from, Removed);
                emit(to, Created);
            }
            return;
        }
        EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => Created,
        EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => Removed,
        EventKind::Modify(_) => Modified,
        _ => return,
    };
    for path in &event.paths {
        emit(path, kind);
    }
}

fn record_change(
    pending: &mut HashMap<PathBuf, PendingChange>,
    ready: &mut Vec<FileChange>,
    quiet: Duration,
    path: &Path,
    next: PendingChange,
) {
    let Some(previous) = pending.get_mut(path) else {
        pending.insert(path.to_owned(), next);
        return;
    };
    if next.time.saturating_duration_since(previous.time) >= quiet {
        ready.push(FileChange {
            path: path.to_owned(),
            kind: previous.kind,
        });
        *previous = next;
    } else if let Some(kind) = merge(previous.kind, next.kind) {
        *previous = PendingChange { kind, ..next };
    } else {
        pending.remove(path);
    }
}

fn merge(previous: FileChangeKind, next: FileChangeKind) -> Option<FileChangeKind> {
    use FileChangeKind::{Created, Modified, Removed};

    match (previous, next) {
        (Created, Removed) => None,
        (Created, Created | Modified) => Some(Created),
        (Removed, Modified | Removed) | (Modified, Removed) => Some(Removed),
        (Modified | Removed, Created) | (Modified, Modified) => Some(Modified),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use FileChangeKind::{Created, Modified, Removed};

    #[test]
    fn merges_repeated_events_and_replacements() {
        assert_eq!(merge(Modified, Modified), Some(Modified));
        assert_eq!(merge(Created, Modified), Some(Created));
        assert_eq!(merge(Removed, Created), Some(Modified));
        assert_eq!(merge(Created, Removed), None);
    }

    #[test]
    fn splits_a_rename_into_source_and_destination_changes() {
        let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path("original.ron".into())
            .add_path("renamed.ron".into());
        let mut changes = Vec::new();
        for_each_change(&event, |path, kind| changes.push((path.to_owned(), kind)));
        assert_eq!(
            changes,
            [
                (PathBuf::from("original.ron"), Removed),
                (PathBuf::from("renamed.ron"), Created),
            ]
        );
    }

    #[test]
    fn repeated_creation_is_a_modification_until_removal() {
        let path = PathBuf::from("assets/shader.wgsl");
        let mut known = BTreeSet::new();
        for (reported, expected) in [
            (Created, Created),
            (Created, Modified),
            (Removed, Removed),
            (Created, Created),
        ] {
            let mut change = FileChange {
                path: path.clone(),
                kind: reported,
            };
            classify_change(&mut known, &mut change);
            assert_eq!(change.kind, expected);
        }

        let mut directory = FileChange {
            path: "assets".into(),
            kind: Removed,
        };
        classify_change(&mut known, &mut directory);
        assert!(known.is_empty());
    }

    #[test]
    fn separates_changes_across_quiet_periods() {
        let quiet = Duration::from_millis(30);
        let path = Path::new("scene.ron");
        let start = Instant::now();
        let mut pending = HashMap::new();
        let mut ready = Vec::new();
        for (kind, time) in [
            (Modified, start),
            (Modified, start + Duration::from_millis(5)),
            (Removed, start + Duration::from_millis(40)),
        ] {
            record_change(
                &mut pending,
                &mut ready,
                quiet,
                path,
                PendingChange { time, kind },
            );
        }
        assert_eq!(
            ready,
            [FileChange {
                path: path.into(),
                kind: Modified,
            }]
        );
        assert_eq!(pending.get(path).unwrap().kind, Removed);
    }
}
