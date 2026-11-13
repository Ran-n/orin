// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:43:35.350775
//+ Revised: 	2026/11/12 09:16:21.070607
// -----------------------------------------------------------------------------

//! Structured conversation state; the agent's terminal screen is never parsed.

use super::{
    controls::Controls,
    editor::{EditorAction, TextEditor, TextValue},
};
use crate::agents::{AgentSession, AgentSnapshot, AgentStatus};
use gpui::{Context, Entity, EventEmitter, ScrollHandle, prelude::*};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub enum AgentAction {
    OpenFile(PathBuf),
}

pub struct AgentView {
    pub session: Arc<Mutex<AgentSession>>,
    pub snapshot: AgentSnapshot,
    pub composer: Entity<TextEditor>,
    pub controls: Controls,
    pub scroll: ScrollHandle,
    pub error: Option<String>,
}

impl AgentView {
    pub fn new(
        session: Arc<Mutex<AgentSession>>,
        controls: Controls,
        cx: &mut Context<Self>,
    ) -> Self {
        let snapshot = session
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .snapshot();
        let input_controls = controls.clone();
        let composer = cx.new(|cx| TextEditor::prompt("agent-prompt", input_controls, cx));
        cx.observe(&composer, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&composer, |view, _, action, cx| {
            if matches!(action, EditorAction::Submit) {
                view.send_prompt(cx);
            }
        })
        .detach();
        Self {
            session,
            snapshot,
            composer,
            controls,
            scroll: ScrollHandle::new(),
            error: None,
        }
    }
    pub fn running(&self) -> bool {
        !matches!(
            self.snapshot.status,
            AgentStatus::Stopped | AgentStatus::Failed
        )
    }
    pub fn poll(&mut self, cx: &mut Context<Self>) {
        let Ok(session) = self.session.try_lock() else {
            return;
        };
        let snapshot = session.snapshot();
        drop(session);
        if snapshot != self.snapshot {
            let follow = self.scroll.offset().y <= -self.scroll.max_offset().height + gpui::px(24.);
            self.snapshot = snapshot;
            if follow {
                self.scroll.scroll_to_bottom();
            }
            cx.notify();
        }
    }
    pub fn send_prompt(&mut self, cx: &mut Context<Self>) {
        if self.snapshot.status != AgentStatus::Ready {
            return;
        }
        let text = self.composer.read(cx).buffer().text().to_owned();
        if text.trim().is_empty() {
            return;
        }
        let result = self
            .session
            .try_lock()
            .map_err(|_| {
                "The agent is stopping. Wait before sending another instruction.".to_owned()
            })
            .and_then(|session| {
                session.prompt(&text).map_err(|error| {
                    tracing::warn!(%error,"Agent instruction rejected");
                    "The instruction could not be sent. Check the agent's status and try again."
                        .into()
                })
            });
        match result {
            Ok(()) => {
                self.composer.update(cx, |editor, cx| {
                    editor.value =
                        TextValue::Prompt(crate::editor::EditorBuffer::new(String::new()));
                    cx.notify();
                });
                self.error = None;
            }
            Err(message) => self.error = Some(message),
        }
        cx.notify();
    }
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        let result = self
            .session
            .try_lock()
            .map_err(|_| "The agent is already stopping.".to_owned())
            .and_then(|session| {
                session.cancel().map_err(|error| {
                    tracing::warn!(%error,"Agent cancellation failed");
                    "The cancellation could not be sent. Close this session if it remains busy."
                        .into()
                })
            });
        if let Err(message) = result {
            self.error = Some(message);
        }
        cx.notify();
    }
    pub fn permission(&mut self, request: &str, option: Option<&str>, cx: &mut Context<Self>) {
        let result = self
            .session
            .try_lock()
            .map_err(|_| "The agent is stopping.".to_owned())
            .and_then(|session| {
                session
                    .respond_permission(request, option)
                    .map_err(|error| {
                        tracing::warn!(%error,"Permission response failed");
                        "That request could not be answered. It may have been cancelled.".into()
                    })
            });
        if let Err(message) = result {
            self.error = Some(message);
        }
        cx.notify();
    }
    pub fn authenticate(&mut self, method: &str, cx: &mut Context<Self>) {
        let result = self
            .session
            .try_lock()
            .map_err(|_| "The agent is stopping.".to_owned())
            .and_then(|session| {
                session.authenticate(method).map_err(|error| {
                    tracing::warn!(%error,"Agent authentication failed");
                    "Sign-in could not start. Use the agent's own sign-in tools and retry.".into()
                })
            });
        if let Err(message) = result {
            self.error = Some(message);
        }
        cx.notify();
    }
}
impl EventEmitter<AgentAction> for AgentView {}

pub fn status_label(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Starting => "Connecting",
        AgentStatus::Ready => "Ready",
        AgentStatus::Working => "Working",
        AgentStatus::WaitingPermission => "Needs your approval",
        AgentStatus::Cancelling => "Cancelling",
        AgentStatus::AuthRequired => "Sign in required",
        AgentStatus::Failed => "Stopped after an error",
        AgentStatus::Stopped => "Stopped",
    }
}

/// Status colour: green when it can take work, walnut while busy, warning when it needs you.
pub fn status_color(status: AgentStatus) -> u32 {
    match status {
        AgentStatus::Ready => super::theme::colors().accent,
        AgentStatus::Starting | AgentStatus::Working | AgentStatus::Cancelling => {
            super::theme::colors().walnut
        }
        AgentStatus::WaitingPermission | AgentStatus::AuthRequired => {
            super::theme::colors().warning
        }
        AgentStatus::Failed => super::theme::colors().error,
        AgentStatus::Stopped => super::theme::colors().faint,
    }
}
