// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:50:00.412454
//+ Revised: 	2026/11/13 16:19:59.736479
// -----------------------------------------------------------------------------

use super::{AgentSnapshot, AgentStatus, Provider, permissions, protocol, transport, updates};
use proptest::prelude::*;
use serde_json::{Value, json};
use std::io::Cursor;

fn working() -> AgentSnapshot {
    let mut state = AgentSnapshot::new(Provider::Claude);
    state.status = AgentStatus::Working;
    state.session_id = Some("session".into());
    state
}

fn permission(option: &str) -> Value {
    json!({"sessionId":"session","toolCall":{"toolCallId":"edit","title":"Edit the entrypoint"},
        "options":[{"optionId":option,"name":"Allow once","kind":"allow_once"}]})
}

#[test]
fn contract_initialize_matches_checked_in_v1_golden() {
    let mut golden: Value =
        serde_json::from_str(include_str!("../../tests/agents/golden/initialize-v1.json")).unwrap();
    // The wire contract is stable; the advertised client version follows each release.
    golden["params"]["clientInfo"]["version"] = json!(env!("CARGO_PKG_VERSION"));
    assert_eq!(protocol::initialize(), golden);
    assert_eq!(
        protocol::new_session("/project", 2)["params"],
        json!({"cwd":"/project","mcpServers":[]})
    );
}

#[test]
fn regression_agent_errors_do_not_expose_payloads_or_protocol_codes() {
    // Adapter failures may contain secrets, paths, or stack traces. They used to reach the UI verbatim.
    for code in [-32603, -32602, 7] {
        let frame = json!({"error":{"code":code,"message":"PRIVATE_ADAPTER_PAYLOAD"}});
        let message = protocol::response_result(&frame).unwrap_err().to_string();
        assert!(!message.contains("PRIVATE_ADAPTER_PAYLOAD"));
        assert!(!message.contains(&code.to_string()));
        assert!(message.contains("try again"));
    }
}

#[test]
fn unit_jsonrpc_decoder_rejects_bad_versions_ids_and_response_shapes() {
    for frame in [
        json!({}),
        json!([]),
        json!({"jsonrpc":"1.0","id":1,"result":{}}),
        json!({"jsonrpc":"2.0","id":null,"result":{}}),
        json!({"jsonrpc":"2.0","id":1}),
        json!({"jsonrpc":"2.0","id":1,"result":{},"error":{}}),
    ] {
        assert!(protocol::decode(&serde_json::to_vec(&frame).unwrap()).is_err());
    }
    assert!(protocol::decode(&vec![b'x'; protocol::MAX_FRAME_BYTES + 1]).is_err());
}

#[test]
fn unit_frame_reader_requires_newline_and_handles_multiple_messages() {
    let mut reader = Cursor::new(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n{\"jsonrpc\":\"2.0\",\"method\":\"notice\"}\n");
    assert_eq!(
        transport::read_frame(&mut reader).unwrap().unwrap()["id"],
        1
    );
    assert_eq!(
        transport::read_frame(&mut reader).unwrap().unwrap()["method"],
        "notice"
    );
    assert!(transport::read_frame(&mut reader).unwrap().is_none());
    assert!(
        transport::read_frame(&mut Cursor::new(
            b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}"
        ))
        .is_err()
    );
}

#[test]
fn unit_permissions_reject_unknown_sessions_options_ids_and_kinds() {
    let mut state = working();
    let mut request = permission("allow");
    request["sessionId"] = json!("other");
    assert!(permissions::receive(&mut state, &json!(3), &request).is_err());
    let mut request = permission("allow");
    request["options"][0]["kind"] = json!("allow_everything");
    assert!(permissions::receive(&mut state, &json!(3), &request).is_err());
    permissions::receive(&mut state, &json!(3), &permission("allow")).unwrap();
    assert!(permissions::answer(&mut state, "3", Some("not-offered")).is_err());
    assert!(permissions::answer(&mut state, "999", Some("allow")).is_err());
    assert_eq!(state.pending_permissions.len(), 1);
    assert_eq!(
        permissions::cancel_all(&mut state).unwrap()[0]["result"]["outcome"],
        json!({"outcome":"cancelled"})
    );
    assert!(state.pending_permissions.is_empty());
}

#[test]
fn regression_completed_tool_update_retains_diff_paths_from_pending_update() {
    // Completion updates are partial; losing the earlier diff hid the changed file.
    let mut state = working();
    updates::tool_update(
        &mut state,
        &json!({"toolCallId":"edit","kind":"edit","status":"pending", "content":[
        {"type":"diff","path":"/project/main.rs","oldText":"old","newText":"new"}]}),
    )
    .unwrap();
    assert!(state.changed_files.is_empty());
    updates::tool_update(
        &mut state,
        &json!({"toolCallId":"edit","status":"completed"}),
    )
    .unwrap();
    assert_eq!(
        state.changed_files,
        vec![std::path::PathBuf::from("/project/main.rs")]
    );
    assert!(state.tool_calls[0].content[0].contains("new"));
}

#[test]
fn unit_updates_handle_chunk_types_plans_and_reject_other_sessions() {
    let mut state = working();
    updates::apply(&mut state, &json!({"sessionId":"session","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"resource_link","name":"Readme","uri":"file:///README.md"}}})).unwrap();
    assert!(state.messages[0].text.contains("Readme"));
    updates::apply(&mut state, &json!({"sessionId":"session","update":{"sessionUpdate":"plan","entries":[{"status":"pending","content":"Run tests"}]}})).unwrap();
    assert!(state.messages[1].text.contains("Run tests"));
    assert!(
        updates::apply(
            &mut state,
            &json!({"sessionId":"other","update":{"sessionUpdate":"agent_message_chunk"}})
        )
        .is_err()
    );
}

#[test]
fn unit_duplicate_permission_option_ids_cannot_confuse_allow_and_reject() {
    let mut state = working();
    let mut request = permission("same-id");
    request["options"]
        .as_array_mut()
        .unwrap()
        .push(json!({"optionId":"same-id","kind":"reject_once","name":"Reject"}));
    assert!(permissions::receive(&mut state, &json!(1), &request).is_err());
    assert!(state.pending_permissions.is_empty());
}

#[test]
fn unit_conversation_limit_is_checked_before_appending_more_text() {
    let mut state = working();
    let text = "x".repeat(super::model::MAX_TEXT_BYTES);
    state.message(super::MessageRole::Agent, &text).unwrap();
    assert!(
        state
            .message(super::MessageRole::Agent, "one more byte")
            .is_err()
    );
    assert_eq!(state.messages[0].text.len(), super::model::MAX_TEXT_BYTES);
}

proptest! {
    #[test]
    fn property_prompt_roundtrip_has_one_wire_line_and_preserves_text(text in ".{0,512}") {
        let frame = protocol::prompt(3, "session", &text);
        let encoded = serde_json::to_vec(&frame).unwrap();
        prop_assert!(!encoded.contains(&b'\n'));
        let decoded = protocol::decode(&encoded).unwrap();
        prop_assert_eq!(decoded["params"]["prompt"][0]["text"].as_str(), Some(text.as_str()));
    }

    #[test]
    fn property_permission_response_selects_only_the_offered_option(option in ".{1,64}") {
        let mut state = working();
        permissions::receive(&mut state, &json!("request-1"), &permission(&option)).unwrap();
        let response = permissions::answer(&mut state, "\"request-1\"", Some(&option)).unwrap();
        prop_assert_eq!(response["id"].as_str(), Some("request-1"));
        prop_assert_eq!(response["result"]["outcome"]["optionId"].as_str(), Some(option.as_str()));
        prop_assert!(state.pending_permissions.is_empty());
    }
}
