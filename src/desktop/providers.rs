// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:58:27.525538
//+ Revised: 	2026/11/13 07:53:51.034948
// -----------------------------------------------------------------------------

//! Provider discovery: each provider is probed on its own thread, all at once.

use crate::agents::{Provider, ProviderStatus};

pub fn probe_all() -> Vec<(Provider, ProviderStatus)> {
    std::thread::scope(|scope| {
        let probes =
            Provider::ALL.map(|provider| (provider, scope.spawn(move || provider.probe())));
        probes
            .into_iter()
            .map(|(provider, probe)| {
                let status = probe.join().unwrap_or_else(|_| {
                    tracing::error!(provider = provider.label(), "Agent discovery failed");
                    ProviderStatus {
                        available: false,
                        executable: None,
                        error: Some("Agent discovery failed. Refresh to try again.".into()),
                        package: None,
                    }
                });
                (provider, status)
            })
            .collect()
    })
}
