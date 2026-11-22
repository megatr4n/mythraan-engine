//! Versioned engine settings loaded through the virtual filesystem.

use std::io;

use serde::Deserialize;
use thiserror::Error;

use crate::vfs::Vfs;

const FORMAT_VERSION: u32 = 1;

/// The rendering quality preset selected for the engine.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
pub enum QualityPreset {
    /// Prioritizes performance.
    Low,
    /// Uses the engine's baseline quality settings.
    #[default]
    Medium,
    /// Prioritizes visual quality.
    High,
}

/// Settings used when starting the engine.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EngineConfig {
    quality: QualityPreset,
}

impl EngineConfig {
    /// Loads a RON config through the supplied filesystem.
    ///
    /// A missing file uses the defaults. A present file must declare its format
    /// version. For example, `(format_version: 1, quality: High)`.
    ///
    /// # Errors
    ///
    /// Returns an error for unreadable files, invalid RON, or unsupported versions.
    pub fn load(vfs: &dyn Vfs, path: &str) -> Result<Self, ConfigError> {
        let bytes = match vfs.read(path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(ConfigError::Read {
                    path: path.into(),
                    source,
                });
            }
        };

        let file: ConfigFile =
            ron::de::from_bytes(&bytes).map_err(|source| ConfigError::Parse {
                path: path.into(),
                source: Box::new(source),
            })?;

        if file.format_version != FORMAT_VERSION {
            return Err(ConfigError::UnsupportedVersion {
                path: path.into(),
                version: file.format_version,
            });
        }
        Ok(Self {
            quality: file.quality,
        })
    }

    /// Returns the selected rendering quality preset.
    #[must_use]
    pub const fn quality(&self) -> QualityPreset {
        self.quality
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    format_version: u32,
    #[serde(default)]
    quality: QualityPreset,
}

/// A failure to load engine settings.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The filesystem could not read the configuration file.
    #[error("could not read engine config at {path:?} ({source})")]
    Read {
        /// The virtual config path.
        path: String,
        /// The underlying filesystem error.
        #[source]
        source: io::Error,
    },
    /// The configuration file contains invalid RON or fields.
    #[error("invalid engine config at {path:?} ({source})")]
    Parse {
        /// The virtual config path.
        path: String,
        /// The parse error and its position in the file.
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    /// The configuration file declares an unsupported version.
    #[error("unsupported engine config version {version} at {path:?} (expected {FORMAT_VERSION})")]
    UnsupportedVersion {
        /// The virtual config path.
        path: String,
        /// The version declared by the file.
        version: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = "settings/project.ron";

    struct MemoryVfs(Result<&'static [u8], io::ErrorKind>);

    impl Vfs for MemoryVfs {
        fn read(&self, path: &str) -> io::Result<Vec<u8>> {
            assert_eq!(path, PATH);
            self.0.map(<[u8]>::to_vec).map_err(io::Error::from)
        }
    }

    #[test]
    fn loads_defaults_for_missing_file_or_quality() {
        for input in [
            MemoryVfs(Err(io::ErrorKind::NotFound)),
            MemoryVfs(Ok(b"(format_version: 1)")),
        ] {
            let config = EngineConfig::load(&input, PATH).unwrap();
            assert_eq!(config.quality(), QualityPreset::Medium);
        }
    }

    #[test]
    fn loads_the_configured_quality() {
        let input = MemoryVfs(Ok(b"(format_version: 1, quality: High)"));
        let config = EngineConfig::load(&input, PATH).unwrap();
        assert_eq!(config.quality(), QualityPreset::High);
    }

    #[test]
    fn rejects_invalid_ron_and_missing_versions() {
        for bytes in [
            b"(format_version: 1, quality:" as &[u8],
            b"(quality: Low)",
            b"(format_version: 1, qualitty: Low)",
        ] {
            let error = EngineConfig::load(&MemoryVfs(Ok(bytes)), PATH).unwrap_err();
            assert!(matches!(error, ConfigError::Parse { path, .. } if path == PATH));
        }
    }

    #[test]
    fn rejects_unsupported_versions() {
        let input = MemoryVfs(Ok(b"(format_version: 2)"));
        let error = EngineConfig::load(&input, PATH).unwrap_err();
        assert!(matches!(
            error,
            ConfigError::UnsupportedVersion { path, version: 2 } if path == PATH
        ));
    }

    #[test]
    fn preserves_read_failures_other_than_missing_files() {
        let input = MemoryVfs(Err(io::ErrorKind::PermissionDenied));
        let error = EngineConfig::load(&input, PATH).unwrap_err();
        assert!(matches!(
            error,
            ConfigError::Read { path, source }
                if path == PATH && source.kind() == io::ErrorKind::PermissionDenied
        ));
    }
}
