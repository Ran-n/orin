// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:23.444418
//+ Revised: 	2026/11/12 09:16:17.110267
// -----------------------------------------------------------------------------

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::{AgentSnapshot, AgentStatus, PendingPermission, PermissionOption, protocol, updates};

/// Limits on what an adapter may ask at once, so a faulty one can't grow the UI without bound.
const MAX_PENDING_PERMISSIONS: usize = 32;
const MAX_PERMISSION_OPTIONS: usize = 64;

pub(super) fn receive(state: &mut AgentSnapshot, id: &Value, params: &Value) -> Result<()> {
    ensure!(
        params["sessionId"].as_str() == state.session_id.as_deref(),
        "Permission request belongs to an unknown session."
    );
    ensure!(
        state.is_busy(),
        "Agent requested permission outside an active turn."
    );
    ensure!(
        state.pending_permissions.len() < MAX_PENDING_PERMISSIONS,
        "Agent has too many pending permission requests."
    );
    let request_id = serde_json::to_string(id)?;
    ensure!(
        !state
            .pending_permissions
            .iter()
            .any(|request| request.request_id == request_id),
        "Agent reused a pending permission request id."
    );
    let tool = &params["toolCall"];
    updates::tool_update(state, tool)?;
    let options = params["options"]
        .as_array()
        .context("Agent supplied no permission options.")?
        .iter()
        .map(option)
        .collect::<Result<Vec<_>>>()?;
    ensure!(!options.is_empty(), "Agent supplied no permission options.");
    ensure!(
        options.len() <= MAX_PERMISSION_OPTIONS,
        "Agent supplied too many permission options."
    );
    let unique: std::collections::HashSet<_> =
        options.iter().map(|option| &option.option_id).collect();
    ensure!(
        unique.len() == options.len(),
        "Agent supplied duplicate permission option ids."
    );
    state.pending_permissions.push(PendingPermission {
        request_id,
        title: tool["title"]
            .as_str()
            .unwrap_or("Agent operation requires permission")
            .into(),
        tool_call_id: tool["toolCallId"]
            .as_str()
            .context("Permission request has no tool call id.")?
            .into(),
        options,
    });
    state.status = AgentStatus::WaitingPermission;
    Ok(())
}

fn option(value: &Value) -> Result<PermissionOption> {
    let kind = value["kind"]
        .as_str()
        .context("Permission option has no kind.")?;
    ensure!(
        matches!(
            kind,
            "allow_once" | "allow_always" | "reject_once" | "reject_always"
        ),
        "Agent supplied an unsupported permission option kind."
    );
    let option = PermissionOption {
        option_id: value["optionId"]
            .as_str()
            .context("Permission option has no id.")?
            .into(),
        name: value["name"]
            .as_str()
            .context("Permission option has no name.")?
            .into(),
        kind: kind.into(),
    };
    ensure!(
        !option.option_id.is_empty(),
        "Agent supplied an empty permission option id."
    );
    Ok(option)
}

pub(super) fn answer(
    state: &mut AgentSnapshot,
    request_id: &str,
    selected: Option<&str>,
) -> Result<Value> {
    let index = state
        .pending_permissions
        .iter()
        .position(|request| request.request_id == request_id)
        .context("This permission request is no longer pending.")?;
    let request = &state.pending_permissions[index];
    if let Some(option) = selected {
        ensure!(
            request
                .options
                .iter()
                .any(|candidate| candidate.option_id == option),
            "This permission option was not offered by the agent."
        );
    }
    let id = serde_json::from_str(&request.request_id)?;
    state.pending_permissions.remove(index);
    if state.pending_permissions.is_empty() && state.status == AgentStatus::WaitingPermission {
        state.status = AgentStatus::Working;
    }
    Ok(protocol::permission_response(id, selected))
}

pub(super) fn cancel_all(state: &mut AgentSnapshot) -> Result<Vec<Value>> {
    let ids: Vec<_> = state
        .pending_permissions
        .iter()
        .map(|request| request.request_id.clone())
        .collect();
    ids.iter().map(|id| answer(state, id, None)).collect()
}
