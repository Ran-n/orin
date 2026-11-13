// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 09:16:12.335154
//+ Revised: 	2026/11/12 15:08:15.009888
// -----------------------------------------------------------------------------

//! Agent sign-in status, as each agent's own CLI reports it.
//!
//! Orin has no account of its own. It asks `claude auth status` and `codex login status`, and for
//! Gemini CLI it only checks whether a Google sign-in file or API key exists. It never reads,
//! stores, or sends credentials.

use super::Provider;
use crate::process::launch;
use serde::Serialize;
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

/// How long a status command may take before it is treated as unknown.
const STATUS_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum SignIn {
    /// Signed in; the detail is what the agent reports, such as the account email or method.
    SignedIn(Option<String>),
    SignedOut,
    /// The agent's CLI is not on `PATH`, so Orin can neither check nor start a sign-in.
    NoCli,
    /// The status could not be determined; the detail says why.
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Account {
    pub provider: Provider,
    /// The agent CLI used for status and sign-in, when installed.
    pub cli: Option<PathBuf>,
    pub sign_in: SignIn,
}

impl Provider {
    /// The agent's own command-line tool (the ACP adapter is a separate program).
    pub fn cli(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
        }
    }

    /// Arguments that start the agent's own interactive sign-in.
    pub fn sign_in_args(self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["auth", "login"],
            Self::Codex => &["login"],
            // Gemini CLI asks how to sign in when it starts without credentials.
            Self::Gemini => &[],
        }
    }
}

/// Where the account checks look: the search path for CLIs and the home folder.
#[derive(Clone, Debug)]
pub struct Environment {
    pub path: Option<OsString>,
    pub home: Option<PathBuf>,
    pub gemini_api_key: bool,
}

impl Environment {
    pub fn current() -> Self {
        Self {
            path: env::var_os("PATH"),
            home: directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_owned()),
            gemini_api_key: ["GEMINI_API_KEY", "GOOGLE_API_KEY"]
                .iter()
                .any(|name| env::var_os(name).is_some_and(|value| !value.is_empty())),
        }
    }
}

/// Check every agent at once; each check runs its own CLI.
pub fn check_all(environment: &Environment) -> Vec<Account> {
    thread::scope(|scope| {
        let checks =
            Provider::ALL.map(|provider| scope.spawn(move || check(provider, environment)));
        checks
            .into_iter()
            .zip(Provider::ALL)
            .map(|(check, provider)| {
                check.join().unwrap_or_else(|_| {
                    tracing::error!(target: "orin::accounts", provider = provider.label(), "account check panicked");
                    Account {
                        provider,
                        cli: None,
                        sign_in: SignIn::Unknown("The check failed unexpectedly.".into()),
                    }
                })
            })
            .collect()
    })
}

pub fn check(provider: Provider, environment: &Environment) -> Account {
    let cli = launch::resolve(Path::new(provider.cli()), environment.path.as_deref());
    let sign_in = match (&cli, provider) {
        (None, _) => SignIn::NoCli,
        (Some(cli), Provider::Claude) => match run(cli, &["auth", "status", "--json"], environment)
        {
            Ok(output) => claude_status(&output.stdout),
            Err(reason) => SignIn::Unknown(reason),
        },
        (Some(cli), Provider::Codex) => match run(cli, &["login", "status"], environment) {
            Ok(output) => codex_status(output.success, &output.stdout, &output.stderr),
            Err(reason) => SignIn::Unknown(reason),
        },
        (Some(_), Provider::Gemini) => {
            gemini_status(environment.home.as_deref(), environment.gemini_api_key)
        }
    };
    tracing::info!(
        target: "orin::accounts",
        provider = provider.label(),
        installed = cli.is_some(),
        state = ?std::mem::discriminant(&sign_in),
        "agent account checked"
    );
    Account {
        provider,
        cli,
        sign_in,
    }
}

/// Read `claude auth status --json`: `loggedIn`, plus the email or method to show.
pub fn claude_status(stdout: &str) -> SignIn {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout.trim()) else {
        return SignIn::Unknown("Claude Code reported a status Orin could not read.".into());
    };
    match value.get("loggedIn").and_then(serde_json::Value::as_bool) {
        Some(true) => SignIn::SignedIn(
            ["email", "authMethod"]
                .iter()
                .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
                .filter(|detail| !detail.is_empty())
                .map(str::to_owned),
        ),
        Some(false) => SignIn::SignedOut,
        None => SignIn::Unknown("Claude Code reported a status Orin could not read.".into()),
    }
}

/// Read `codex login status`: it succeeds and describes the login when signed in.
pub fn codex_status(success: bool, stdout: &str, stderr: &str) -> SignIn {
    let first_line = |text: &str| {
        text.lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_owned)
    };
    if success {
        SignIn::SignedIn(first_line(stdout).or_else(|| first_line(stderr)))
    } else {
        SignIn::SignedOut
    }
}

/// Gemini CLI has no status command: an API key or its Google sign-in file means signed in.
pub fn gemini_status(home: Option<&Path>, api_key: bool) -> SignIn {
    if api_key {
        return SignIn::SignedIn(Some("API key".into()));
    }
    match home {
        Some(home) if home.join(".gemini").join("oauth_creds.json").is_file() => {
            SignIn::SignedIn(Some("Google account".into()))
        }
        Some(_) => SignIn::SignedOut,
        None => SignIn::Unknown("Your home folder could not be found.".into()),
    }
}

struct Output {
    success: bool,
    stdout: String,
    stderr: String,
}

fn run(cli: &Path, args: &[&str], environment: &Environment) -> Result<Output, String> {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut command: Command = launch::command(cli, &args).map_err(|error| {
        tracing::warn!(target: "orin::accounts", %error, "status command could not be prepared");
        "The status command could not be started.".to_owned()
    })?;
    if let Some(path) = &environment.path {
        command.env("PATH", path);
    }
    let output = crate::process::capture::output(&mut command, STATUS_TIMEOUT, 1 << 20).map_err(
        |error| {
            tracing::warn!(target: "orin::accounts", %error, "status command could not complete");
            "The status check could not finish. Try again after checking the CLI.".to_owned()
        },
    )?;
    let decode = |bytes| {
        String::from_utf8(bytes).map_err(|_| {
            tracing::warn!(target: "orin::accounts", "status output was not valid text");
            "The status command returned unreadable text.".to_owned()
        })
    };
    Ok(Output {
        success: output.status.success(),
        stdout: decode(output.stdout)?,
        stderr: decode(output.stderr)?,
    })
}

/// The command line that starts `provider`'s sign-in with the exact CLI that was checked, quoted
/// for the Terminal column's shell (cmd.exe on Windows, a POSIX shell elsewhere). Using the full
/// path means the shell can't pick a different copy from its own search path.
///
/// `None` when the path can't be typed safely: on Windows, cmd.exe expands `%NAME%` even inside
/// quotes, so a path containing `%` could run something else.
pub fn sign_in_command_line(provider: Provider, cli: &Path) -> Option<String> {
    let line = std::iter::once(quote(&cli.to_string_lossy())?)
        .chain(
            provider
                .sign_in_args()
                .iter()
                .map(|argument| (*argument).to_owned()),
        )
        .collect::<Vec<_>>()
        .join(" ");
    Some(line)
}

#[cfg(windows)]
fn quote(path: &str) -> Option<String> {
    // Windows paths cannot contain quotes, so one pair keeps spaces and `&` literal for cmd.exe;
    // nothing keeps `%` literal at an interactive prompt.
    (!path.contains('%')).then(|| format!("\"{path}\""))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn regression_status_reader_does_not_wait_for_descendants_after_parent_exits() {
        // A status CLI can exit leaving a child holding its output pipe open.
        // The old reader join waited for that child instead of respecting the status deadline.
        let environment = Environment::current();
        let shell = launch::shell_program();
        let started = std::time::Instant::now();
        let output = run(
            &shell,
            &[
                "/D",
                "/C",
                "start /b ping.exe -n 5 127.0.0.1 & echo status-ready",
            ],
            &environment,
        )
        .unwrap();
        assert!(output.stdout.contains("status-ready"));
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}

#[cfg(not(windows))]
fn quote(path: &str) -> Option<String> {
    Some(format!("'{}'", path.replace('\'', r"'\''")))
}
