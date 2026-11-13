// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:43:35.532192
//+ Revised: 	2026/11/12 09:16:20.697816
// -----------------------------------------------------------------------------

//! Conversations, explicit approval choices, and per-agent activity.

use super::{
    agent_view::{AgentAction, AgentView, status_color, status_label},
    assets::Icon,
    controls::{ControlSpec, Emphasis, button, icon},
    theme,
};
use crate::agents::{AgentStatus, MessageRole};
use gpui::{AnyElement, Context, Window, div, prelude::*, rgb};

impl Render for AgentView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.snapshot.status;
        let can_send = status == AgentStatus::Ready
            && !self.composer.read(cx).buffer().text().trim().is_empty();
        theme::column()
            .size_full()
            .child(
                theme::row()
                    .h(theme::scaled(28.))
                    .flex_shrink_0()
                    .px_3()
                    .gap_2()
                    .text_size(theme::scaled(theme::SMALL_TEXT))
                    .child(
                        div()
                            .size(theme::scaled(7.))
                            .rounded_full()
                            .bg(rgb(status_color(status))),
                    )
                    .child(
                        div()
                            .text_color(rgb(theme::colors().muted))
                            .child(status_label(status)),
                    )
                    .child(div().flex_1())
                    .child(
                        ControlSpec::new("agent-cancel", &self.controls, cx)
                            .enabled(self.snapshot.is_busy())
                            .tooltip("Cancel this turn")
                            .icon_text(Icon::Stop, "Cancel", |view, _, cx| view.cancel(cx)),
                    ),
            )
            .child(self.conversation(cx))
            // Decisions the agent is waiting on stay pinned in view, never scrolled away.
            .when(!self.authentication(cx).is_empty(), |view| {
                view.child(
                    theme::row()
                        .flex_wrap()
                        .gap_2()
                        .px_3()
                        .pb_2()
                        .flex_shrink_0()
                        .children(self.authentication(cx)),
                )
            })
            .children(self.permission_panels(cx))
            .children(
                self.error
                    .clone()
                    .or_else(|| self.snapshot.error.clone())
                    .map(|message| {
                        theme::row()
                            .flex_shrink_0()
                            .mx_3()
                            .mb_2()
                            .gap_2()
                            .text_size(theme::scaled(theme::SMALL_TEXT))
                            .text_color(rgb(theme::colors().error))
                            .child(icon(Icon::Error, theme::colors().error))
                            .child(div().flex_1().whitespace_normal().child(message))
                    }),
            )
            .child(
                theme::column()
                    .flex_shrink_0()
                    .m_3()
                    .mt_0()
                    .rounded(theme::scaled(6.))
                    .border_1()
                    .border_color(rgb(theme::colors().border))
                    .bg(rgb(theme::colors().panel))
                    .child(div().h(theme::scaled(64.)).child(self.composer.clone()))
                    .child(
                        theme::row()
                            .px_2()
                            .pb_2()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(theme::scaled(theme::TINY_TEXT))
                                    .text_color(rgb(theme::colors().faint))
                                    .child("Enter to send · Shift+Enter for a new line"),
                            )
                            .child(
                                ControlSpec::new("agent-send", &self.controls, cx)
                                    .enabled(can_send)
                                    .emphasis(Emphasis::Primary)
                                    .tooltip("Send (Enter)")
                                    .icon(Icon::Send, |view, _, cx| view.send_prompt(cx)),
                            ),
                    ),
            )
    }
}
impl AgentView {
    fn conversation(&self, cx: &Context<Self>) -> AnyElement {
        let messages = self.snapshot.messages.iter().map(|message| {
            let (label, color) = match message.role {
                MessageRole::User => ("You", theme::colors().walnut),
                MessageRole::Agent => (self.snapshot.provider.label(), theme::colors().accent),
                MessageRole::Thought => ("Reasoning", theme::colors().muted),
            };
            theme::column()
                .flex_shrink_0()
                .gap_1()
                .py_2()
                .child(
                    div()
                        .text_size(theme::scaled(theme::TINY_TEXT))
                        .text_color(rgb(color))
                        .child(label),
                )
                .child(
                    div()
                        .whitespace_normal()
                        .text_color(rgb(if message.role == MessageRole::Thought {
                            theme::colors().muted
                        } else {
                            theme::colors().text
                        }))
                        .child(message.text.clone()),
                )
        });
        let tools = self
            .snapshot
            .tool_calls
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(|tool| {
                theme::row()
                    .flex_shrink_0()
                    .gap_2()
                    .py_1()
                    .text_xs()
                    .child(
                        div()
                            .text_color(rgb(theme::colors().muted))
                            .child(tool_label(&tool.status)),
                    )
                    .child(tool.title.clone())
            });
        theme::column().id("conversation-scroll").flex_1().px_3().overflow_y_scroll().track_scroll(&self.scroll)
            .children(messages).children(tools)
            .when(!self.snapshot.changed_files.is_empty(), |view| {
                view.child(theme::row().flex_wrap().flex_shrink_0().gap_2().py_1().children(self.changed_files(cx)))
            })
            .when(self.snapshot.messages.is_empty(),|view|view.child(div().py_2().text_size(theme::scaled(theme::SMALL_TEXT)).text_color(rgb(theme::colors().muted)).child("Give this agent a task. Its progress, questions, and tool activity will appear here.")))
            .into_any_element()
    }
    fn permission_panels(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        self.snapshot
            .pending_permissions
            .iter()
            .map(|request| {
                let mut choices = Vec::new();
                for option in &request.options {
                    let request_id = request.request_id.clone();
                    let option_id = option.option_id.clone();
                    choices.push(button(
                        format!("permission-{}-{}", request_id, option_id),
                        option.name.clone(),
                        true,
                        false,
                        &self.controls,
                        cx,
                        move |view, _, cx| view.permission(&request_id, Some(&option_id), cx),
                    ));
                }
                let request_id = request.request_id.clone();
                choices.push(button(
                    format!("permission-cancel-{request_id}"),
                    "Decline",
                    true,
                    false,
                    &self.controls,
                    cx,
                    move |view, _, cx| view.permission(&request_id, None, cx),
                ));
                theme::column()
                    .id(gpui::SharedString::from(format!(
                        "permission-{}",
                        request.request_id
                    )))
                    .flex_shrink_0()
                    .max_h(theme::scaled(180.))
                    .overflow_y_scroll()
                    .mx_3()
                    .mb_2()
                    .p_2()
                    .gap_2()
                    .border_1()
                    .border_color(rgb(theme::colors().warning))
                    .rounded(theme::scaled(6.))
                    .bg(rgb(theme::colors().panel))
                    .child(
                        theme::row()
                            .gap_2()
                            .child(icon(Icon::Error, theme::colors().warning))
                            .child(
                                div()
                                    .flex_1()
                                    .whitespace_normal()
                                    .child(request.title.clone()),
                            ),
                    )
                    .child(theme::row().flex_wrap().gap_2().children(choices))
                    .into_any_element()
            })
            .collect()
    }
    fn authentication(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        if self.snapshot.status != AgentStatus::AuthRequired {
            return vec![];
        }
        self.snapshot
            .auth_methods
            .iter()
            .map(|method| {
                let id = method.id.clone();
                button(
                    format!("authenticate-{id}"),
                    method.name.clone(),
                    true,
                    false,
                    &self.controls,
                    cx,
                    move |view, _, cx| view.authenticate(&id, cx),
                )
            })
            .collect()
    }
    fn changed_files(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        self.snapshot
            .changed_files
            .iter()
            .take(8)
            .map(|path| {
                let path = path.clone();
                button(
                    format!("changed-file-{}", path.display()),
                    format!(
                        "Review {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    true,
                    false,
                    &self.controls,
                    cx,
                    move |_, _, cx| cx.emit(AgentAction::OpenFile(path.clone())),
                )
            })
            .collect()
    }
}
fn tool_label(status: &str) -> &'static str {
    match status {
        "completed" => "Done",
        "failed" => "Failed",
        "in_progress" => "Running",
        "pending" => "Pending",
        _ => "Activity",
    }
}
