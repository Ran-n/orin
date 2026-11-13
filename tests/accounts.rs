// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 09:16:12.638044
//+ Revised: 	2026/11/12 15:30:03.160430
// -----------------------------------------------------------------------------

//! Agent sign-in status read from each agent's own CLI, using stand-in CLIs in a temporary folder.

use orin::agents::{
    Provider,
    accounts::{
        self, Environment, SignIn, claude_status, codex_status, gemini_status, sign_in_command_line,
    },
};
use std::{fs, path::Path};
use tempfile::TempDir;

/// Write a stand-in CLI named `name` that prints `stdout` and exits with `code`.
fn fake_cli(folder: &Path, name: &str, stdout: &str, code: i32) {
    #[cfg(windows)]
    {
        let lines: String = stdout
            .lines()
            .map(|line| format!("echo {line}\r\n"))
            .collect();
        fs::write(
            folder.join(format!("{name}.cmd")),
            format!("@echo off\r\n{lines}exit /b {code}\r\n"),
        )
        .unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = folder.join(name);
        fs::write(
            &path,
            // PATH deliberately contains only fixtures; use the shell's builtin, not `cat`.
            format!(
                "#!/bin/sh\nprintf '%s\\n' '{}'\nexit {code}\n",
                stdout.replace('\'', r"'\''")
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn environment(folder: &Path) -> Environment {
    Environment {
        path: Some(folder.as_os_str().to_owned()),
        home: Some(folder.to_owned()),
        gemini_api_key: false,
    }
}

#[test]
fn claude_status_reads_logged_in_and_the_account_email() {
    assert_eq!(
        claude_status(r#"{"loggedIn":true,"email":"me@example.com","authMethod":"claude.ai"}"#),
        SignIn::SignedIn(Some("me@example.com".into()))
    );
    assert_eq!(
        claude_status(r#"{"loggedIn":true,"authMethod":"api_key"}"#),
        SignIn::SignedIn(Some("api_key".into()))
    );
    assert_eq!(claude_status(r#"{"loggedIn":false}"#), SignIn::SignedOut);
    assert!(matches!(claude_status("not json"), SignIn::Unknown(_)));
    assert!(matches!(
        claude_status(r#"{"other":1}"#),
        SignIn::Unknown(_)
    ));
}

#[test]
fn codex_status_uses_the_exit_code_and_first_line() {
    assert_eq!(
        codex_status(true, "\n  Logged in using ChatGPT\nmore", ""),
        SignIn::SignedIn(Some("Logged in using ChatGPT".into()))
    );
    assert_eq!(
        codex_status(true, "", "Logged in using an API key"),
        SignIn::SignedIn(Some("Logged in using an API key".into()))
    );
    assert_eq!(codex_status(false, "Not logged in", ""), SignIn::SignedOut);
}

#[test]
fn gemini_status_checks_for_a_key_or_a_google_sign_in_without_reading_it() {
    let home = TempDir::new().unwrap();
    assert_eq!(gemini_status(Some(home.path()), false), SignIn::SignedOut);
    assert_eq!(
        gemini_status(Some(home.path()), true),
        SignIn::SignedIn(Some("API key".into()))
    );
    fs::create_dir_all(home.path().join(".gemini")).unwrap();
    fs::write(home.path().join(".gemini/oauth_creds.json"), "secret").unwrap();
    assert_eq!(
        gemini_status(Some(home.path()), false),
        SignIn::SignedIn(Some("Google account".into()))
    );
    assert!(matches!(gemini_status(None, false), SignIn::Unknown(_)));
}

#[test]
fn missing_clis_are_reported_as_not_installed() {
    let empty = TempDir::new().unwrap();
    for account in accounts::check_all(&environment(empty.path())) {
        assert_eq!(account.sign_in, SignIn::NoCli, "{:?}", account.provider);
        assert!(account.cli.is_none());
    }
}

#[test]
fn installed_clis_are_asked_for_their_status() {
    let folder = TempDir::new().unwrap();
    fake_cli(
        folder.path(),
        "claude",
        r#"{"loggedIn":true,"email":"me@example.com"}"#,
        0,
    );
    fake_cli(folder.path(), "codex", "Not logged in", 1);
    fake_cli(folder.path(), "gemini", "", 0);
    let checked = accounts::check_all(&environment(folder.path()));
    let state = |provider: Provider| {
        checked
            .iter()
            .find(|account| account.provider == provider)
            .map(|account| account.sign_in.clone())
            .unwrap()
    };
    assert_eq!(
        state(Provider::Claude),
        SignIn::SignedIn(Some("me@example.com".into()))
    );
    assert_eq!(state(Provider::Codex), SignIn::SignedOut);
    assert_eq!(state(Provider::Gemini), SignIn::SignedOut);
    assert!(checked.iter().all(|account| account.cli.is_some()));
}

#[test]
fn sign_in_runs_each_agents_own_login_through_the_checked_cli() {
    #[cfg(windows)]
    {
        let folder = Path::new(r"C:\Program Files\agents");
        assert_eq!(
            sign_in_command_line(Provider::Claude, &folder.join("claude.cmd")).unwrap(),
            r#""C:\Program Files\agents\claude.cmd" auth login"#
        );
        assert_eq!(
            sign_in_command_line(Provider::Codex, &folder.join("codex.cmd")).unwrap(),
            r#""C:\Program Files\agents\codex.cmd" login"#
        );
        assert_eq!(
            sign_in_command_line(Provider::Gemini, &folder.join("gemini.cmd")).unwrap(),
            r#""C:\Program Files\agents\gemini.cmd""#
        );
        // cmd.exe expands %NAME% even inside quotes, so such a path is never typed.
        assert_eq!(
            sign_in_command_line(Provider::Claude, Path::new(r"C:\%TEMP%\claude.cmd")),
            None
        );
    }
    #[cfg(unix)]
    {
        assert_eq!(
            sign_in_command_line(Provider::Claude, Path::new("/opt/my agents/claude")).unwrap(),
            "'/opt/my agents/claude' auth login"
        );
        assert_eq!(
            sign_in_command_line(Provider::Codex, Path::new("/tmp/it's/codex")).unwrap(),
            r"'/tmp/it'\''s/codex' login"
        );
    }
}
