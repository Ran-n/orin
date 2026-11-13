// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:21.490311
//+ Revised: 	2026/11/12 15:03:48.659246
// -----------------------------------------------------------------------------

use anyhow::{Result, bail, ensure};
use serde_json::{Value, json};

pub(super) const MAX_FRAME_BYTES: usize = 4 * 1_024 * 1_024;

pub(super) fn initialize() -> Value {
    request(
        1,
        "initialize",
        json!({"protocolVersion":1,"clientCapabilities":{},
        "clientInfo":{"name":"orin","title":"Orin","version":env!("CARGO_PKG_VERSION")}}),
    )
}

pub(super) fn new_session(cwd: &str, id: u64) -> Value {
    request(id, "session/new", json!({"cwd":cwd,"mcpServers":[]}))
}

pub(super) fn prompt(id: u64, session: &str, text: &str) -> Value {
    request(
        id,
        "session/prompt",
        json!({"sessionId":session,"prompt":[{"type":"text","text":text}]}),
    )
}

pub(super) fn request(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}

pub(super) fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","method":method,"params":params})
}

pub(super) fn permission_response(id: Value, option: Option<&str>) -> Value {
    let outcome = option.map_or_else(
        || json!({"outcome":"cancelled"}),
        |option| json!({"outcome":"selected","optionId":option}),
    );
    json!({"jsonrpc":"2.0","id":id,"result":{"outcome":outcome}})
}

pub(super) fn method_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

pub(super) fn decode(bytes: &[u8]) -> Result<Value> {
    ensure!(
        bytes.len() <= MAX_FRAME_BYTES,
        "ACP message exceeds the 4 MiB limit."
    );
    let frame: Value = serde_json::from_slice(bytes)?;
    ensure!(
        frame.is_object() && frame["jsonrpc"] == "2.0",
        "Agent sent an invalid JSON-RPC 2.0 message."
    );
    if let Some(id) = frame.get("id") {
        ensure!(
            id.is_string() || id.is_u64() || id.is_i64(),
            "Agent sent an invalid request id."
        );
    }
    if frame.get("method").is_none() {
        ensure!(
            frame.get("id").is_some()
                && (frame.get("result").is_some() ^ frame.get("error").is_some()),
            "Agent sent an invalid JSON-RPC response."
        );
    } else {
        ensure!(
            frame["method"].is_string(),
            "Agent sent an invalid method name."
        );
    }
    Ok(frame)
}

pub(super) fn response_result(frame: &Value) -> Result<&Value> {
    if let Some(error) = frame.get("error") {
        if error["code"].as_i64() == Some(-32000) {
            bail!(
                "Authentication required. Sign in using the agent's own CLI, then restart this session."
            );
        }
        // Error payloads are untrusted and may include credentials or implementation details.
        bail!(
            "This request is not available. Check the agent's sign-in and selected options, then try again."
        );
    }
    frame
        .get("result")
        .ok_or_else(|| anyhow::anyhow!("Agent response did not contain a result."))
}
