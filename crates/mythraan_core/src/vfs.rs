//! Read-only access to files through virtual paths.

use std::io;
use std::path::Path;

use cap_std::{ambient_authority, fs::Dir};

/// Reads files from a source such as a directory or an asset archive.
pub trait Vfs: Send + Sync {
    /// Reads a file at a relative, slash-separated virtual path.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid paths, missing files, or reads that fail.
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;
}

/// A virtual filesystem backed by an open root directory.
///
/// Relative symbolic links may resolve within the root. Absolute links and
/// links that lead outside the root are rejected.
pub struct DirVfs {
    root: Dir,
}

impl DirVfs {
    /// Opens an existing directory as the virtual filesystem root.
    ///
    /// # Errors
    ///
    /// Returns an error if the root does not exist or is not a directory.
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = Dir::open_ambient_dir(root, ambient_authority())?;
        Ok(Self { root })
    }
}

impl Vfs for DirVfs {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        if !is_virtual_path(path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected a relative, slash-separated virtual path",
            ));
        }

        if !self.root.metadata(path)?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "virtual path does not point to a regular file",
            ));
        }
        self.root.read(path)
    }
}

fn is_virtual_path(path: &str) -> bool {
    path.split('/').all(|component| {
        !component.is_empty()
            && component != "."
            && component != ".."
            && !component.contains(['\\', ':', '\0'])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let base = std::env::temp_dir();
            loop {
                let path = base.join(format!(
                    "mythraan-vfs-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("could not create test directory: {error}"),
                }
            }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reads_nested_files_and_preserves_not_found() {
        let root = TestDir::new();
        fs::create_dir(root.0.join("settings")).unwrap();
        fs::write(root.0.join("settings/project.ron"), b"()").unwrap();
        let vfs = DirVfs::new(&root.0).unwrap();

        assert_eq!(vfs.read("settings/project.ron").unwrap(), b"()");
        assert_eq!(
            vfs.read("settings/missing.ron").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            vfs.read("settings").unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn loads_engine_config_from_the_directory_backend() {
        use crate::config::ConfigError;
        use crate::prelude::{EngineConfig, QualityPreset};

        let root = TestDir::new();
        let vfs = DirVfs::new(&root.0).unwrap();
        let path = "project.ron";
        let file = root.0.join(path);

        assert_eq!(
            EngineConfig::load(&vfs, path).unwrap().quality(),
            QualityPreset::Medium
        );
        fs::write(&file, b"(format_version: 1, quality: Low)").unwrap();
        assert_eq!(
            EngineConfig::load(&vfs, path).unwrap().quality(),
            QualityPreset::Low
        );
        fs::write(&file, b"(format_version: 1, quality: Broken)").unwrap();
        assert!(matches!(
            EngineConfig::load(&vfs, path),
            Err(ConfigError::Parse { path: invalid, .. }) if invalid == path
        ));
        assert!(matches!(
            EngineConfig::load(&vfs, "../project.ron"),
            Err(ConfigError::Read { source, .. }) if source.kind() == io::ErrorKind::InvalidInput
        ));
    }

    #[test]
    fn rejects_absolute_parent_and_nonportable_paths() {
        let root = TestDir::new();
        let vfs = DirVfs::new(&root.0).unwrap();
        for path in [
            "",
            ".",
            "../project.ron",
            "settings/../project.ron",
            "/project.ron",
            "C:/project.ron",
            "settings\\project.ron",
            "project.ron:stream",
            "settings//project.ron",
            "settings/\0",
        ] {
            assert_eq!(
                vfs.read(path).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{path:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_links_that_resolve_outside_the_root() {
        use std::os::unix::fs::symlink;

        let root = TestDir::new();
        let sibling = TestDir::new();
        fs::write(sibling.0.join("external.ron"), b"()").unwrap();
        symlink(&sibling.0, root.0.join("external")).unwrap();
        let vfs = DirVfs::new(&root.0).unwrap();

        assert_eq!(
            vfs.read("external/external.ron").unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[cfg(unix)]
    #[test]
    fn follows_relative_links_within_the_root() {
        use std::os::unix::fs::symlink;

        let root = TestDir::new();
        fs::create_dir(root.0.join("settings")).unwrap();
        fs::write(root.0.join("settings/project.ron"), b"()").unwrap();
        symlink("settings/project.ron", root.0.join("project.ron")).unwrap();
        let vfs = DirVfs::new(&root.0).unwrap();

        assert_eq!(vfs.read("project.ron").unwrap(), b"()");
    }

    #[cfg(unix)]
    #[test]
    fn stays_bound_to_the_open_root_after_it_is_renamed() {
        let parent = TestDir::new();
        let original = parent.0.join("project");
        fs::create_dir(&original).unwrap();
        fs::write(original.join("asset.bin"), b"original").unwrap();
        let vfs = DirVfs::new(&original).unwrap();

        fs::rename(&original, parent.0.join("renamed")).unwrap();
        fs::create_dir(&original).unwrap();
        fs::write(original.join("asset.bin"), b"replacement").unwrap();

        assert_eq!(vfs.read("asset.bin").unwrap(), b"original");
    }
}
