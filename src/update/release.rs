// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:37.535812
//+ Revised: 	2026/11/12 22:43:37.535812
// -----------------------------------------------------------------------------

//! A published release as the forges describe it, parsed without touching the network.

use serde_json::Value;

/// Why an update step failed, in words the status line and the error banner can show.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("{0}")]
    Network(String),
    #[error("the server answered {0}")]
    Status(u16),
    #[error("the answer was missing {0}")]
    Parse(&'static str),
    #[error("{0:?} is not a version number")]
    Version(String),
    #[error("the download didn't match its checksum")]
    Checksum,
    #[error("the download is larger than {0} MiB")]
    TooLarge(u64),
    #[error("the download was cancelled")]
    Cancelled,
    #[error("{0}")]
    Io(String),
}

/// One downloadable file of a release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

/// The newest release a source reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: semver::Version,
    /// The release's web page, shown when Orin can't install it itself.
    pub page: String,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == name)
    }
}

/// `GET /repos/{owner}/{repo}/releases/latest` from the GitHub REST API.
pub fn parse_github(json: &str) -> Result<Release, UpdateError> {
    let value: Value = serde_json::from_str(json).map_err(|_| UpdateError::Parse("JSON"))?;
    let assets = value["assets"]
        .as_array()
        .ok_or(UpdateError::Parse("assets"))?
        .iter()
        .filter_map(|asset| asset_from(asset, "browser_download_url"))
        .collect();
    release_from(&value, value["html_url"].as_str(), assets)
}

/// `GET /projects/{id}/releases/permalink/latest` from the GitLab REST API.
pub fn parse_gitlab(json: &str) -> Result<Release, UpdateError> {
    let value: Value = serde_json::from_str(json).map_err(|_| UpdateError::Parse("JSON"))?;
    let assets = value["assets"]["links"]
        .as_array()
        .ok_or(UpdateError::Parse("assets"))?
        .iter()
        .filter_map(|link| asset_from(link, "direct_asset_url").or_else(|| asset_from(link, "url")))
        .collect();
    release_from(&value, value["_links"]["self"].as_str(), assets)
}

fn asset_from(value: &Value, url_field: &str) -> Option<Asset> {
    Some(Asset {
        name: value["name"].as_str()?.to_owned(),
        url: value[url_field].as_str()?.to_owned(),
    })
}

fn release_from(
    value: &Value,
    page: Option<&str>,
    assets: Vec<Asset>,
) -> Result<Release, UpdateError> {
    let tag = value["tag_name"]
        .as_str()
        .ok_or(UpdateError::Parse("tag_name"))?;
    Ok(Release {
        version: parse_tag(tag)?,
        page: page
            .ok_or(UpdateError::Parse("the release page"))?
            .to_owned(),
        assets,
    })
}

/// `v0.3.0` or `0.3.0` as a version.
pub fn parse_tag(tag: &str) -> Result<semver::Version, UpdateError> {
    let trimmed = tag.trim();
    semver::Version::parse(trimmed.strip_prefix('v').unwrap_or(trimmed))
        .map_err(|_| UpdateError::Version(tag.chars().take(64).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GITHUB: &str = include_str!("../../tests/golden/update/github-latest.json");
    const GITLAB: &str = include_str!("../../tests/golden/update/gitlab-latest.json");

    #[test]
    fn unit_parses_github_latest() {
        let release = parse_github(GITHUB).unwrap();
        assert_eq!(release.version, semver::Version::new(0, 3, 0));
        assert_eq!(
            release.page,
            "https://github.com/Ran-n/orin/releases/tag/v0.3.0"
        );
        assert_eq!(release.assets.len(), 6);
        assert_eq!(
            release.asset("orin-0.3.0-linux-x86_64.b3").unwrap().url,
            "https://github.com/Ran-n/orin/releases/download/v0.3.0/orin-0.3.0-linux-x86_64.b3"
        );
    }

    #[test]
    fn unit_parses_gitlab_latest() {
        let release = parse_gitlab(GITLAB).unwrap();
        assert_eq!(release.version, semver::Version::new(0, 3, 0));
        assert_eq!(
            release.page,
            "https://gitlab.com/Ran-n/orin/-/releases/v0.3.0"
        );
        assert_eq!(release.assets.len(), 6);
        assert!(
            release
                .asset("orin-0.3.0-windows-x86_64.exe")
                .unwrap()
                .url
                .ends_with("/orin-0.3.0-windows-x86_64.exe")
        );
    }

    #[test]
    fn unit_rejects_bad_tag() {
        for tag in ["latest", "", "v", "1.2"] {
            assert!(
                matches!(parse_tag(tag), Err(UpdateError::Version(_))),
                "{tag:?}"
            );
        }
        let json = r#"{"tag_name":"latest","html_url":"https://x","assets":[]}"#;
        assert!(matches!(parse_github(json), Err(UpdateError::Version(_))));
    }

    #[test]
    fn unit_strips_v_prefix() {
        assert_eq!(parse_tag("v1.2.3").unwrap(), semver::Version::new(1, 2, 3));
        assert_eq!(parse_tag("1.2.3").unwrap(), semver::Version::new(1, 2, 3));
        assert_eq!(parse_tag("v0.2.0-rc.1").unwrap().pre.as_str(), "rc.1");
    }

    #[test]
    fn unit_missing_fields_are_parse_errors() {
        assert_eq!(parse_github("not json"), Err(UpdateError::Parse("JSON")));
        assert_eq!(
            parse_github(r#"{"tag_name":"v1.0.0","html_url":"https://x"}"#),
            Err(UpdateError::Parse("assets"))
        );
        assert_eq!(
            parse_gitlab(r#"{"tag_name":"v1.0.0","assets":{"links":[]}}"#),
            Err(UpdateError::Parse("the release page"))
        );
    }
}
