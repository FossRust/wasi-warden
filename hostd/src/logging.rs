use tracing_subscriber::{EnvFilter, fmt};

/// Initialize tracing using RUST_LOG or a sensible default.
pub fn init() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,wasi_warden=debug,hostd=debug"));

    let _ = fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .try_init();
}
