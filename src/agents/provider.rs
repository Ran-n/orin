// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:21.100281
//+ Revised: 	2026/11/14 01:02:25.526131
// -----------------------------------------------------------------------------

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::process::launch;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Claude,
    Codex,
    Gemini,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderStatus {
    pub available: bool,
    pub executable: Option<PathBuf>,
    pub error: Option<String>,
    /// The npm package `npx` fetches and runs, when the adapter itself isn't installed.
    pub package: Option<&'static str>,
}

impl Provider {
    pub const ALL: [Self; 3] = [Self::Claude, Self::Codex, Self::Gemini];

    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Gemini => "Gemini CLI",
        }
    }

    pub fn program(self) -> &'static str {
        match self {
            Self::Claude => "claude-agent-acp",
            Self::Codex => "codex-acp",
            Self::Gemini => "gemini",
        }
    }

    /// The adapter's npm package, run with `npx` when the adapter isn't on `PATH`. The version
    /// range follows the adapter's current major release so new models arrive without an
    /// Orin update while breaking releases don't.
    pub fn package(self) -> &'static str {
        match self {
            Self::Claude => "@agentclientprotocol/claude-agent-acp@0",
            Self::Codex => "@agentclientprotocol/codex-acp@2",
            Self::Gemini => "@google/gemini-cli@0",
        }
    }

    pub fn install_hint(self) -> &'static str {
        match self {
            Self::Claude => {
                "Install Node.js so Orin can fetch Claude's adapter, then sign in with Claude Code."
            }
            Self::Codex => {
                "Install Node.js so Orin can fetch Codex's adapter, then sign in with Codex."
            }
            Self::Gemini => {
                "Install Node.js so Orin can fetch Gemini CLI, then sign in with Gemini CLI."
            }
        }
    }

    /// The vendor's own installation guide for the agent's command-line tool.
    pub fn setup_url(self) -> &'static str {
        match self {
            Self::Claude => "https://code.claude.com/docs/en/setup",
            Self::Codex => "https://developers.openai.com/codex/cli/",
            Self::Gemini => "https://geminicli.com/docs/get-started/installation/",
        }
    }

    pub fn probe(self) -> ProviderStatus {
        self.probe_in(launch::tool_path(&[], true).as_deref())
    }

    /// Find the adapter on `path`, or else `npx` to fetch it on first use.
    pub fn probe_in(self, path: Option<&std::ffi::OsStr>) -> ProviderStatus {
        let status = if let Some(executable) = launch::resolve(Path::new(self.program()), path) {
            ProviderStatus {
                available: true,
                executable: Some(executable),
                error: None,
                package: None,
            }
        } else if let Some(npx) = launch::resolve(Path::new("npx"), path) {
            ProviderStatus {
                available: true,
                executable: Some(npx),
                error: None,
                package: Some(self.package()),
            }
        } else {
            ProviderStatus {
                available: false,
                executable: None,
                error: Some(format!(
                    "{} is not on PATH and Node.js (npx) isn't installed to fetch it. {}",
                    self.program(),
                    self.install_hint()
                )),
                package: None,
            }
        };
        tracing::debug!(
            provider = self.label(),
            available = status.available,
            via_npx = status.package.is_some(),
            "ACP adapter probed"
        );
        status
    }

    /// The launch command, and whether it may first download the adapter.
    pub(super) fn command(self) -> Result<(Command, bool)> {
        self.command_in(launch::tool_path(&[], true).as_deref())
    }

    pub(super) fn command_in(self, path: Option<&std::ffi::OsStr>) -> Result<(Command, bool)> {
        let status = self.probe_in(path);
        let executable = status
            .executable
            .context(status.error.unwrap_or_default())?;
        let args = self.arguments(status.package);
        let mut command = launch::command(&executable, &args)?;
        if let Some(path) = path {
            command.env("PATH", path);
        }
        Ok((command, status.package.is_some()))
    }

    /// Arguments for the adapter itself, or for `npx` running `package`.
    pub fn arguments(self, package: Option<&str>) -> Vec<OsString> {
        let mut args: Vec<OsString> = Vec::new();
        if let Some(package) = package {
            args.extend(["-y".into(), package.into()]);
        }
        if self == Self::Gemini {
            args.push("--acp".into());
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, fs, path::Path};

    /// A stand-in program named `name` in `directory`; it is never run.
    fn stand_in(directory: &Path, name: &str) {
        #[cfg(windows)]
        fs::write(directory.join(format!("{name}.cmd")), "@exit /b 0\r\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = directory.join(name);
            fs::write(&path, "#!/bin/sh\n").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn path_of(directory: &Path) -> OsString {
        directory.as_os_str().to_owned()
    }

    #[test]
    fn unit_an_adapter_on_path_is_used_directly() {
        let directory = tempfile::tempdir().unwrap();
        stand_in(directory.path(), "claude-agent-acp");
        stand_in(directory.path(), "npx");
        let status = Provider::Claude.probe_in(Some(&path_of(directory.path())));
        assert!(status.available);
        assert_eq!(status.package, None);
        assert!(
            status
                .executable
                .unwrap()
                .file_stem()
                .is_some_and(|stem| stem == "claude-agent-acp")
        );
    }

    #[test]
    fn unit_without_the_adapter_npx_fetches_its_package() {
        let directory = tempfile::tempdir().unwrap();
        stand_in(directory.path(), "npx");
        for provider in [Provider::Claude, Provider::Codex, Provider::Gemini] {
            let status = provider.probe_in(Some(&path_of(directory.path())));
            assert!(status.available, "{provider:?}");
            assert_eq!(status.package, Some(provider.package()));
            assert!(
                status
                    .executable
                    .unwrap()
                    .file_stem()
                    .is_some_and(|stem| stem == "npx")
            );
        }
        let arguments = Provider::Gemini.arguments(Some(Provider::Gemini.package()));
        assert_eq!(
            arguments,
            ["-y", "@google/gemini-cli@0", "--acp"].map(OsString::from)
        );
        assert_eq!(Provider::Codex.arguments(None), Vec::<OsString>::new());
    }

    #[test]
    fn unit_without_node_the_agent_is_unavailable_and_says_why() {
        let directory = tempfile::tempdir().unwrap();
        let status = Provider::Codex.probe_in(Some(&path_of(directory.path())));
        assert!(!status.available);
        assert_eq!(status.executable, None);
        let error = status.error.unwrap();
        assert!(
            error.contains("codex-acp") && error.contains("Node.js"),
            "{error}"
        );
        assert!(!Provider::Codex.probe_in(None).available);
    }
}
