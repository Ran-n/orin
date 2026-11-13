// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:21.271627
//+ Revised: 	2026/11/12 10:31:34.637419
// -----------------------------------------------------------------------------

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use serde::{Deserialize, Serialize};

use super::{Provider, options::ConfigOption};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Starting,
    Ready,
    Working,
    WaitingPermission,
    Cancelling,
    AuthRequired,
    Failed,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Agent,
    Thought,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: MessageRole,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub content: Vec<String>,
    pub locations: Vec<PathBuf>,
    pub diff_paths: Vec<PathBuf>,
    pub raw_input: Option<serde_json::Value>,
    pub raw_output: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingPermission {
    pub request_id: String,
    pub title: String,
    pub tool_call_id: String,
    pub options: Vec<PermissionOption>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthMethod {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub provider: Provider,
    pub status: AgentStatus,
    pub session_id: Option<String>,
    pub messages: Vec<AgentMessage>,
    pub tool_calls: Vec<ToolCall>,
    pub pending_permissions: Vec<PendingPermission>,
    pub changed_files: Vec<PathBuf>,
    pub error: Option<String>,
    pub stop_reason: Option<String>,
    pub auth_methods: Vec<AuthMethod>,
    /// Session options the agent offers, such as its models.
    pub config_options: Vec<ConfigOption>,
}

pub(super) type SharedState = Arc<Mutex<AgentSnapshot>>;
pub(super) const MAX_MESSAGES: usize = 1_000;
pub(super) const MAX_TEXT_BYTES: usize = 2 * 1_024 * 1_024;
pub(super) const MAX_TOOLS: usize = 1_000;

impl AgentSnapshot {
    pub fn new(provider: Provider) -> Self {
        Self {
            provider,
            status: AgentStatus::Starting,
            session_id: None,
            messages: Vec::new(),
            tool_calls: Vec::new(),
            pending_permissions: Vec::new(),
            changed_files: Vec::new(),
            error: None,
            stop_reason: None,
            auth_methods: Vec::new(),
            config_options: Vec::new(),
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self.status,
            AgentStatus::Working | AgentStatus::WaitingPermission | AgentStatus::Cancelling
        )
    }

    pub(super) fn fail(&mut self, error: String) {
        self.status = AgentStatus::Failed;
        self.error = Some(error);
        self.pending_permissions.clear();
    }

    pub(super) fn message(&mut self, role: MessageRole, text: &str) -> anyhow::Result<()> {
        let size = self.text_bytes();
        anyhow::ensure!(
            size.saturating_add(text.len()) <= MAX_TEXT_BYTES,
            "Conversation reached its {} MiB text limit; start a new session.",
            MAX_TEXT_BYTES >> 20
        );
        if let Some(message) = self
            .messages
            .last_mut()
            .filter(|message| message.role == role && role != MessageRole::User)
        {
            message.text.push_str(text);
        } else {
            anyhow::ensure!(
                self.messages.len() < MAX_MESSAGES,
                "Conversation reached its message limit; start a new session."
            );
            self.messages.push(AgentMessage {
                role,
                text: text.into(),
            });
        }
        Ok(())
    }

    pub(super) fn text_bytes(&self) -> usize {
        self.messages
            .iter()
            .map(|message| message.text.len())
            .sum::<usize>()
            + self
                .tool_calls
                .iter()
                .map(|tool| {
                    tool.content.iter().map(String::len).sum::<usize>()
                        + tool.raw_input.as_ref().map_or(0, json_len)
                        + tool.raw_output.as_ref().map_or(0, json_len)
                })
                .sum::<usize>()
    }
}

/// The length of `value` as compact JSON, measured without building the string: this runs for
/// every streamed chunk, over every tool call in the conversation.
fn json_len(value: &serde_json::Value) -> usize {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    // Writing a `Value` to an infallible writer can't fail.
    if serde_json::to_writer(&mut counter, value).is_err() {
        return value.to_string().len();
    }
    counter.0
}

pub(super) fn lock(state: &SharedState) -> MutexGuard<'_, AgentSnapshot> {
    state.lock().unwrap_or_else(|poisoned| {
        tracing::error!("agent state lock was poisoned");
        let mut state = poisoned.into_inner();
        state.fail("Agent state was interrupted. Start a new session.".into());
        state
    })
}

#[cfg(test)]
mod size_tests {
    use super::json_len;

    #[test]
    fn unit_json_length_matches_the_serialised_text() {
        for value in [
            serde_json::json!(null),
            serde_json::json!({"path": "src/ñ.rs", "lines": [1, 2, 3], "nested": {"ok": true}}),
            serde_json::json!("quote \" and newline \n"),
        ] {
            assert_eq!(json_len(&value), value.to_string().len());
        }
    }
}
