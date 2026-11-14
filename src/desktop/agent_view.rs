// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:43:35.350775
//+ Revised: 	2026/11/14 01:02:27.885621
// -----------------------------------------------------------------------------

//! Structured conversation state; the agent's terminal screen is never parsed.

use super::{
    controls::Controls,
    editor::{EditorAction, TextEditor, TextValue},
    markdown::{self, Block},
};
use crate::agents::{AgentMessage, AgentSession, AgentSnapshot, AgentStatus, MessageRole};
use gpui::{Context, Entity, EventEmitter, ScrollHandle, prelude::*};
use std::{
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub enum AgentAction {
    /// A file the agent reported changing, by the path the agent gave.
    OpenFile(PathBuf),
    /// A project file linked from a reply, relative to the project root.
    OpenProjectFile(PathBuf),
}

pub struct AgentView {
    pub session: Arc<Mutex<AgentSession>>,
    /// The project folder the agent works in; paths inside it are shown relative to it.
    pub root: PathBuf,
    pub snapshot: AgentSnapshot,
    pub composer: Entity<TextEditor>,
    pub controls: Controls,
    pub scroll: ScrollHandle,
    pub error: Option<String>,
    /// Each message's reply parsed as Markdown, in step with `snapshot.messages`; empty for the
    /// user's own messages and reasoning, which are shown as written.
    pub replies: Vec<Rc<[Block]>>,
}

impl AgentView {
    pub fn new(
        session: Arc<Mutex<AgentSession>>,
        root: PathBuf,
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
        let replies = parsed_replies(&[], Vec::new(), &snapshot.messages);
        Self {
            session,
            root,
            snapshot,
            composer,
            controls,
            scroll: ScrollHandle::new(),
            error: None,
            replies,
        }
    }
    pub fn poll(&mut self, cx: &mut Context<Self>) {
        let Ok(session) = self.session.try_lock() else {
            return;
        };
        let changed = session.changed_snapshot(&self.snapshot);
        drop(session);
        if let Some(snapshot) = changed {
            let follow = self.scroll.offset().y <= -self.scroll.max_offset().height + gpui::px(24.);
            self.replies = parsed_replies(
                &self.snapshot.messages,
                std::mem::take(&mut self.replies),
                &snapshot.messages,
            );
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

/// The parsed replies for `messages`, reusing `parsed` (the replies of `previous`) wherever a
/// message is unchanged: while a reply streams in, only the message being written is parsed
/// again, not the whole conversation.
pub fn parsed_replies(
    previous: &[AgentMessage],
    mut parsed: Vec<Rc<[Block]>>,
    messages: &[AgentMessage],
) -> Vec<Rc<[Block]>> {
    parsed.truncate(messages.len());
    for (index, message) in messages.iter().enumerate() {
        if index < parsed.len() && previous.get(index) == Some(message) {
            continue;
        }
        let blocks: Rc<[Block]> = if message.role == MessageRole::Agent {
            markdown::parse(&message.text).into()
        } else {
            Rc::from([])
        };
        if index < parsed.len() {
            parsed[index] = blocks;
        } else {
            parsed.push(blocks);
        }
    }
    parsed
}

pub fn status_label(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Starting => "Connecting…",
        AgentStatus::Ready => "Ready",
        AgentStatus::Working => "Working…",
        AgentStatus::WaitingPermission => "Needs your approval",
        AgentStatus::Cancelling => "Cancelling…",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: MessageRole, text: &str) -> AgentMessage {
        AgentMessage {
            role,
            text: text.into(),
        }
    }

    #[test]
    fn unit_only_changed_replies_are_parsed_again() {
        let first = vec![
            message(MessageRole::User, "**not parsed**"),
            message(MessageRole::Agent, "# Title"),
            message(MessageRole::Agent, "streaming"),
        ];
        let parsed = parsed_replies(&[], Vec::new(), &first);
        assert_eq!(parsed.len(), 3);
        assert!(parsed[0].is_empty(), "the user's text is shown as written");
        assert!(matches!(parsed[1][0], Block::Heading(1, _)));

        let mut second = first.clone();
        second[2].text.push_str(" more\n\n- item");
        second.push(message(MessageRole::Thought, "thinking"));
        let kept = parsed[1].clone();
        let updated = parsed_replies(&first, parsed, &second);
        assert_eq!(updated.len(), 4);
        assert!(
            Rc::ptr_eq(&kept, &updated[1]),
            "an unchanged reply keeps its blocks"
        );
        assert_eq!(updated[2].len(), 2, "the streamed reply is parsed again");
        assert!(updated[3].is_empty(), "reasoning is shown as written");

        // A shorter conversation (a new session's first snapshot) drops the stale replies.
        let shorter = parsed_replies(&second, updated, &second[..1]);
        assert_eq!(shorter.len(), 1);
    }

    #[test]
    fn unit_every_status_has_a_label_and_a_colour() {
        for status in [
            AgentStatus::Starting,
            AgentStatus::Ready,
            AgentStatus::Working,
            AgentStatus::WaitingPermission,
            AgentStatus::Cancelling,
            AgentStatus::AuthRequired,
            AgentStatus::Failed,
            AgentStatus::Stopped,
        ] {
            assert!(!status_label(status).is_empty());
            assert_ne!(status_color(status), 0);
        }
        assert_ne!(
            status_color(AgentStatus::Ready),
            status_color(AgentStatus::WaitingPermission),
            "an agent waiting on you never looks idle"
        );
    }
}
