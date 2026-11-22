//! Exercises real file changes through the public watcher API.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mythraan_core::prelude::{FileChangeKind, FileWatcher};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let path = std::env::temp_dir().join(format!(
            "mythraan-watcher-{}-{}",
            std::process::id(),
            timestamp.as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn wait_for_change(watcher: &mut FileWatcher, path: &Path, expected: FileChangeKind) {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut observed = Vec::new();
    while Instant::now() < deadline {
        let changes = watcher.poll().unwrap();
        if changes
            .iter()
            .any(|change| change.path == path && change.kind == expected)
        {
            return;
        }
        observed.extend(changes);
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {expected:?} at {path:?}, observed {observed:?}");
}

#[test]
fn observes_creating_modifying_and_removing_a_nested_file() {
    let root = TestDir::new();
    fs::create_dir(root.0.join("assets")).unwrap();
    let path = Path::new("assets/shader.wgsl");
    let asset = root.0.join(path);
    let mut watcher = FileWatcher::watch(&root.0, Duration::from_millis(40)).unwrap();

    fs::write(&asset, b"initial").unwrap();
    wait_for_change(&mut watcher, path, FileChangeKind::Created);

    fs::write(&asset, b"updated content").unwrap();
    wait_for_change(&mut watcher, path, FileChangeKind::Modified);

    fs::remove_file(&asset).unwrap();
    wait_for_change(&mut watcher, path, FileChangeKind::Removed);
}
