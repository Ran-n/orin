// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:52:05.186137
//+ Revised: 	2026/11/12 13:59:40.081632
// -----------------------------------------------------------------------------

use super::*;
use crate::agents::{AuthMethod, options, permissions, updates};
impl Engine {
    pub(super) fn frame(&mut self, frame: Value) -> Result<()> {
        if let Some(method) = frame["method"].as_str() {
            return match method {
                "session/update" => updates::apply(&mut lock(&self.state), &frame["params"]),
                "session/request_permission" => self.permission(&frame),
                _ => {
                    if let Some(id) = frame.get("id") {
                        self.send(protocol::method_error(
                            id.clone(),
                            -32601,
                            "This client method is not supported.",
                        ))?;
                    }
                    Ok(())
                }
            };
        }
        let id = frame["id"]
            .as_u64()
            .context("Agent replied with an unknown request id.")?;
        let request = self
            .pending
            .remove(&id)
            .context("Agent replied to an unknown or completed request.")?;
        self.response(request, frame)
    }

    fn response(&mut self, request: Request, frame: Value) -> Result<()> {
        let result = match protocol::response_result(&frame) {
            Ok(result) => result,
            Err(error)
                if frame["error"]["code"].as_i64() == Some(-32000)
                    || matches!(request, Request::Authenticate) =>
            {
                let mut state = lock(&self.state);
                state.status = AgentStatus::AuthRequired;
                state.error = Some(error.to_string());
                self.deadline = None;
                return Ok(());
            }
            Err(error) => {
                match request {
                    // A refused option change leaves the session usable.
                    Request::SetConfig(..) => {
                        tracing::warn!(target: "orin::agents", "agent refused a session option change");
                        let message = error.to_string();
                        lock(&self.state).error = Some(message.clone());
                        self.option_error = Some(message);
                    }
                    // A refused message ends that turn only, so the user can try again.
                    Request::Prompt => {
                        tracing::warn!(target: "orin::agents", "agent refused a message");
                        for reply in permissions::cancel_all(&mut lock(&self.state))? {
                            self.send(reply)?;
                        }
                        let mut state = lock(&self.state);
                        state.error = Some(error.to_string());
                        state.status = AgentStatus::Ready;
                        self.deadline = None;
                    }
                    _ => return Err(error),
                }
                return Ok(());
            }
        };
        match request {
            Request::Initialize => self.initialized(result)?,
            Request::NewSession => {
                let id = result["sessionId"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .context("Agent did not return a session id.")?;
                let mut state = lock(&self.state);
                state.session_id = Some(id.into());
                state.config_options = options::from_session(result);
                state.status = AgentStatus::Ready;
                state.error = None;
                self.deadline = None;
            }
            Request::Prompt => {
                let stop = result["stopReason"]
                    .as_str()
                    .context("Agent did not provide a prompt stop reason.")?;
                let replies = permissions::cancel_all(&mut lock(&self.state))?;
                for reply in replies {
                    self.send(reply)?;
                }
                let mut state = lock(&self.state);
                state.stop_reason = Some(stop.into());
                state.status = AgentStatus::Ready;
                self.deadline = None;
            }
            Request::Authenticate => self.open_session()?,
            Request::SetConfig(id, value) => {
                let mut state = lock(&self.state);
                let updated = options::from_config_options(&result["configOptions"]);
                if updated.is_empty() {
                    if let Some(option) = state
                        .config_options
                        .iter_mut()
                        .find(|option| option.id == id)
                    {
                        option.current = value;
                    }
                } else {
                    updates::replace_options(&mut state, updated);
                }
                // A refused choice's error no longer applies once a choice is taken.
                if let Some(refused) = self.option_error.take()
                    && state.error.as_deref() == Some(refused.as_str())
                {
                    state.error = None;
                }
                tracing::info!(target: "orin::agents", option = %id, "session option changed");
            }
        }
        Ok(())
    }

    fn initialized(&mut self, result: &Value) -> Result<()> {
        ensure!(
            result["protocolVersion"] == 1,
            "This adapter does not support ACP v1. Install a compatible adapter."
        );
        let methods = result["authMethods"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|method| {
                Ok(AuthMethod {
                    id: method["id"]
                        .as_str()
                        .context("Authentication method has no id.")?
                        .into(),
                    name: method["name"]
                        .as_str()
                        .context("Authentication method has no name.")?
                        .into(),
                    description: method["description"].as_str().map(String::from),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        lock(&self.state).auth_methods = methods;
        self.open_session()
    }

    fn open_session(&mut self) -> Result<()> {
        let id = self.allocate(Request::NewSession);
        self.deadline = Some(Instant::now() + NEW_SESSION_TIMEOUT);
        self.send(protocol::new_session(&self.cwd, id))
    }

    fn permission(&mut self, frame: &Value) -> Result<()> {
        let id = frame
            .get("id")
            .context("Permission request has no request id.")?;
        // Checked and recorded under one lock, so a cancel can't slip in between.
        let received = {
            let mut state = lock(&self.state);
            if state.status == AgentStatus::Cancelling {
                None
            } else {
                Some(permissions::receive(&mut state, id, &frame["params"]))
            }
        };
        let Some(received) = received else {
            return self.send(protocol::permission_response(id.clone(), None));
        };
        match received {
            Ok(()) => Ok(()),
            Err(_) => {
                lock(&self.state).error = Some(
                    "The adapter supplied an invalid permission request; Orin cancelled it.".into(),
                );
                tracing::warn!("cancelled invalid ACP permission request");
                self.send(protocol::permission_response(id.clone(), None))
            }
        }
    }
}
