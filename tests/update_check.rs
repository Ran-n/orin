// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:38.534086
//+ Revised: 	2026/11/12 22:43:38.534086
// -----------------------------------------------------------------------------

//! The real HTTP sources against a local server: parsing, falling back, and timing out.

#[path = "update/fixture.rs"]
mod fixture;

use fixture::{FixtureServer, Reply};
use orin::update::{
    check::{UpdateStatus, check},
    source::{GitHub, GitLab, Http, ReleaseSource, Timeouts},
};
use std::time::{Duration, Instant};

const GITHUB: &str = include_str!("golden/update/github-latest.json");
const GITLAB: &str = include_str!("golden/update/gitlab-latest.json");

fn http() -> Http {
    Http::new(
        Timeouts {
            connect: Duration::from_secs(1),
            total: Duration::from_secs(1),
            idle: Duration::from_secs(1),
        },
        false,
    )
}

fn sources(server: &FixtureServer) -> Vec<Box<dyn ReleaseSource + Send>> {
    vec![
        Box::new(GitHub {
            url: server.url("/github"),
            http: http(),
        }),
        Box::new(GitLab {
            url: server.url("/gitlab"),
            http: http(),
        }),
    ]
}

fn run(server: &FixtureServer) -> UpdateStatus {
    check(
        &sources(server),
        &semver::Version::new(0, 2, 0),
        Some("linux-x86_64"),
        None,
    )
}

fn offered_by(status: &UpdateStatus) -> Option<String> {
    match status {
        UpdateStatus::Available { asset, .. } => Some(asset.url.clone()),
        _ => None,
    }
}

#[test]
fn integration_github_answer_is_parsed_and_gitlab_is_not_asked() {
    let server = FixtureServer::start(vec![
        ("/github", Reply::ok(GITHUB)),
        ("/gitlab", Reply::ok(GITLAB)),
    ]);
    let url = offered_by(&run(&server)).expect("0.3.0 is offered");
    assert!(url.starts_with("https://github.com/"), "{url}");
    assert_eq!(server.requests("/github"), 1);
    assert_eq!(server.requests("/gitlab"), 0);
}

#[test]
fn integration_rate_limits_and_server_errors_fall_back_to_gitlab() {
    for status in [403, 429, 500] {
        let server = FixtureServer::start(vec![
            ("/github", Reply::status(status)),
            ("/gitlab", Reply::ok(GITLAB)),
        ]);
        let url = offered_by(&run(&server)).unwrap_or_default();
        assert!(url.starts_with("https://gitlab.com/"), "{status}: {url}");
        assert_eq!(server.requests("/gitlab"), 1);
    }
}

#[test]
fn integration_unparsable_github_answer_falls_back() {
    let server = FixtureServer::start(vec![
        ("/github", Reply::ok("<html>maintenance</html>")),
        ("/gitlab", Reply::ok(GITLAB)),
    ]);
    assert!(offered_by(&run(&server)).is_some());
}

#[test]
fn integration_a_hanging_source_times_out_and_both_failures_are_reported() {
    let server = FixtureServer::start(vec![
        ("/github", Reply::hang()),
        ("/gitlab", Reply::status(503)),
    ]);
    let started = Instant::now();
    let status = run(&server);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(
        status.message("x"),
        "Couldn't check for updates: GitHub: the request timed out; GitLab: the server answered 503."
    );
}

#[test]
fn integration_requests_carry_no_identifying_data() {
    let server = FixtureServer::start(vec![("/github", Reply::ok(GITHUB))]);
    run(&server);
    assert_eq!(server.total_requests(), 1);
    let headers = server.headers("/github");
    let names: Vec<&str> = headers.iter().map(|(name, _)| name.as_str()).collect();
    for name in &names {
        assert!(
            ["host", "accept", "user-agent", "accept-encoding"].contains(name),
            "unexpected header {name}: {headers:?}"
        );
    }
    let value = |wanted: &str| {
        headers
            .iter()
            .find(|(name, _)| name == wanted)
            .map(|(_, value)| value.clone())
    };
    assert_eq!(
        value("user-agent"),
        Some(format!("Orin/{}", env!("CARGO_PKG_VERSION")))
    );
    assert_eq!(
        value("accept").as_deref(),
        Some("application/vnd.github+json")
    );
}
