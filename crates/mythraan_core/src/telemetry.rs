//! Engine logging and CPU profiling.

use tracing::level_filters::LevelFilter;
use tracing::subscriber::SetGlobalDefaultError;
use tracing_subscriber::fmt::time::Uptime;

pub use tracing;

#[cfg(feature = "profiling")]
#[doc(hidden)]
pub use puffin;

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

/// Enables or disables CPU profiling at a frame boundary.
///
/// Disabling profiling submits any completed scopes in the final partial frame.
#[cfg(feature = "profiling")]
pub fn set_profiling_enabled(enabled: bool) {
    if enabled == puffin::are_scopes_on() {
        return;
    }
    puffin::set_scopes_on(enabled);
    if !enabled {
        puffin::GlobalProfiler::lock().new_frame();
    }
}

/// Submits completed scopes and starts the next profiling frame.
#[cfg(feature = "profiling")]
pub fn advance_profile_frame() {
    if puffin::are_scopes_on() {
        puffin::GlobalProfiler::lock().new_frame();
    }
}

/// Profiles from this call to the end of the enclosing block.
///
/// The name is a string literal. Recording requires the `profiling` feature
/// and runtime activation through `telemetry::set_profiling_enabled`.
#[cfg(feature = "profiling")]
#[macro_export]
macro_rules! profile_scope {
    ($name:literal $(,)?) => {
        $crate::telemetry::puffin::profile_scope!($name);
    };
}

/// Marks a scope for CPU profiling. This expands to no runtime code when the
/// `profiling` feature is disabled.
#[cfg(not(feature = "profiling"))]
#[macro_export]
macro_rules! profile_scope {
    ($name:literal $(,)?) => {
        const _: &str = $name;
    };
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

    #[cfg(feature = "profiling")]
    #[test]
    fn submits_a_profiled_scope_at_the_next_frame() {
        let (frames, received) = std::sync::mpsc::channel();
        let sink = puffin::GlobalProfiler::lock().add_sink(Box::new(move |frame| {
            let _ = frames.send(frame);
        }));

        advance_profile_frame();
        let disabled_frame = received.try_recv();
        set_profiling_enabled(true);
        {
            crate::profile_scope!("telemetry_test_scope");
        }
        advance_profile_frame();
        let frame = received.try_recv();
        set_profiling_enabled(false);
        drop(puffin::GlobalProfiler::lock().remove_sink(sink));

        assert!(matches!(
            disabled_frame,
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        let frame = frame.unwrap();
        assert_eq!(frame.meta().num_scopes, 1);
        assert!(
            frame
                .scope_delta
                .iter()
                .any(|scope| { scope.scope_name.as_deref() == Some("telemetry_test_scope") })
        );
    }
}
