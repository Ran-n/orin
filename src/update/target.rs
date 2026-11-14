// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:37.403166
//+ Revised: 	2026/11/14 12:09:04.062863
// -----------------------------------------------------------------------------

//! Which release build this binary is, and what its file is called in a release.
//!
//! Release builds are compiled with `ORIN_RELEASE_TARGET=<target key>`. Any other build (a
//! developer's `cargo run`, a binary someone built themselves) has no target and never updates
//! itself, so the updater can't overwrite `target/debug/orin` or a hand-made build.

use std::sync::Once;

/// Every target a release carries a binary for, as `<os>-<arch>`.
pub const TARGETS: [&str; 3] = ["windows-x86_64", "linux-x86_64", "macos-aarch64"];

/// The target this binary was released for, or `None` for a build from source.
///
/// Debug builds also read the variable at run time, so rendered tests can exercise the updater
/// without a release build; a release build only trusts what it was compiled with.
pub fn release_target() -> Option<&'static str> {
    static RUNTIME: std::sync::OnceLock<Option<&'static str>> = std::sync::OnceLock::new();
    known_target(option_env!("ORIN_RELEASE_TARGET")).or_else(|| {
        *RUNTIME.get_or_init(|| {
            if !cfg!(debug_assertions) {
                return None;
            }
            let value = std::env::var("ORIN_RELEASE_TARGET").ok()?;
            TARGETS.iter().copied().find(|target| *target == value)
        })
    })
}

fn known_target(value: Option<&'static str>) -> Option<&'static str> {
    let value = value.filter(|value| !value.is_empty())?;
    if TARGETS.contains(&value) {
        return Some(value);
    }
    static WARNED: Once = Once::new();
    WARNED.call_once(|| {
        tracing::warn!(
            target: "orin::update",
            value,
            "Unknown ORIN_RELEASE_TARGET; treating this as a build from source"
        );
    });
    None
}

/// The release asset holding `version` for `target`: `orin-<version>-<target>`, plus `.exe` on
/// Windows. The single definition of the naming, shared by CI (through `orin-asset-name`) and
/// the updater, so the two can't drift apart.
pub fn asset_name(version: &semver::Version, target: &str) -> String {
    let extension = if target.starts_with("windows-") {
        ".exe"
    } else {
        ""
    };
    format!("orin-{version}-{target}{extension}")
}

/// The checksum file published next to `asset`.
pub fn checksum_name(asset: &str) -> String {
    format!("{asset}.b3")
}

/// The signature file published next to `asset`, made with `orin-sign`.
pub fn signature_name(asset: &str) -> String {
    format!("{asset}.sig")
}

/// The running version, from `Cargo.toml`.
pub fn current_version() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo.toml holds a semver version")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_asset_name_per_target() {
        let version = semver::Version::new(0, 2, 0);
        let names: Vec<String> = TARGETS
            .iter()
            .map(|target| asset_name(&version, target))
            .collect();
        assert_eq!(
            names,
            [
                "orin-0.2.0-windows-x86_64.exe",
                "orin-0.2.0-linux-x86_64",
                "orin-0.2.0-macos-aarch64"
            ]
        );
        assert_eq!(checksum_name(&names[1]), "orin-0.2.0-linux-x86_64.b3");
        assert_eq!(signature_name(&names[1]), "orin-0.2.0-linux-x86_64.sig");
    }

    #[test]
    fn unit_asset_name_keeps_prerelease() {
        let version = semver::Version::parse("0.2.0-rc.1").unwrap();
        assert_eq!(
            asset_name(&version, "linux-x86_64"),
            "orin-0.2.0-rc.1-linux-x86_64"
        );
    }

    #[test]
    fn unit_source_builds_have_no_target() {
        // A normal `cargo test` run is a build from source.
        if option_env!("ORIN_RELEASE_TARGET").is_none()
            && std::env::var_os("ORIN_RELEASE_TARGET").is_none()
        {
            assert_eq!(release_target(), None);
        }
        assert_eq!(known_target(None), None);
        assert_eq!(known_target(Some("")), None);
        assert_eq!(known_target(Some("plan9-mips")), None);
        assert_eq!(known_target(Some("linux-x86_64")), Some("linux-x86_64"));
    }

    #[test]
    fn unit_current_version_is_the_package_version() {
        assert_eq!(current_version().to_string(), env!("CARGO_PKG_VERSION"));
    }
}
