// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 10:31:31.357082
//+ Revised: 	2026/11/12 13:59:48.459543
// -----------------------------------------------------------------------------

//! Session options (model, effort, mode) offered over ACP, through a real adapter subprocess.

use super::support::{ready, spawn, wait};
use orin::agents::{AgentSnapshot, AgentStatus};
use serde_json::Value;
use std::fs;

fn current(state: &AgentSnapshot, id: &str) -> String {
    state
        .config_options
        .iter()
        .find(|option| option.id == id)
        .map(|option| option.current.clone())
        .unwrap_or_default()
}

fn wire(directory: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(directory.join("wire.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn integration_offered_options_are_read_and_changed_with_set_config_option() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("normal", directory.path());
    let state = ready(&session);
    // Select options in the shown categories are read; the boolean one is left out.
    let ids: Vec<&str> = state
        .config_options
        .iter()
        .map(|option| option.id.as_str())
        .collect();
    assert_eq!(ids, ["model", "effort", "mode"]);
    let model = &state.config_options[0];
    assert_eq!(model.current_name(), "Fixture Fast");
    assert_eq!(model.choices.len(), 3, "grouped choices are flattened");
    assert!(state.config_options.iter().all(|option| option.shown()));

    session.set_config_option("model", "fixture-deep").unwrap();
    let state = wait(&session, |state| current(state, "model") == "fixture-deep");
    assert_eq!(state.config_options[0].current_name(), "Fixture Deep");
    session.set_config_option("effort", "high").unwrap();
    wait(&session, |state| current(state, "effort") == "high");
    let frames = wire(directory.path());
    let set = frames
        .iter()
        .find(|frame| frame["method"] == "session/set_config_option")
        .unwrap();
    assert_eq!(set["params"]["sessionId"], "fixture-session");
    assert_eq!(set["params"]["configId"], "model");
    assert_eq!(set["params"]["value"], "fixture-deep");

    // Choices the agent doesn't offer are refused before anything is sent.
    assert!(session.set_config_option("model", "not-offered").is_err());
    assert!(session.set_config_option("no-such-option", "x").is_err());
    session.stop().unwrap();
}

#[test]
fn integration_a_refused_option_change_reports_the_error_and_keeps_the_session() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("normal", directory.path());
    ready(&session);
    session
        .set_config_option("model", "fixture-unavailable")
        .unwrap();
    let state = wait(&session, |state| state.error.is_some());
    assert!(state.error.as_deref().unwrap().contains("not available"));
    assert_eq!(state.status, AgentStatus::Ready);
    assert_eq!(current(&state, "model"), "fixture-fast");
    // Choosing an offered model clears the refusal's error.
    session.set_config_option("model", "fixture-deep").unwrap();
    let state = wait(&session, |state| current(state, "model") == "fixture-deep");
    assert_eq!(state.error, None);
    // The session still works afterwards.
    session.prompt("Hello").unwrap();
    wait(&session, |state| {
        state.status == AgentStatus::Ready && state.stop_reason.is_some()
    });
    session.stop().unwrap();
}

#[test]
fn integration_agents_may_change_their_own_options_mid_turn() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("normal", directory.path());
    ready(&session);
    session.prompt("plan first").unwrap();
    wait(&session, |state| current(state, "mode") == "plan");
    session.prompt("go deep").unwrap();
    let state = wait(&session, |state| current(state, "model") == "fixture-deep");
    assert_eq!(current(&state, "mode"), "plan");
    session.stop().unwrap();
}

#[test]
fn integration_older_adapters_list_models_and_change_them_with_set_model() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = spawn("legacy-models", directory.path());
    let state = ready(&session);
    assert_eq!(state.config_options.len(), 1);
    assert!(state.config_options[0].legacy_model);
    session.set_config_option("model", "fixture-deep").unwrap();
    wait(&session, |state| current(state, "model") == "fixture-deep");
    let frames = wire(directory.path());
    let set = frames
        .iter()
        .find(|frame| frame["method"] == "session/set_model")
        .unwrap();
    assert_eq!(set["params"]["modelId"], "fixture-deep");
    session.stop().unwrap();
}
