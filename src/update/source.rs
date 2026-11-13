// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:37.670549
//+ Revised: 	2026/11/12 23:40:35.762423
// -----------------------------------------------------------------------------

//! Where Orin asks for the latest release: GitHub first, GitLab when GitHub can't answer.

use super::release::{Release, UpdateError, parse_github, parse_gitlab};
use std::{io::Read, time::Duration};

pub const GITHUB_LATEST: &str = "https://api.github.com/repos/Ran-n/orin/releases/latest";
pub const GITLAB_LATEST: &str =
    "https://gitlab.com/api/v4/projects/Ran-n%2Forin/releases/permalink/latest";
/// The largest release description accepted; real ones are a few KiB.
const MAX_RELEASE_JSON: u64 = 1024 * 1024;

/// A place that knows the latest release (Strategy pattern, so checks are testable with fakes).
pub trait ReleaseSource {
    fn name(&self) -> &'static str;
    fn latest(&self) -> Result<Release, UpdateError>;
}

/// How long a request may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeouts {
    pub connect: Duration,
    /// For a whole release check.
    pub total: Duration,
    /// For a download: the longest wait for the next bytes.
    pub idle: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            total: Duration::from_secs(30),
            idle: Duration::from_secs(60),
        }
    }
}

/// The one HTTP client the updater uses. Requests carry only the URL, `Accept`, and
/// `User-Agent: Orin/<version>`: no identifier, so the "no telemetry" promise holds.
#[derive(Clone)]
pub struct Http {
    check: ureq::Agent,
    download: ureq::Agent,
}

impl Http {
    /// `https_only` is relaxed only for the debug-build test override.
    pub fn new(timeouts: Timeouts, https_only: bool) -> Self {
        let agent = |total: Option<Duration>, idle: Option<Duration>| {
            ureq::Agent::config_builder()
                .https_only(https_only)
                .http_status_as_error(false)
                .user_agent(format!("Orin/{}", env!("CARGO_PKG_VERSION")))
                .timeout_connect(Some(timeouts.connect))
                .timeout_global(total)
                .timeout_per_read(idle)
                .build()
                .new_agent()
        };
        Self {
            check: agent(Some(timeouts.total), None),
            download: agent(None, Some(timeouts.idle)),
        }
    }

    /// GET `url` as text of at most `limit` bytes.
    pub fn text(&self, url: &str, accept: &str, limit: u64) -> Result<String, UpdateError> {
        let mut response = self
            .check
            .get(url)
            .header("Accept", accept)
            .call()
            .map_err(network)?;
        expect_ok(response.status().as_u16())?;
        response
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_string()
            .map_err(network)
    }

    /// GET `url` for streaming: its declared length and a reader with no overall time limit.
    pub fn stream(&self, url: &str) -> Result<(Option<u64>, impl Read + use<>), UpdateError> {
        let response = self
            .download
            .get(url)
            .header("Accept", "application/octet-stream")
            .call()
            .map_err(network)?;
        expect_ok(response.status().as_u16())?;
        let length = response.body().content_length();
        Ok((length, response.into_body().into_reader()))
    }
}

fn expect_ok(status: u16) -> Result<(), UpdateError> {
    if status == 200 {
        Ok(())
    } else {
        Err(UpdateError::Status(status))
    }
}

/// A short reason without URLs or headers, so nothing private reaches logs or the screen.
fn network(error: ureq::Error) -> UpdateError {
    UpdateError::Network(match error {
        ureq::Error::Timeout(_) => "the request timed out".into(),
        ureq::Error::HostNotFound => "the server couldn't be found".into(),
        ureq::Error::ConnectionFailed => "the connection failed".into(),
        ureq::Error::BodyExceedsLimit(_) => "the answer was too large".into(),
        ureq::Error::RequireHttpsOnly(_) => "the server didn't use HTTPS".into(),
        ureq::Error::Io(error) => format!("the connection broke ({})", error.kind()),
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => "the secure connection failed".into(),
        _ => "the request failed".into(),
    })
}

pub struct GitHub {
    pub url: String,
    pub http: Http,
}

impl ReleaseSource for GitHub {
    fn name(&self) -> &'static str {
        "GitHub"
    }

    fn latest(&self) -> Result<Release, UpdateError> {
        let json = self
            .http
            .text(&self.url, "application/vnd.github+json", MAX_RELEASE_JSON)?;
        parse_github(&json)
    }
}

pub struct GitLab {
    pub url: String,
    pub http: Http,
}

impl ReleaseSource for GitLab {
    fn name(&self) -> &'static str {
        "GitLab"
    }

    fn latest(&self) -> Result<Release, UpdateError> {
        let json = self
            .http
            .text(&self.url, "application/json", MAX_RELEASE_JSON)?;
        parse_gitlab(&json)
    }
}

/// The sources in the order they are asked, and the client their downloads use.
///
/// Debug builds honour `ORIN_UPDATE_SOURCES=<github-url>|<gitlab-url>` so tests can serve
/// releases locally; a release build always asks the real forges over HTTPS.
pub fn sources(timeouts: Timeouts) -> (Vec<Box<dyn ReleaseSource + Send>>, Http) {
    let redirect = cfg!(debug_assertions)
        .then(|| std::env::var("ORIN_UPDATE_SOURCES").ok())
        .flatten();
    sources_from(redirect.as_deref(), timeouts)
}

/// [`sources`] with the override passed in: `Some("<github-url>|<gitlab-url>")` points both
/// sources elsewhere and allows plain HTTP; anything without a `|` is ignored.
fn sources_from(
    redirect: Option<&str>,
    timeouts: Timeouts,
) -> (Vec<Box<dyn ReleaseSource + Send>>, Http) {
    let redirected = redirect.and_then(|value| {
        let (github, gitlab) = value.split_once('|')?;
        Some((github.to_owned(), gitlab.to_owned()))
    });
    let http = Http::new(timeouts, redirected.is_none());
    let (github, gitlab) =
        redirected.unwrap_or_else(|| (GITHUB_LATEST.into(), GITLAB_LATEST.into()));
    let sources: Vec<Box<dyn ReleaseSource + Send>> = vec![
        Box::new(GitHub {
            url: github,
            http: http.clone(),
        }),
        Box::new(GitLab {
            url: gitlab,
            http: http.clone(),
        }),
    ];
    (sources, http)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::fixture::{FixtureServer, Reply};

    fn quick() -> Timeouts {
        Timeouts {
            connect: Duration::from_secs(1),
            total: Duration::from_secs(2),
            idle: Duration::from_secs(2),
        }
    }

    fn names(sources: &[Box<dyn ReleaseSource + Send>]) -> Vec<&'static str> {
        sources.iter().map(|source| source.name()).collect()
    }

    #[test]
    fn unit_github_is_asked_before_gitlab() {
        let (sources, _) = sources_from(None, quick());
        assert_eq!(names(&sources), ["GitHub", "GitLab"]);
    }

    #[test]
    fn unit_real_sources_refuse_plain_http() {
        let (_, http) = sources_from(None, quick());
        assert_eq!(
            http.text("http://127.0.0.1:9/latest", "application/json", 10),
            Err(UpdateError::Network("the server didn't use HTTPS".into()))
        );
    }

    #[test]
    fn unit_override_points_both_sources_at_another_server() {
        let github = include_str!("../../tests/golden/update/github-latest.json");
        let gitlab = include_str!("../../tests/golden/update/gitlab-latest.json");
        let server =
            FixtureServer::start(vec![("/gh", Reply::ok(github)), ("/gl", Reply::ok(gitlab))]);
        let redirect = format!("{}|{}", server.url("/gh"), server.url("/gl"));
        let (sources, _) = sources_from(Some(&redirect), quick());
        assert_eq!(names(&sources), ["GitHub", "GitLab"]);
        for source in &sources {
            assert_eq!(
                source.latest().unwrap().version,
                semver::Version::new(0, 3, 0)
            );
        }
        assert_eq!((server.requests("/gh"), server.requests("/gl")), (1, 1));
        // Without a separator the override is ignored, so the real HTTPS sources stay.
        let (_, http) = sources_from(Some("no-separator"), quick());
        assert!(
            http.text(&server.url("/gh"), "application/json", 10)
                .is_err()
        );
    }

    #[test]
    fn unit_refused_connections_and_oversized_answers_have_plain_reasons() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", closed.local_addr().unwrap());
        drop(closed);
        let http = Http::new(quick(), false);
        let Err(UpdateError::Network(reason)) = http.text(&url, "application/json", 10) else {
            panic!("a closed port must fail")
        };
        assert!(
            // Windows retries a refused localhost connection until the connect timeout.
            reason == "the connection failed"
                || reason == "the request timed out"
                || reason.starts_with("the connection broke"),
            "{reason}"
        );
        let server = FixtureServer::start(vec![("/big", Reply::ok(vec![b'x'; 100]))]);
        assert_eq!(
            http.text(&server.url("/big"), "text/plain", 10),
            Err(UpdateError::Network("the answer was too large".into()))
        );
    }

    #[test]
    fn unit_streams_report_their_length_and_refuse_errors() {
        let server = FixtureServer::start(vec![
            ("/file", Reply::ok(vec![7_u8; 1234])),
            ("/missing", Reply::status(404)),
        ]);
        let http = Http::new(quick(), false);
        let (length, mut body) = http.stream(&server.url("/file")).unwrap();
        assert_eq!(length, Some(1234));
        let mut bytes = Vec::new();
        body.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes.len(), 1234);
        assert!(matches!(
            http.stream(&server.url("/missing")),
            Err(UpdateError::Status(404))
        ));
    }
}
