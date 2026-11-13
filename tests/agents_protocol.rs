// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:23.881409
//+ Revised: 	2026/11/12 09:16:45.562254
// -----------------------------------------------------------------------------

use orin::agents::{AgentSnapshot, AgentStatus, Provider};
use proptest::prelude::*;

#[test]
fn contract_provider_names_require_structured_adapters() {
    assert_eq!(
        Provider::ALL.map(Provider::program),
        ["claude-agent-acp", "codex-acp", "gemini"]
    );
    // Saved settings store these keys as `selected_provider`; renaming one breaks restore.
    assert_eq!(
        Provider::ALL.map(|provider| serde_json::to_string(&provider).unwrap()),
        ["\"claude\"", "\"codex\"", "\"gemini\""]
    );
    assert_eq!(
        Provider::ALL.map(Provider::label),
        ["Claude Code", "Codex", "Gemini CLI"]
    );
    for provider in Provider::ALL {
        let serialized = serde_json::to_string(&provider).unwrap();
        assert_eq!(
            serde_json::from_str::<Provider>(&serialized).unwrap(),
            provider
        );
        let probe = provider.probe();
        assert_eq!(probe.available, probe.executable.is_some());
        assert_eq!(probe.available, probe.error.is_none());
    }
}

#[test]
fn unit_only_active_turn_statuses_are_busy() {
    for status in [
        AgentStatus::Starting,
        AgentStatus::Ready,
        AgentStatus::AuthRequired,
        AgentStatus::Failed,
        AgentStatus::Stopped,
    ] {
        let mut state = AgentSnapshot::new(Provider::Claude);
        state.status = status;
        assert!(!state.is_busy());
    }
    for status in [
        AgentStatus::Working,
        AgentStatus::WaitingPermission,
        AgentStatus::Cancelling,
    ] {
        let mut state = AgentSnapshot::new(Provider::Claude);
        state.status = status;
        assert!(state.is_busy());
    }
}

proptest! {
    #[test]
    fn property_snapshot_serde_roundtrip_preserves_message_text(text in ".{0,512}", provider_index in 0usize..3) {
        let mut state = AgentSnapshot::new(Provider::ALL[provider_index]);
        state.messages.push(orin::agents::AgentMessage { role: orin::agents::MessageRole::Agent, text });
        let encoded = serde_json::to_vec(&state).unwrap();
        prop_assert_eq!(serde_json::from_slice::<AgentSnapshot>(&encoded).unwrap(), state);
    }
}
