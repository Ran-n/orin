// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 18:02:55.217337
//+ Revised: 	2026/11/12 15:25:32.866908
// -----------------------------------------------------------------------------

use super::support::{self, ready, spawn, wait};
use orin::agents::{AgentSession, AgentStatus, Provider};
use std::{
    fs,
    process::Command,
    thread,
    time::{Duration, Instant},
};
#[test]
fn integration_authentication_failure_and_explicit_recovery_are_structured() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("auth", directory.path());
    let state = wait(&session, |state| state.status == AgentStatus::AuthRequired);
    assert!(state.error.unwrap().contains("Sign in"));
    assert_eq!(state.auth_methods[0].id, "fixture-login");
    assert!(session.authenticate("not-offered").is_err());
    session.authenticate("fixture-login").unwrap();
    ready(&session);
    session.stop().unwrap();
}

#[test]
fn integration_drop_kills_the_adapter_and_its_descendants() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("child.pid");
    let mut command = Command::new(env!("CARGO_BIN_EXE_orin-acp-fixture"));
    command
        .env("ORIN_ACP_FIXTURE_MODE", "child")
        .env("ORIN_ACP_FIXTURE_CHILD_PID", &marker);
    let session = AgentSession::spawn_command(Provider::Codex, command, directory.path()).unwrap();
    ready(&session);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !marker.exists() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    let child: u32 = fs::read_to_string(marker).unwrap().parse().unwrap();
    let parent = session.process_id();
    assert!(support::alive(parent) && support::alive(child));
    drop(session);
    assert!(!support::alive(parent));
    assert!(!support::alive(child));
}

#[test]
fn integration_parallel_agents_are_independent_and_input_is_validated() {
    let first_dir = tempfile::tempdir().unwrap();
    let second_dir = tempfile::tempdir().unwrap();
    let mut first = spawn("normal", first_dir.path());
    let mut second = spawn("normal", second_dir.path());
    ready(&first);
    ready(&second);
    assert!(first.prompt("").is_err());
    assert!(first.prompt(&"x".repeat(131_073)).is_err());
    assert!(first.cancel().is_err());
    first.prompt("hold").unwrap();
    assert!(first.prompt("second overlapping prompt").is_err());
    second.prompt("An independent prompt.").unwrap();
    let state = wait(&second, |state| {
        state.status == AgentStatus::Ready && !state.messages.is_empty()
    });
    assert!(!state.messages.iter().any(|message| message.text == "hold"));
    first.cancel().unwrap();
    ready(&first);
    first.stop().unwrap();
    second.stop().unwrap();
    second.stop().unwrap();
}

#[test]
#[ignore = "requires explicit ORIN_LIVE_AGENT=claude|codex|gemini and a real ACP adapter; no model prompt is sent"]
fn live_installed_adapter_negotiates_acp_without_a_model_prompt() {
    let name = std::env::var("ORIN_LIVE_AGENT")
        .expect("Explicitly select ORIN_LIVE_AGENT before running this ignored test.");
    let provider = serde_json::from_str::<Provider>(&format!("\"{name}\""))
        .expect("Select claude, codex, or gemini.");
    let directory = tempfile::tempdir().unwrap();
    let mut session = AgentSession::spawn(provider, directory.path()).unwrap();
    // Offline fixtures answer immediately; a real adapter may need the full download/startup deadline.
    let state = support::wait_for(&session, Duration::from_secs(310), |state| {
        matches!(
            state.status,
            AgentStatus::Ready | AgentStatus::AuthRequired | AgentStatus::Failed
        )
    });
    // A signed-out adapter has still negotiated ACP: authentication is a supported outcome.
    // This is a handshake test, not authorization to sign in or send a model prompt.
    match state.status {
        AgentStatus::Ready => assert!(state.session_id.is_some()),
        AgentStatus::AuthRequired => {
            assert!(state.session_id.is_none());
            assert!(
                state
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("Sign in"))
            );
        }
        other => panic!("live ACP negotiation failed: {other:?}: {:?}", state.error),
    }
    session.stop().unwrap();
}
