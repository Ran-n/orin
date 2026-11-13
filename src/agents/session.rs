// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:22.063603
//+ Revised: 	2026/11/12 10:31:35.259659
// -----------------------------------------------------------------------------

use std::{
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result, bail, ensure};

use super::{
    AgentSnapshot, AgentStatus, Provider,
    engine::{Control, Engine, EngineParts, Event},
    model::{SharedState, lock},
    transport,
};
use crate::process::{self, ProcessTree};

mod spawn;

const QUEUE_CAPACITY: usize = 64;
const MAX_PROMPT_BYTES: usize = 131_072;

/// One ACP agent process and all of its streams and children.
pub struct AgentSession {
    events: SyncSender<Event>,
    state: SharedState,
    shutdown: Arc<AtomicBool>,
    tree: Arc<ProcessTree>,
    workers: Vec<JoinHandle<()>>,
    process_id: u32,
}

impl AgentSession {
    pub fn prompt(&self, text: &str) -> Result<()> {
        ensure!(
            !text.trim().is_empty(),
            "Write a message before sending it."
        );
        ensure!(
            text.len() <= MAX_PROMPT_BYTES,
            "Message exceeds the 128 KiB limit."
        );
        let mut state = lock(&self.state);
        ensure!(
            state.status == AgentStatus::Ready,
            "The agent is not ready for a new message."
        );
        self.enqueue(Control::Prompt(text.into()))?;
        state.status = AgentStatus::Working;
        state.error = None;
        Ok(())
    }

    pub fn cancel(&self) -> Result<()> {
        let mut state = lock(&self.state);
        ensure!(state.is_busy(), "There is no active agent turn to cancel.");
        ensure!(
            state.status != AgentStatus::Cancelling,
            "Cancellation is already pending."
        );
        self.enqueue(Control::Cancel)?;
        state.status = AgentStatus::Cancelling;
        Ok(())
    }

    pub fn respond_permission(&self, request_id: &str, option_id: Option<&str>) -> Result<()> {
        let state = lock(&self.state);
        let permission = state
            .pending_permissions
            .iter()
            .find(|request| request.request_id == request_id)
            .context("This permission is no longer pending.")?;
        if let Some(option) = option_id {
            ensure!(
                permission
                    .options
                    .iter()
                    .any(|candidate| candidate.option_id == option),
                "The agent did not offer this permission option."
            );
        }
        self.enqueue(Control::Permission(
            request_id.into(),
            option_id.map(String::from),
        ))
    }

    /// Change a session option, such as the model, to one of the values the agent offers.
    pub fn set_config_option(&self, option_id: &str, value: &str) -> Result<()> {
        let state = lock(&self.state);
        ensure!(
            state.session_id.is_some(),
            "The agent session is not ready yet."
        );
        let option = state
            .config_options
            .iter()
            .find(|option| option.id == option_id)
            .context("The agent does not offer this option.")?;
        ensure!(
            option.offers(value),
            "The agent does not offer this choice."
        );
        self.enqueue(Control::SetConfig(option_id.into(), value.into()))
    }

    pub fn authenticate(&self, method_id: &str) -> Result<()> {
        let mut state = lock(&self.state);
        ensure!(
            state.status == AgentStatus::AuthRequired,
            "This agent is not waiting for authentication."
        );
        ensure!(
            state
                .auth_methods
                .iter()
                .any(|method| method.id == method_id),
            "The agent did not offer this authentication method."
        );
        self.enqueue(Control::Authenticate(method_id.into()))?;
        state.status = AgentStatus::Starting;
        Ok(())
    }

    fn enqueue(&self, control: Control) -> Result<()> {
        ensure!(
            !self.shutdown.load(Ordering::Acquire),
            "This agent session has stopped."
        );
        match self.events.try_send(Event::Control(control)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => bail!("Agent input is busy; retry after it catches up."),
            Err(TrySendError::Disconnected(_)) => bail!("The agent session has ended."),
        }
    }

    pub fn snapshot(&self) -> AgentSnapshot {
        lock(&self.state).clone()
    }
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    /// Join on a background thread. Closing the owning app must also call this.
    pub fn stop(&mut self) -> Result<()> {
        if self.workers.is_empty() {
            return Ok(());
        }
        self.shutdown.store(true, Ordering::Release);
        let termination = self.tree.terminate();
        let mut panicked = false;
        for worker in self.workers.drain(..) {
            let joined = crate::process::join_within(worker, crate::process::WORKER_JOIN_LIMIT);
            panicked |= joined == crate::process::Joined::Panicked;
        }
        let mut state = lock(&self.state);
        if state.status != AgentStatus::Failed {
            state.status = AgentStatus::Stopped;
        }
        state.pending_permissions.clear();
        ensure!(!panicked, "An agent worker panicked during shutdown.");
        termination?;
        Ok(())
    }
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            tracing::error!(%error, "agent shutdown failed");
        }
    }
}
