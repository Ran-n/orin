// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:37.940661
//+ Revised: 	2026/11/13 07:53:51.105333
// -----------------------------------------------------------------------------

//! Downloading a release binary and proving it is the one the release published.

use super::{
    release::{Asset, UpdateError},
    source::Http,
};
use std::{
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tempfile::NamedTempFile;

/// Larger than any Orin binary will be; stops a bad server from filling the disk.
pub const MAX_DOWNLOAD: u64 = 200 * 1024 * 1024;
/// A server that keeps trickling bytes (each within the per-read timeout) is given up on after
/// this long; at the size cap that is still above 110 KiB/s.
pub const MAX_DOWNLOAD_TIME: Duration = Duration::from_secs(30 * 60);
/// A `.b3` file is one line: 64 hex digits, two spaces, and the file name.
const MAX_CHECKSUM: u64 = 1024;
/// Every temporary download starts with this, so leftovers can be told from anything else.
pub const PREFIX: &str = "orin-update-";

/// Stream `asset` into a temporary file in `dir` while hashing it, and keep it only when its
/// BLAKE3 hash matches `checksum`. `progress` gets the bytes read and the declared size.
/// Setting `cancel` stops between chunks; the temporary file is deleted whenever this fails.
pub fn download(
    http: &Http,
    asset: &Asset,
    checksum: &Asset,
    dir: &Path,
    cancel: &AtomicBool,
    progress: impl FnMut(u64, Option<u64>),
) -> Result<NamedTempFile, UpdateError> {
    download_within(
        http,
        asset,
        checksum,
        dir,
        cancel,
        progress,
        MAX_DOWNLOAD_TIME,
    )
}

fn download_within(
    http: &Http,
    asset: &Asset,
    checksum: &Asset,
    dir: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, Option<u64>),
    limit: Duration,
) -> Result<NamedTempFile, UpdateError> {
    let started = Instant::now();
    tracing::info!(target: "orin::update", asset = %asset.name, "Update download started");
    let expected = expected_hash(&http.text(&checksum.url, "text/plain", MAX_CHECKSUM)?)?;
    let (length, mut body) = http.stream(&asset.url)?;
    if length.is_some_and(|length| length > MAX_DOWNLOAD) {
        return Err(UpdateError::TooLarge(MAX_DOWNLOAD >> 20));
    }
    std::fs::create_dir_all(dir).map_err(io)?;
    let mut file = tempfile::Builder::new()
        .prefix(PREFIX)
        .tempfile_in(dir)
        .map_err(io)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut read = 0_u64;
    loop {
        if cancel.load(Ordering::Acquire) {
            tracing::info!(target: "orin::update", "Update download cancelled");
            return Err(UpdateError::Cancelled);
        }
        if started.elapsed() > limit {
            tracing::warn!(target: "orin::update", minutes = limit.as_secs() / 60, "Update download took too long");
            return Err(UpdateError::Network(format!(
                "the download took longer than {} minutes",
                limit.as_secs() / 60
            )));
        }
        let count = body.read(&mut buffer).map_err(|error| {
            UpdateError::Network(format!("the download broke off ({})", error.kind()))
        })?;
        if count == 0 {
            break;
        }
        read += count as u64;
        if read > MAX_DOWNLOAD {
            return Err(UpdateError::TooLarge(MAX_DOWNLOAD >> 20));
        }
        hasher.update(&buffer[..count]);
        file.write_all(&buffer[..count]).map_err(io)?;
        progress(read, length);
    }
    file.as_file().sync_all().map_err(io)?;
    let actual = hasher.finalize().to_hex();
    if !actual.as_str().eq_ignore_ascii_case(&expected) {
        tracing::error!(target: "orin::update", asset = %asset.name, "Update download didn't match its checksum");
        return Err(UpdateError::Checksum);
    }
    tracing::info!(target: "orin::update", asset = %asset.name, bytes = read, "Update download verified");
    Ok(file)
}

/// The hash in a `b3sum` line: the first 64 characters of the first line, which must be hex.
fn expected_hash(text: &str) -> Result<String, UpdateError> {
    let hash: String = text
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(64)
        .collect();
    if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(hash)
    } else {
        Err(UpdateError::Parse("the checksum"))
    }
}

fn io(error: std::io::Error) -> UpdateError {
    UpdateError::Io(format!("the download couldn't be saved ({})", error.kind()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::source::Timeouts;
    use std::time::Duration;

    use crate::update::fixture::{FixtureServer, Reply};

    fn http() -> Http {
        Http::new(
            Timeouts {
                connect: Duration::from_secs(1),
                total: Duration::from_secs(5),
                idle: Duration::from_secs(2),
            },
            false,
        )
    }

    fn assets(server: &FixtureServer) -> (Asset, Asset) {
        (
            Asset {
                name: "orin".into(),
                url: server.url("/orin"),
            },
            Asset {
                name: "orin.b3".into(),
                url: server.url("/orin.b3"),
            },
        )
    }

    fn b3(bytes: &[u8]) -> String {
        format!("{}  orin\n", blake3::hash(bytes).to_hex())
    }

    fn files_in(dir: &Path) -> usize {
        std::fs::read_dir(dir).map_or(0, |entries| entries.count())
    }

    #[test]
    fn unit_good_download_verifies() {
        let binary = vec![7_u8; 300_000];
        let server = FixtureServer::start(vec![
            ("/orin.b3", Reply::ok(b3(&binary))),
            ("/orin", Reply::ok(binary.clone())),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let (asset, checksum) = assets(&server);
        let file = download(
            &http(),
            &asset,
            &checksum,
            dir.path(),
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(std::fs::read(file.path()).unwrap(), binary);
        assert!(
            file.path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(PREFIX)
        );
    }

    #[test]
    fn unit_checksum_mismatch_deletes_file() {
        let server = FixtureServer::start(vec![
            ("/orin.b3", Reply::ok(b3(b"something else"))),
            ("/orin", Reply::ok(vec![1_u8; 1000])),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let (asset, checksum) = assets(&server);
        let result = download(
            &http(),
            &asset,
            &checksum,
            dir.path(),
            &AtomicBool::new(false),
            |_, _| {},
        );
        assert_eq!(result.unwrap_err(), UpdateError::Checksum);
        assert_eq!(files_in(dir.path()), 0, "the rejected download is deleted");
    }

    #[test]
    fn unit_oversize_is_refused() {
        let server = FixtureServer::start(vec![
            ("/orin.b3", Reply::ok(b3(b"x"))),
            (
                "/orin",
                Reply::ok(b"x".to_vec()).declaring_length(MAX_DOWNLOAD + 1),
            ),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let (asset, checksum) = assets(&server);
        let result = download(
            &http(),
            &asset,
            &checksum,
            dir.path(),
            &AtomicBool::new(false),
            |_, _| {},
        );
        assert_eq!(result.unwrap_err(), UpdateError::TooLarge(200));
        assert_eq!(files_in(dir.path()), 0);
    }

    #[test]
    fn unit_cancel_stops_and_cleans() {
        let binary = vec![3_u8; 1_000_000];
        let server = FixtureServer::start(vec![
            ("/orin.b3", Reply::ok(b3(&binary))),
            ("/orin", Reply::ok(binary)),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let (asset, checksum) = assets(&server);
        let cancel = AtomicBool::new(false);
        let result = download(
            &http(),
            &asset,
            &checksum,
            dir.path(),
            &cancel,
            |read, _| {
                if read > 100_000 {
                    cancel.store(true, Ordering::Release);
                }
            },
        );
        assert_eq!(result.unwrap_err(), UpdateError::Cancelled);
        assert_eq!(files_in(dir.path()), 0);
    }

    #[test]
    fn unit_a_download_slower_than_the_limit_is_abandoned_and_cleaned() {
        let binary = vec![5_u8; 64 * 1024 * 4];
        let server = FixtureServer::start(vec![
            ("/orin.b3", Reply::ok(b3(&binary))),
            ("/orin", Reply::ok(binary).paced(Duration::from_millis(30))),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let (asset, checksum) = assets(&server);
        let result = download_within(
            &http(),
            &asset,
            &checksum,
            dir.path(),
            &AtomicBool::new(false),
            |_, _| {},
            Duration::from_millis(1),
        );
        assert!(
            matches!(&result, Err(UpdateError::Network(reason)) if reason.contains("took longer")),
            "{result:?}"
        );
        assert_eq!(files_in(dir.path()), 0);
    }

    #[test]
    fn unit_a_truncated_body_is_never_kept() {
        let binary = b"the start of a binary".to_vec();
        let length = binary.len() as u64 + 1_000;
        let server = FixtureServer::start(vec![
            ("/orin.b3", Reply::ok(b3(&binary))),
            ("/orin", Reply::ok(binary).declaring_length(length)),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let (asset, checksum) = assets(&server);
        let result = download(
            &http(),
            &asset,
            &checksum,
            dir.path(),
            &AtomicBool::new(false),
            |_, _| {},
        );
        assert!(result.is_err(), "a short body is refused");
        assert_eq!(files_in(dir.path()), 0);
    }

    #[test]
    fn unit_progress_reaches_total() {
        let binary = vec![9_u8; 250_000];
        let server = FixtureServer::start(vec![
            ("/orin.b3", Reply::ok(b3(&binary))),
            ("/orin", Reply::ok(binary.clone())),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let (asset, checksum) = assets(&server);
        let mut last = (0, None);
        download(
            &http(),
            &asset,
            &checksum,
            dir.path(),
            &AtomicBool::new(false),
            |read, total| {
                assert!(read >= last.0, "progress only grows");
                last = (read, total);
            },
        )
        .unwrap();
        assert_eq!(last, (250_000, Some(250_000)));
    }

    #[test]
    fn unit_uppercase_checksum_accepted() {
        let binary = b"orin binary".to_vec();
        let server = FixtureServer::start(vec![
            ("/orin.b3", Reply::ok(b3(&binary).to_uppercase())),
            ("/orin", Reply::ok(binary)),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let (asset, checksum) = assets(&server);
        assert!(
            download(
                &http(),
                &asset,
                &checksum,
                dir.path(),
                &AtomicBool::new(false),
                |_, _| {}
            )
            .is_ok()
        );
    }

    #[test]
    fn unit_malformed_checksum_file_is_refused_before_downloading() {
        let server = FixtureServer::start(vec![("/orin.b3", Reply::ok("not a hash\n"))]);
        let dir = tempfile::tempdir().unwrap();
        let (asset, checksum) = assets(&server);
        let result = download(
            &http(),
            &asset,
            &checksum,
            dir.path(),
            &AtomicBool::new(false),
            |_, _| {},
        );
        assert_eq!(result.unwrap_err(), UpdateError::Parse("the checksum"));
        assert_eq!(server.requests("/orin"), 0);
    }
}
