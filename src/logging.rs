// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:17:17.590959
//+ Revised: 	2026/11/12 09:16:33.830232
// -----------------------------------------------------------------------------

//! Structured diagnostics; terminal input and project contents are never logged.

use tracing_subscriber::{EnvFilter, fmt};

pub fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = std::env::var("ORIN_LOG").unwrap_or_else(|_| "orin=info,warn".into());
    fmt()
        .json()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::try_new(filter)?)
        .with_target(true)
        .try_init()
}
