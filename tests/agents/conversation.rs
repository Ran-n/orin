// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 18:02:55.077310
//+ Revised: 	2026/11/12 09:16:43.033503
// -----------------------------------------------------------------------------

use super::support::{ready, spawn, wait};
use orin::agents::{AgentStatus, MessageRole};
use serde_json::Value;
use std::fs;
#[test]
fn integration_handshake_streaming_and_wire_contract_are_real_subprocess_io() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("normal", directory.path());
    ready(&session);
    session
        .prompt("Explain this project. 智慧\nDo not lose Unicode.")
        .unwrap();
    let state = wait(&session, |state| {
        state.status == AgentStatus::Ready && !state.messages.is_empty()
    });
    assert_eq!(state.messages[0].role, MessageRole::User);
    assert!(state.messages[0].text.contains("智慧\n"));
    assert!(
        state
            .messages
            .iter()
            .any(|message| message.role == MessageRole::Thought)
    );
    assert!(
        state
            .messages
            .iter()
            .any(|message| message.text == "I can update src/main.rs.")
    );
    let frames: Vec<Value> = fs::read_to_string(directory.path().join("wire.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let golden: Value = serde_json::from_str(include_str!("golden/initialize-v1.json")).unwrap();
    assert_eq!(frames[0], golden);
    assert_eq!(frames[1]["method"], "session/new");
    assert_eq!(frames[1]["params"]["mcpServers"], serde_json::json!([]));
    assert_eq!(frames[2]["method"], "session/prompt");
    assert_eq!(frames[2]["params"]["sessionId"], "fixture-session");
    session.stop().unwrap();
}

#[test]
fn integration_permission_is_explicit_and_tracks_a_real_file_change() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("permission", directory.path());
    ready(&session);
    session.prompt("Update the entrypoint.").unwrap();
    let pending = wait(&session, |state| {
        state.status == AgentStatus::WaitingPermission
    });
    assert!(!directory.path().join("src/main.rs").exists());
    let permission = &pending.pending_permissions[0];
    assert_eq!(permission.options[0].kind, "allow_once");
    assert!(
        session
            .respond_permission(&permission.request_id, Some("unoffered-option"))
            .is_err()
    );
    assert_eq!(session.snapshot().pending_permissions.len(), 1);
    session
        .respond_permission(&permission.request_id, Some("allow-once"))
        .unwrap();
    let state = wait(&session, |state| {
        state.status == AgentStatus::Ready && state.stop_reason.is_some()
    });
    assert_eq!(state.tool_calls[0].status, "completed");
    assert!(
        state
            .changed_files
            .iter()
            .any(|path| path.ends_with("src/main.rs"))
    );
    assert!(
        fs::read_to_string(directory.path().join("src/main.rs"))
            .unwrap()
            .contains("Created with Orin")
    );
    assert!(state.pending_permissions.is_empty());
    session.stop().unwrap();
}

#[test]
fn integration_rejected_permission_never_writes_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("permission", directory.path());
    ready(&session);
    session.prompt("Reject this change.").unwrap();
    let pending = wait(&session, |state| !state.pending_permissions.is_empty());
    session
        .respond_permission(
            &pending.pending_permissions[0].request_id,
            Some("reject-once"),
        )
        .unwrap();
    let state = wait(&session, |state| {
        state.status == AgentStatus::Ready && state.stop_reason.is_some()
    });
    assert!(!directory.path().join("src/main.rs").exists());
    assert!(state.changed_files.is_empty());
    assert!(
        state
            .messages
            .iter()
            .any(|message| message.text.contains("Change rejected."))
    );
    session.stop().unwrap();
}

#[test]
fn integration_cancel_answers_pending_permissions_before_cancelling_the_turn() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("permission", directory.path());
    ready(&session);
    session.prompt("Cancel this change.").unwrap();
    wait(&session, |state| !state.pending_permissions.is_empty());
    session.cancel().unwrap();
    let state = wait(&session, |state| {
        state.status == AgentStatus::Ready && state.stop_reason.as_deref() == Some("cancelled")
    });
    assert!(state.pending_permissions.is_empty());
    assert!(!directory.path().join("src/main.rs").exists());
    let frames: Vec<Value> = fs::read_to_string(directory.path().join("wire.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let response = frames
        .iter()
        .position(|frame| frame["result"]["outcome"]["outcome"] == "cancelled")
        .unwrap();
    let cancel = frames
        .iter()
        .position(|frame| frame["method"] == "session/cancel")
        .unwrap();
    assert!(response < cancel);
    session.stop().unwrap();
}
