// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 18:02:55.147012
//+ Revised: 	2026/11/13 07:53:50.714319
// -----------------------------------------------------------------------------

use super::support::{self, ready, spawn, wait};
use orin::agents::{AgentSession, AgentStatus, Provider};
use std::process::Command;
#[test]
fn integration_bad_frames_version_and_process_exits_surface_errors() {
    // Each failure names its own cause, so the user knows what to fix.
    // A crash is seen either as the closed output or as the exit status, whichever comes first.
    for (mode, causes) in [
        ("wrong-version", &["ACP v1"][..]),
        ("malformed", &["Invalid ACP output"][..]),
        ("crash", &["exited (17)", "closed its output"][..]),
        ("oversized", &["Invalid ACP output"][..]),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut session = spawn(mode, directory.path());
        if matches!(mode, "crash" | "oversized") {
            ready(&session);
            session.prompt("Trigger fixture failure.").unwrap();
        }
        let state = wait(&session, |state| state.status == AgentStatus::Failed);
        let error = state.error.unwrap_or_default();
        assert!(
            causes.iter().any(|cause| error.contains(cause)),
            "{mode}: {error}"
        );
        assert!(session.prompt("Cannot send").is_err());
        session.stop().unwrap();
    }
}

#[test]
fn integration_unadvertised_fs_and_terminal_requests_cannot_execute() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("unsupported", directory.path());
    ready(&session);
    session.prompt("Probe unsupported capabilities.").unwrap();
    wait(&session, |state| {
        state.status == AgentStatus::Ready && state.stop_reason.is_some()
    });
    assert!(!directory.path().join("src/main.rs").exists());
    session.stop().unwrap();
}

#[test]
fn integration_invalid_permission_is_cancelled_without_deadlocking_or_writing() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("bad-permission", directory.path());
    ready(&session);
    session
        .prompt("An invalid permission option must never authorize a write.")
        .unwrap();
    let state = wait(&session, |state| {
        state.status == AgentStatus::Ready && state.stop_reason.as_deref() == Some("cancelled")
    });
    assert!(state.error.unwrap().contains("invalid permission"));
    assert!(!directory.path().join("src/main.rs").exists());
    session.stop().unwrap();
}

#[test]
fn regression_a_slow_cancellation_keeps_the_session() {
    // An adapter may take several seconds to stop its tools after a cancel; the session used
    // to be failed after 5 s even though the adapter was answering.
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("slow-cancel", directory.path());
    ready(&session);
    session.prompt("hold").unwrap();
    wait(&session, |state| {
        state
            .messages
            .iter()
            .any(|message| message.text == "Waiting for cancellation.")
    });
    session.cancel().unwrap();
    let state = support::wait_for(&session, std::time::Duration::from_secs(15), |state| {
        state.status == AgentStatus::Ready || state.status == AgentStatus::Failed
    });
    assert_eq!(state.status, AgentStatus::Ready, "{:?}", state.error);
    assert_eq!(state.stop_reason.as_deref(), Some("cancelled"));
    // The session still takes a new turn (this one asks for a permission).
    session.prompt("again").unwrap();
    wait(&session, |state| {
        state.status == AgentStatus::WaitingPermission
    });
    session.stop().unwrap();
}

#[test]
fn integration_unresponsive_cancellation_times_out_and_reaps_the_process() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("ignore-cancel", directory.path());
    ready(&session);
    session.prompt("hold").unwrap();
    wait(&session, |state| {
        state
            .messages
            .iter()
            .any(|message| message.text == "Waiting for cancellation.")
    });
    session.cancel().unwrap();
    assert!(session.cancel().is_err());
    // The adapter gets a grace period to wind the turn down, then the session is failed.
    let state = support::wait_for(&session, std::time::Duration::from_secs(25), |state| {
        state.status == AgentStatus::Failed
    });
    assert!(state.error.unwrap().contains("did not respond in time"));
    let process = session.process_id();
    session.stop().unwrap();
    assert!(!support::alive(process));
}

#[test]
fn integration_invalid_command_and_cwd_fail_before_a_session_is_created() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("not-an-executable");
    assert!(
        AgentSession::spawn_command(Provider::Claude, Command::new(&missing), directory.path())
            .is_err()
    );
    assert!(
        AgentSession::spawn_command(
            Provider::Claude,
            Command::new(&missing),
            &directory.path().join("missing")
        )
        .is_err()
    );
}
