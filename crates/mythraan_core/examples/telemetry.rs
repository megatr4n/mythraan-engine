//! Demonstrates structured logging and optional CPU profiling.

use mythraan_core::prelude::{AssetId, profile_scope};
use mythraan_core::telemetry::{self, tracing};

fn main() -> Result<(), tracing::subscriber::SetGlobalDefaultError> {
    telemetry::init_logging()?;
    #[cfg(feature = "profiling")]
    telemetry::set_profiling_enabled(true);

    let asset = AssetId::from_bytes([
        0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00,
        0x00,
    ]);
    {
        profile_scope!("prepare_asset");
        let _span = tracing::info_span!("prepare_asset", %asset).entered();
        tracing::debug!("Asset metadata is ready");
        tracing::info!("Asset prepared");
    }

    #[cfg(feature = "profiling")]
    telemetry::set_profiling_enabled(false);
    Ok(())
}
