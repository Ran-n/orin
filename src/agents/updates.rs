// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:21.803720
//+ Revised: 	2026/11/14 01:02:26.409859
// -----------------------------------------------------------------------------

use std::path::PathBuf;

use anyhow::{Result, ensure};
use serde_json::Value;

use super::{
    AgentSnapshot, MessageRole, ToolCall,
    model::{MAX_PLAN_ENTRIES, MAX_TOOLS, PlanEntry},
    options,
};

/// An update the agent sent in a shape Orin can't read. It says nothing about the session's
/// health, so it is skipped rather than ending the conversation.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct Malformed(&'static str);

/// Apply one `session/update`, skipping a malformed one with a warning. Only a limit the
/// conversation has reached is returned as an error, because the session can't go on past it.
pub(super) fn apply_or_skip(state: &mut AgentSnapshot, params: &Value) -> Result<()> {
    match apply(state, params) {
        Err(error) if error.is::<Malformed>() => {
            tracing::warn!(target: "orin::agents", reason = %error, "malformed agent update skipped");
            Ok(())
        }
        result => result,
    }
}

pub(super) fn apply(state: &mut AgentSnapshot, params: &Value) -> Result<()> {
    if params["sessionId"].as_str() != state.session_id.as_deref() {
        return Err(Malformed("Agent update belongs to an unknown session.").into());
    }
    let update = &params["update"];
    let kind = update["sessionUpdate"]
        .as_str()
        .ok_or(Malformed("Agent update has no type."))?;
    match kind {
        "agent_message_chunk" => {
            state.message(MessageRole::Agent, &content_text(&update["content"]))?
        }
        "agent_thought_chunk" => {
            state.message(MessageRole::Thought, &content_text(&update["content"]))?
        }
        "user_message_chunk" => {} // Orin already recorded the user's outgoing text.
        "tool_call" | "tool_call_update" => tool_update(state, update)?,
        "plan" => plan(state, update)?,
        "config_option_update" => replace_options(
            state,
            options::from_config_options(&update["configOptions"]),
        ),
        "current_mode_update" => {
            if let Some(mode) = update["currentModeId"].as_str() {
                for option in &mut state.config_options {
                    if option.category.as_deref() == Some("mode") && option.offers(mode) {
                        option.current = mode.to_owned();
                    }
                }
            }
        }
        _ => tracing::debug!("received an optional ACP update outside the supported baseline"),
    }
    Ok(())
}

/// Take the agent's new option list, keeping an older-style model picker it doesn't repeat.
pub(super) fn replace_options(state: &mut AgentSnapshot, mut updated: Vec<options::ConfigOption>) {
    let has_model = updated
        .iter()
        .any(|option| option.category.as_deref() == Some("model"));
    if !has_model
        && let Some(model) = state
            .config_options
            .iter()
            .find(|option| option.legacy_model)
            .cloned()
    {
        updated.insert(0, model);
    }
    state.config_options = updated;
}

pub(super) fn content_text(content: &Value) -> String {
    match content["type"].as_str() {
        Some("text") => content["text"].as_str().unwrap_or_default().into(),
        Some("resource_link") => format!(
            "[{}] {}",
            content["name"].as_str().unwrap_or("Resource"),
            content["uri"].as_str().unwrap_or_default()
        ),
        Some("resource") => content["resource"]["text"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| {
                format!(
                    "[Resource: {}]",
                    content["resource"]["uri"].as_str().unwrap_or("unknown")
                )
            }),
        Some(kind) => format!("[Unsupported {kind} content]"),
        None => "[Malformed agent content]".into(),
    }
}

pub(super) fn tool_update(state: &mut AgentSnapshot, update: &Value) -> Result<()> {
    let id = update["toolCallId"]
        .as_str()
        .ok_or(Malformed("Agent tool call has no id."))?;
    if !state.tool_calls.iter().any(|call| call.id == id) {
        ensure!(
            state.tool_calls.len() < MAX_TOOLS,
            "Session reached its tool call limit; start a new session."
        );
        state.tool_calls.push(ToolCall {
            id: id.into(),
            title: id.into(),
            kind: "other".into(),
            status: "pending".into(),
            after_message: state.messages.len(),
            ..ToolCall::default()
        });
    }
    let tool = state
        .tool_calls
        .iter_mut()
        .find(|call| call.id == id)
        .expect("inserted tool call");
    replace(&mut tool.title, &update["title"]);
    replace(&mut tool.kind, &update["kind"]);
    replace(&mut tool.status, &update["status"]);
    if let Some(value) = update.get("rawInput") {
        tool.raw_input = (!value.is_null()).then(|| value.clone());
    }
    if let Some(value) = update.get("rawOutput") {
        tool.raw_output = (!value.is_null()).then(|| value.clone());
    }
    tool.measure_raw();
    if let Some(locations) = update["locations"].as_array() {
        tool.locations = paths(locations);
    }
    if let Some(contents) = update["content"].as_array() {
        tool.content = contents.iter().map(tool_content).collect();
        ensure!(
            tool.content.iter().map(String::len).sum::<usize>() <= super::model::MAX_TEXT_BYTES,
            "Tool output exceeds the 2 MiB limit."
        );
        tool.diff_paths.clear();
        remember_diffs(&mut tool.diff_paths, contents);
    }
    if tool.status == "completed" && matches!(tool.kind.as_str(), "edit" | "delete" | "move") {
        for path in tool.locations.iter().chain(&tool.diff_paths) {
            if !state.changed_files.contains(path) {
                state.changed_files.push(path.clone());
            }
        }
    }
    ensure!(
        state.text_bytes() <= super::model::MAX_TEXT_BYTES,
        "Session output reached its 2 MiB limit; start a new session."
    );
    Ok(())
}

fn replace(target: &mut String, source: &Value) {
    if let Some(text) = source.as_str() {
        *target = text.into();
    }
}

/// The most files one tool call is recorded as touching. Each new path is compared with the
/// ones already kept, so an unbounded list would cost quadratic time under the state lock.
pub(super) const MAX_TOOL_PATHS: usize = 256;

fn paths(locations: &[Value]) -> Vec<PathBuf> {
    locations
        .iter()
        .filter_map(|location| location["path"].as_str().map(PathBuf::from))
        .take(MAX_TOOL_PATHS)
        .collect()
}

fn tool_content(content: &Value) -> String {
    match content["type"].as_str() {
        Some("content") => content_text(&content["content"]),
        Some("diff") => format!(
            "{}\n---\n{}\n+++\n{}",
            content["path"].as_str().unwrap_or_default(),
            content["oldText"].as_str().unwrap_or_default(),
            content["newText"].as_str().unwrap_or_default()
        ),
        Some("terminal") => format!(
            "[Agent terminal: {}]",
            content["terminalId"].as_str().unwrap_or("unknown")
        ),
        _ => "[Unsupported tool content]".into(),
    }
}

fn remember_diffs(changed_files: &mut Vec<PathBuf>, contents: &[Value]) {
    for content in contents.iter().filter(|content| content["type"] == "diff") {
        if changed_files.len() >= MAX_TOOL_PATHS {
            break;
        }
        if let Some(path) = content["path"]
            .as_str()
            .map(PathBuf::from)
            .filter(|path| !changed_files.contains(path))
        {
            changed_files.push(path);
        }
    }
}

fn plan(state: &mut AgentSnapshot, update: &Value) -> Result<()> {
    let entries = update["entries"]
        .as_array()
        .ok_or(Malformed("Agent plan has no entries."))?;
    ensure!(
        entries.len() <= MAX_PLAN_ENTRIES,
        "Agent plan has more than {MAX_PLAN_ENTRIES} steps."
    );
    // Each update carries the whole plan, so it replaces the last one instead of piling up.
    state.plan = entries
        .iter()
        .map(|entry| PlanEntry {
            content: entry["content"].as_str().unwrap_or_default().into(),
            status: entry["status"].as_str().unwrap_or("pending").into(),
        })
        .collect();
    ensure!(
        state.text_bytes() <= super::model::MAX_TEXT_BYTES,
        "Session output reached its 2 MiB limit; start a new session."
    );
    Ok(())
}
