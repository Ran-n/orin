// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:23.247522
//+ Revised: 	2026/11/12 15:28:14.505038
// -----------------------------------------------------------------------------

use std::{
    collections::BTreeMap,
    process::Child,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
mod controls;
mod inbound;

use super::{
    AgentStatus,
    model::{SharedState, lock},
    protocol,
};
use crate::process::ProcessTree;

/// How long an adapter may take to wind down a cancelled turn before the session is failed.
/// Adapters stop their tools first, so this is generous; a hung adapter is still caught.
const CANCEL_GRACE: Duration = Duration::from_secs(15);
/// An agent's own sign-in can wait for the user in a browser.
const AUTHENTICATE_TIMEOUT: Duration = Duration::from_secs(120);
/// Opening a session after initialize or sign-in.
const NEW_SESSION_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) enum Control {
    Prompt(String),
    Cancel,
    Permission(String, Option<String>),
    Authenticate(String),
    /// Set a session option (by id) to one of its offered values.
    SetConfig(String, String),
}
pub(super) enum Event {
    Frame(Value),
    Control(Control),
    TransportError(String),
    Eof,
}
#[derive(Clone)]
enum Request {
    Initialize,
    NewSession,
    Prompt,
    Authenticate,
    SetConfig(String, String),
}

pub(super) struct Engine {
    child: Child,
    tree: Arc<ProcessTree>,
    state: SharedState,
    shutdown: Arc<AtomicBool>,
    outbound: SyncSender<Value>,
    events: Receiver<Event>,
    cwd: String,
    pending: BTreeMap<u64, Request>,
    next_id: u64,
    deadline: Option<Instant>,
    reaped: bool,
    /// The error shown for a refused option change, cleared when a later change succeeds.
    option_error: Option<String>,
}

/// What an engine is built from: the adapter process and the channels around it.
pub(in crate::agents) struct EngineParts {
    pub child: Child,
    pub tree: Arc<ProcessTree>,
    pub state: SharedState,
    pub shutdown: Arc<AtomicBool>,
    pub outbound: SyncSender<Value>,
    pub events: Receiver<Event>,
    pub cwd: String,
    /// How long the adapter may take to answer `initialize` (longer when npx downloads it).
    pub startup: Duration,
}

impl Engine {
    pub(super) fn new(parts: EngineParts) -> Self {
        let EngineParts {
            child,
            tree,
            state,
            shutdown,
            outbound,
            events,
            cwd,
            startup,
        } = parts;
        Self {
            child,
            tree,
            state,
            shutdown,
            outbound,
            events,
            cwd,
            pending: BTreeMap::new(),
            next_id: 2,
            deadline: Some(Instant::now() + startup),
            reaped: false,
            option_error: None,
        }
    }

    pub(super) fn run(mut self) {
        self.pending.insert(1, Request::Initialize);
        if let Err(error) = self
            .send(protocol::initialize())
            .and_then(|()| self.event_loop())
        {
            // Error text is UI data and may contain agent/user text; never log it.
            tracing::error!("ACP session failed");
            lock(&self.state).fail(error.to_string());
        }
        self.cleanup();
    }

    fn event_loop(&mut self) -> Result<()> {
        while !self.shutdown.load(Ordering::Acquire) {
            ensure!(
                self.deadline
                    .is_none_or(|deadline| Instant::now() < deadline),
                "The ACP adapter did not respond in time. Restart the session after checking the CLI."
            );
            match self.events.recv_timeout(Duration::from_millis(20)) {
                Ok(Event::Frame(frame)) => self.frame(frame)?,
                // A refused answer or option change is reported; the session stays usable.
                Ok(Event::Control(
                    control @ (Control::Permission(_, _) | Control::SetConfig(_, _)),
                )) => {
                    if let Err(error) = self.control(control) {
                        tracing::warn!(target: "orin::agents", "agent control refused");
                        lock(&self.state).error = Some(error.to_string());
                    }
                }
                Ok(Event::Control(control)) => self.control(control)?,
                Ok(Event::TransportError(error)) => bail!(error),
                Ok(Event::Eof) => bail!(
                    "The ACP adapter closed its output. Check its CLI installation and sign-in, then start a new session."
                ),
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => self.check_child()?,
            }
        }
        Ok(())
    }

    fn check_child(&mut self) -> Result<()> {
        if let Some(status) = self.child.try_wait()? {
            self.reaped = true;
            bail!(
                "The ACP adapter exited ({}). Check its CLI installation and sign-in, then start a new session.",
                status
                    .code()
                    .map_or("signal".into(), |code| code.to_string())
            );
        }
        Ok(())
    }

    fn allocate(&mut self, request: Request) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, request);
        id
    }
    fn send(&self, frame: Value) -> Result<()> {
        self.outbound
            .try_send(frame)
            .map_err(|_| anyhow::anyhow!("The ACP adapter is not accepting messages."))
    }

    fn cleanup(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Err(error) = self.tree.terminate() {
            tracing::error!(%error, "failed to terminate agent descendants");
        }
        if !self.reaped {
            if let Err(error) = self.child.kill() {
                tracing::debug!(%error, "agent child may already have exited");
            }
            if let Err(error) = self.child.wait() {
                tracing::error!(%error, "failed to reap agent process");
            }
            self.reaped = true;
        }
        let mut state = lock(&self.state);
        if state.status != AgentStatus::Failed {
            state.status = AgentStatus::Stopped;
        }
        state.pending_permissions.clear();
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if !self.reaped {
            self.cleanup();
        }
    }
}
