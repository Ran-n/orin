// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:52:05.262767
//+ Revised: 	2026/11/12 10:31:33.472209
// -----------------------------------------------------------------------------

use super::*;
use crate::agents::{MessageRole, options, permissions};
use serde_json::json;
impl Engine {
    pub(super) fn control(&mut self, control: Control) -> Result<()> {
        match control {
            Control::Prompt(text) => {
                ensure!(
                    !self.prompt_pending(),
                    "The agent is already handling a message."
                );
                let session = lock(&self.state)
                    .session_id
                    .clone()
                    .context("Agent session is not ready.")?;
                lock(&self.state).message(MessageRole::User, &text)?;
                let id = self.allocate(Request::Prompt);
                self.send(protocol::prompt(id, &session, &text))?;
            }
            Control::Cancel => {
                let replies = permissions::cancel_all(&mut lock(&self.state))?;
                for reply in replies {
                    self.send(reply)?;
                }
                // The turn already ended (its response raced the cancel): nothing to wait for.
                if !self.prompt_pending() {
                    let mut state = lock(&self.state);
                    if state.status == AgentStatus::Cancelling {
                        state.status = AgentStatus::Ready;
                    }
                    return Ok(());
                }
                let session = lock(&self.state)
                    .session_id
                    .clone()
                    .context("Agent session is not ready.")?;
                // `session/cancel` is a notification: only the prompt's response ends the wait.
                self.deadline = Some(Instant::now() + CANCEL_GRACE);
                self.send(protocol::notification(
                    "session/cancel",
                    json!({"sessionId":session}),
                ))?;
            }
            Control::Permission(id, option) => {
                let frame = permissions::answer(&mut lock(&self.state), &id, option.as_deref())?;
                self.send(frame)?;
            }
            Control::SetConfig(option_id, value) => {
                let (session, option) = {
                    let state = lock(&self.state);
                    let session = state
                        .session_id
                        .clone()
                        .context("Agent session is not ready.")?;
                    let option = state
                        .config_options
                        .iter()
                        .find(|option| option.id == option_id)
                        .cloned()
                        .context("The agent no longer offers this option.")?;
                    (session, option)
                };
                ensure!(
                    option.offers(&value),
                    "The agent does not offer this choice."
                );
                let id = self.allocate(Request::SetConfig(option_id, value.clone()));
                self.send(options::set_request(id, &session, &option, &value))?;
            }
            Control::Authenticate(method) => {
                let id = self.allocate(Request::Authenticate);
                self.deadline = Some(Instant::now() + AUTHENTICATE_TIMEOUT);
                self.send(protocol::request(
                    id,
                    "authenticate",
                    json!({"methodId":method}),
                ))?;
            }
        }
        Ok(())
    }

    /// Whether a `session/prompt` is waiting for its response.
    fn prompt_pending(&self) -> bool {
        self.pending
            .values()
            .any(|request| matches!(request, Request::Prompt))
    }
}
