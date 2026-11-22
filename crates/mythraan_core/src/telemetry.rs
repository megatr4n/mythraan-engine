//! Engine logging and tracing setup.

use tracing::level_filters::LevelFilter;
use tracing::subscriber::SetGlobalDefaultError;
use tracing_subscriber::fmt::time::Uptime;

pub use tracing;

/// Installs the engine's global log subscriber.
///
/// Set `MYTHRAAN_LOG` to `off`, `error`, `warn`, `info`, `debug` or `trace`.
/// Missing or invalid settings default to `info`.
///
/// # Errors
///
/// Returns an error if a global subscriber is already installed.
pub fn init_logging() -> Result<(), SetGlobalDefaultError> {
    let setting = std::env::var("MYTHRAAN_LOG").ok();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(log_level(setting.as_deref()))
        .with_timer(Uptime::default())
        .with_writer(std::io::stderr)
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)
}

fn log_level(setting: Option<&str>) -> LevelFilter {
    setting
        .and_then(|level| level.trim().parse().ok())
        .unwrap_or(LevelFilter::INFO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_log_levels_and_defaults_to_info() {
        assert_eq!(log_level(Some(" DEBUG ")), LevelFilter::DEBUG);
        assert_eq!(log_level(Some("off")), LevelFilter::OFF);
        assert_eq!(log_level(Some("unknown")), LevelFilter::INFO);
        assert_eq!(log_level(None), LevelFilter::INFO);
    }
}
