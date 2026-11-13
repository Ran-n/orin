// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:57:00.504602
//+ Revised: 	2026/11/12 15:03:20.577601
// -----------------------------------------------------------------------------

#![cfg(feature = "agent-fixture")]

#[path = "agents/support.rs"]
#[allow(dead_code)]
mod support;

use orin::agents::AgentStatus;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn integration_logs_operations_and_errors_without_prompt_stderr_or_error_payloads() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(bytes.clone());
    tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(move || writer.clone())
        .try_init()
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut session = support::spawn("prompt-error", directory.path());
    support::ready(&session);
    session
        .prompt("ACP_PROMPT_SECRET_MUST_NOT_BE_LOGGED")
        .unwrap();
    // A refused message ends that turn only: the session is ready for another try.
    let state = support::wait(&session, |state| {
        state.status == AgentStatus::Ready && state.error.is_some()
    });
    let message = state.error.unwrap();
    assert!(!message.contains("ACP_ERROR_SECRET_MUST_NOT_BE_LOGGED"));
    assert!(message.contains("try again"));
    session.stop().unwrap();
    let logs = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("ACP adapter launched") && logs.contains("agent refused a message"));
    for secret in ["ACP_PROMPT_SECRET", "ACP_STDERR_SECRET", "ACP_ERROR_SECRET"] {
        assert!(!logs.contains(secret));
    }
    for line in logs.lines() {
        serde_json::from_str::<serde_json::Value>(line).unwrap();
    }
}
