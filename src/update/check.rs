// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:37.806031
//+ Revised: 	2026/11/14 12:09:04.771742
// -----------------------------------------------------------------------------

//! Deciding, from the sources' answers, whether this copy of Orin has an update.

use super::{
    release::{Asset, Release},
    source::ReleaseSource,
    target::{asset_name, checksum_name, signature_name},
};
use semver::Version;

/// The result of the last check, as the banner and Settings › Updates show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    /// Built from source: never checks, never installs.
    SourceBuild,
    UpToDate,
    /// A newer release with a build for this system, its checksum and its signature.
    Available {
        release: Release,
        asset: Asset,
        checksum: Asset,
        signature: Asset,
    },
    /// A newer release without a build (or its checksum or signature) for this system.
    NoBuild(Version),
    /// A newer release the user chose to skip; it stays quiet until a newer one appears.
    Skipped(Version),
    /// Every source failed, with each one's reason, in the order they were asked.
    Failed(Vec<(&'static str, String)>),
}

impl UpdateStatus {
    /// The Settings status line; `checked` is the local time of the check, like `14:02`.
    pub fn message(&self, checked: &str) -> String {
        match self {
            Self::SourceBuild => {
                "This copy was built from source. Update it with git pull and a new build.".into()
            }
            Self::UpToDate => format!("Orin is up to date (checked {checked})."),
            Self::Available { release, .. } => {
                format!("Orin {} is available (checked {checked}).", release.version)
            }
            Self::NoBuild(version) => {
                format!("Orin {version} is out, but has no build for this system yet.")
            }
            Self::Skipped(version) => {
                format!("Orin {version} is available; you chose to skip it (checked {checked}).")
            }
            Self::Failed(reasons) => format!(
                "Couldn't check for updates: {}.",
                reasons
                    .iter()
                    .map(|(source, reason)| format!("{source}: {reason}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        }
    }

    /// The version the banner offers, if any.
    pub fn offered(&self) -> Option<&Version> {
        match self {
            Self::Available { release, .. } => Some(&release.version),
            _ => None,
        }
    }
}

/// Whether `candidate` should be offered to someone running `current`: it must be newer, and a
/// pre-release is only offered to someone already on a pre-release.
pub fn is_offered(current: &Version, candidate: &Version) -> bool {
    candidate > current && (candidate.pre.is_empty() || !current.pre.is_empty())
}

/// Ask each source in order until one answers, then decide. A source build asks nobody.
pub fn check(
    sources: &[Box<dyn ReleaseSource + Send>],
    current: &Version,
    target: Option<&str>,
    skipped: Option<&Version>,
) -> UpdateStatus {
    let Some(target) = target else {
        return UpdateStatus::SourceBuild;
    };
    let mut failures = Vec::new();
    for source in sources {
        match source.latest() {
            Ok(release) => {
                tracing::debug!(
                    target: "orin::update",
                    source = source.name(),
                    version = %release.version,
                    "latest release read"
                );
                return decide(release, current, target, skipped);
            }
            Err(error) => {
                tracing::debug!(target: "orin::update", source = source.name(), %error, "release source failed");
                failures.push((source.name(), error.to_string()));
            }
        }
    }
    tracing::warn!(target: "orin::update", ?failures, "Update check failed at every source");
    UpdateStatus::Failed(failures)
}

fn decide(
    release: Release,
    current: &Version,
    target: &str,
    skipped: Option<&Version>,
) -> UpdateStatus {
    if !is_offered(current, &release.version) {
        return UpdateStatus::UpToDate;
    }
    let name = asset_name(&release.version, target);
    let (Some(asset), Some(checksum), Some(signature)) = (
        release.asset(&name).cloned(),
        release.asset(&checksum_name(&name)).cloned(),
        release.asset(&signature_name(&name)).cloned(),
    ) else {
        return UpdateStatus::NoBuild(release.version);
    };
    if skipped.is_some_and(|skipped| *skipped >= release.version) {
        return UpdateStatus::Skipped(release.version);
    }
    UpdateStatus::Available {
        release,
        asset,
        checksum,
        signature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::release::UpdateError;
    use proptest::prelude::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct Fake {
        name: &'static str,
        answer: Result<Release, UpdateError>,
        calls: Arc<AtomicUsize>,
    }

    impl ReleaseSource for Fake {
        fn name(&self) -> &'static str {
            self.name
        }

        fn latest(&self) -> Result<Release, UpdateError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.answer.clone()
        }
    }

    fn release(version: &str, targets: &[&str], checksums: bool) -> Release {
        let mut release = unsigned_release(version, targets, checksums);
        let names: Vec<String> = targets
            .iter()
            .map(|target| asset_name(&release.version, target))
            .collect();
        release.assets.extend(names.into_iter().map(|name| Asset {
            name: signature_name(&name),
            url: format!("https://example.invalid/{name}.sig"),
        }));
        release
    }

    fn unsigned_release(version: &str, targets: &[&str], checksums: bool) -> Release {
        let version = Version::parse(version).unwrap();
        let mut assets = Vec::new();
        for target in targets {
            let name = asset_name(&version, target);
            if checksums {
                assets.push(Asset {
                    name: checksum_name(&name),
                    url: format!("https://example.invalid/{name}.b3"),
                });
            }
            assets.push(Asset {
                url: format!("https://example.invalid/{name}"),
                name,
            });
        }
        Release {
            version,
            page: "https://example.invalid/release".into(),
            assets,
        }
    }

    fn sources(
        answers: Vec<Result<Release, UpdateError>>,
    ) -> (Vec<Box<dyn ReleaseSource + Send>>, Vec<Arc<AtomicUsize>>) {
        let names = ["GitHub", "GitLab"];
        let mut counters = Vec::new();
        let sources = answers
            .into_iter()
            .zip(names)
            .map(|(answer, name)| {
                let calls = Arc::new(AtomicUsize::new(0));
                counters.push(calls.clone());
                Box::new(Fake {
                    name,
                    answer,
                    calls,
                }) as Box<dyn ReleaseSource + Send>
            })
            .collect();
        (sources, counters)
    }

    const TARGET: &str = "linux-x86_64";

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn unit_newer_release_is_available() {
        let (sources, _) = sources(vec![Ok(release(
            "0.3.0",
            &crate::update::target::TARGETS,
            true,
        ))]);
        let status = check(&sources, &v("0.2.0"), Some(TARGET), None);
        let UpdateStatus::Available {
            asset,
            checksum,
            signature,
            ..
        } = &status
        else {
            panic!("{status:?}")
        };
        assert_eq!(asset.name, "orin-0.3.0-linux-x86_64");
        assert_eq!(checksum.name, "orin-0.3.0-linux-x86_64.b3");
        assert_eq!(signature.name, "orin-0.3.0-linux-x86_64.sig");
        assert_eq!(
            status.message("14:02"),
            "Orin 0.3.0 is available (checked 14:02)."
        );
        assert_eq!(status.offered(), Some(&v("0.3.0")));
    }

    #[test]
    fn unit_same_or_older_is_up_to_date() {
        for latest in ["0.2.0", "0.1.9"] {
            let (sources, _) = sources(vec![Ok(release(latest, &[TARGET], true))]);
            let status = check(&sources, &v("0.2.0"), Some(TARGET), None);
            assert_eq!(status, UpdateStatus::UpToDate, "{latest}");
            assert_eq!(
                status.message("09:15"),
                "Orin is up to date (checked 09:15)."
            );
        }
    }

    #[test]
    fn unit_missing_asset_or_checksum_is_no_build() {
        for answer in [
            release("0.3.0", &["windows-x86_64"], true),
            release("0.3.0", &[TARGET], false),
            // A release nobody signed is never offered, however complete it looks.
            unsigned_release("0.3.0", &[TARGET], true),
        ] {
            let (sources, _) = sources(vec![Ok(answer)]);
            let status = check(&sources, &v("0.2.0"), Some(TARGET), None);
            assert_eq!(status, UpdateStatus::NoBuild(v("0.3.0")));
            assert_eq!(
                status.message("x"),
                "Orin 0.3.0 is out, but has no build for this system yet."
            );
        }
    }

    #[test]
    fn unit_skipped_version_stays_quiet_until_a_newer_one() {
        let (quiet, _) = sources(vec![Ok(release("0.3.0", &[TARGET], true))]);
        assert_eq!(
            check(&quiet, &v("0.2.0"), Some(TARGET), Some(&v("0.3.0"))),
            UpdateStatus::Skipped(v("0.3.0"))
        );
        let (newer, _) = sources(vec![Ok(release("0.3.1", &[TARGET], true))]);
        let status = check(&newer, &v("0.2.0"), Some(TARGET), Some(&v("0.3.0")));
        assert_eq!(status.offered(), Some(&v("0.3.1")));
    }

    #[test]
    fn unit_prerelease_only_offered_to_prerelease() {
        let (sources_a, _) = sources(vec![Ok(release("0.3.0-rc.1", &[TARGET], true))]);
        assert_eq!(
            check(&sources_a, &v("0.2.0"), Some(TARGET), None),
            UpdateStatus::UpToDate
        );
        let (sources_b, _) = sources(vec![Ok(release("0.3.0-rc.2", &[TARGET], true))]);
        assert!(
            check(&sources_b, &v("0.3.0-rc.1"), Some(TARGET), None)
                .offered()
                .is_some()
        );
    }

    #[test]
    fn unit_github_failure_falls_back_to_gitlab() {
        let (fallback, calls) = sources(vec![
            Err(UpdateError::Status(403)),
            Ok(release("0.3.0", &[TARGET], true)),
        ]);
        assert!(
            check(&fallback, &v("0.2.0"), Some(TARGET), None)
                .offered()
                .is_some()
        );
        assert_eq!(calls[0].load(Ordering::SeqCst), 1);
        assert_eq!(calls[1].load(Ordering::SeqCst), 1);
        let (working, calls) = sources(vec![
            Ok(release("0.3.0", &[TARGET], true)),
            Ok(release("0.4.0", &[TARGET], true)),
        ]);
        assert_eq!(
            check(&working, &v("0.2.0"), Some(TARGET), None).offered(),
            Some(&v("0.3.0"))
        );
        assert_eq!(
            calls[1].load(Ordering::SeqCst),
            0,
            "GitLab isn't asked when GitHub works"
        );
    }

    #[test]
    fn unit_both_failures_are_reported() {
        let (sources, _) = sources(vec![
            Err(UpdateError::Status(429)),
            Err(UpdateError::Network("the request timed out".into())),
        ]);
        let status = check(&sources, &v("0.2.0"), Some(TARGET), None);
        assert_eq!(
            status.message("x"),
            "Couldn't check for updates: GitHub: the server answered 429; GitLab: the request timed out."
        );
    }

    #[test]
    fn unit_source_build_never_asks() {
        let (sources, calls) = sources(vec![Ok(release("9.0.0", &[TARGET], true))]);
        let status = check(&sources, &v("0.2.0"), None, None);
        assert_eq!(status, UpdateStatus::SourceBuild);
        assert_eq!(calls[0].load(Ordering::SeqCst), 0);
        assert!(status.message("x").contains("built from source"));
    }

    fn version() -> impl Strategy<Value = Version> {
        (0u64..4, 0u64..4, 0u64..4, prop::option::of(1u64..3)).prop_map(
            |(major, minor, patch, rc)| {
                let mut version = Version::new(major, minor, patch);
                if let Some(rc) = rc {
                    version.pre = semver::Prerelease::new(&format!("rc.{rc}")).unwrap();
                }
                version
            },
        )
    }

    proptest! {
        #[test]
        fn property_offered_exactly_when_newer_and_prerelease_rule_holds(a in version(), b in version()) {
            let (sources, _) = sources(vec![Ok(release(&b.to_string(), &[TARGET], true))]);
            let offered = check(&sources, &a, Some(TARGET), None).offered().is_some();
            prop_assert_eq!(offered, b > a && (b.pre.is_empty() || !a.pre.is_empty()));
        }
    }
}
