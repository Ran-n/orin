// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:18:28.471221
//+ Revised: 	2026/11/14 01:02:27.327260
// -----------------------------------------------------------------------------

//! One agent's conversation: what was said and done in the order it happened, the decisions
//! the agent is waiting on, and the message box.

use super::{
    agent_details::tool_row,
    agent_view::{AgentAction, AgentView, status_color, status_label},
    assets::Icon,
    controls::{ControlSpec, Emphasis, agent_color, agent_icon, icon},
    markdown::{self, LinkTarget},
    markdown_view::{LinkOpener, message_blocks},
    theme,
};
use crate::agents::{AgentStatus, MessageRole};
use gpui::{AnyElement, Context, Window, div, prelude::*, rgb};
use std::path::Path;

/// The message box grows with its text between these heights, then scrolls.
const COMPOSER_MIN_HEIGHT: f32 = 52.0;
const COMPOSER_MAX_LINES: f32 = 10.0;

impl Render for AgentView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        theme::column()
            .size_full()
            .child(self.conversation(cx))
            .children(self.plan_panel())
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
                            .items_start()
                            .mx_3()
                            .mb_2()
                            .px_2()
                            .py_1()
                            .gap_2()
                            .rounded(theme::scaled(theme::RADIUS))
                            .bg(rgb(theme::colors().error_surface))
                            .text_size(theme::scaled(theme::SMALL_TEXT))
                            .text_color(rgb(theme::colors().error))
                            .child(
                                div()
                                    .pt(theme::scaled(2.))
                                    .child(icon(Icon::Error, theme::colors().error)),
                            )
                            .child(div().flex_1().min_w_0().whitespace_normal().child(message))
                    }),
            )
            .child(self.composer_box(cx))
    }
}

impl AgentView {
    /// The message box: the draft, then what the agent is doing beside the one action that fits
    /// it. Send while the agent can take an instruction, Stop while it is working.
    fn composer_box(&self, cx: &Context<Self>) -> AnyElement {
        let status = self.snapshot.status;
        let draft = self.composer.read(cx).buffer().text();
        let can_send = status == AgentStatus::Ready && !draft.trim().is_empty();
        // Drawn rows, so a long line that wraps grows the box like a typed line break does.
        let lines = (self.composer.read(cx).visual_rows.max(1) as f32).min(COMPOSER_MAX_LINES);
        let height = (lines * theme::CODE_LINE + 2. * theme::CODE_PADDING).max(COMPOSER_MIN_HEIGHT);
        let footer = if status == AgentStatus::Ready {
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(theme::scaled(theme::TINY_TEXT))
                .text_color(rgb(theme::colors().faint))
                .child("Enter to send · Shift+Enter for a new line")
        } else {
            theme::row()
                .flex_1()
                .gap_2()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(theme::scaled(theme::SMALL_TEXT))
                .text_color(rgb(theme::colors().soft_text))
                .child(
                    div()
                        .size(theme::scaled(7.))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(rgb(status_color(status))),
                )
                .child(status_label(status))
        };
        let action = if self.snapshot.is_busy() {
            ControlSpec::new("agent-cancel", &self.controls, cx)
                .tooltip("Stop this turn")
                .icon_text(Icon::Stop, "Stop", |view, _, cx| view.cancel(cx))
        } else {
            ControlSpec::new("agent-send", &self.controls, cx)
                .enabled(can_send)
                .emphasis(Emphasis::Primary)
                .tooltip("Send (Enter)")
                .icon(Icon::Send, |view, _, cx| view.send_prompt(cx))
        };
        theme::column()
            .flex_shrink_0()
            .m_3()
            .mt_0()
            .rounded(theme::scaled(8.))
            .border_1()
            .border_color(rgb(theme::colors().border))
            .bg(rgb(theme::colors().panel))
            .overflow_hidden()
            .child(div().h(theme::scaled(height)).child(self.composer.clone()))
            .child(
                theme::row()
                    .px_2()
                    .pb_2()
                    .gap_2()
                    .child(footer)
                    .child(action),
            )
            .into_any_element()
    }

    /// Messages and tool calls in the order they happened, then the files the agent changed.
    fn conversation(&self, cx: &Context<Self>) -> AnyElement {
        let colors = theme::colors();
        let messages = &self.snapshot.messages;
        let mut tools = self.snapshot.tool_calls.iter().peekable();
        let mut items: Vec<AnyElement> = Vec::new();
        // The agent's name heads each of its turns once, not every paragraph of it.
        let mut named = false;
        let name = |items: &mut Vec<AnyElement>, named: &mut bool| {
            if !*named {
                items.push(self.agent_name());
                *named = true;
            }
        };
        for index in 0..=messages.len() {
            let last = index == messages.len();
            let mut calls = Vec::new();
            while let Some(tool) = tools.next_if(|tool| last || tool.after_message <= index) {
                calls.push(tool_row(tool));
            }
            if !calls.is_empty() {
                name(&mut items, &mut named);
                items.push(
                    theme::column()
                        .flex_shrink_0()
                        .gap(theme::scaled(2.))
                        .pl_2()
                        .border_l_2()
                        .border_color(rgb(colors.border))
                        .children(calls)
                        .into_any_element(),
                );
            }
            let Some(message) = messages.get(index) else {
                continue;
            };
            match message.role {
                MessageRole::User => {
                    named = false;
                    items.push(
                        div()
                            .flex_shrink_0()
                            .ml(theme::scaled(28.))
                            .mt_1()
                            .px_3()
                            .py_2()
                            .rounded(theme::scaled(8.))
                            // The border keeps the bubble apart from the ground in the light
                            // themes, where the raised surface is close to it.
                            .border_1()
                            .border_color(rgb(colors.border))
                            .bg(rgb(colors.raised))
                            .whitespace_normal()
                            .child(message.text.clone())
                            .into_any_element(),
                    );
                }
                MessageRole::Agent => {
                    name(&mut items, &mut named);
                    let blocks = self.replies.get(index).cloned().unwrap_or_default();
                    items.push(
                        div()
                            .flex_shrink_0()
                            .child(message_blocks(
                                &blocks,
                                &format!("reply-{index}"),
                                self.link_opener(cx),
                            ))
                            .into_any_element(),
                    );
                }
                MessageRole::Thought => {
                    name(&mut items, &mut named);
                    items.push(
                        theme::column()
                            .flex_shrink_0()
                            .gap(theme::scaled(2.))
                            .pl_2()
                            .border_l_2()
                            .border_color(rgb(colors.border))
                            .text_size(theme::scaled(theme::SMALL_TEXT))
                            .child(
                                div()
                                    .text_size(theme::scaled(theme::TINY_TEXT))
                                    .text_color(rgb(colors.faint))
                                    .child("Reasoning"),
                            )
                            .child(
                                div()
                                    .whitespace_normal()
                                    .italic()
                                    .text_color(rgb(colors.muted))
                                    .child(message.text.trim().to_owned()),
                            )
                            .into_any_element(),
                    );
                }
            }
        }
        theme::column()
            .id("conversation-scroll")
            .flex_1()
            .px_3()
            .py_2()
            .gap_2()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .when(messages.is_empty() && items.is_empty(), |view| {
                view.child(
                    div()
                        .text_size(theme::scaled(theme::SMALL_TEXT))
                        .text_color(rgb(colors.muted))
                        .whitespace_normal()
                        .child(if self.snapshot.status == AgentStatus::Starting {
                            "Starting the agent. The first start can take a minute while its \
                             adapter is fetched."
                        } else {
                            "Give this agent a task. Its progress, questions, and tool activity \
                             will appear here."
                        }),
                )
            })
            .children(items)
            .children(self.changed_files(cx))
            .into_any_element()
    }

    /// The agent's logo and name, heading a turn.
    fn agent_name(&self) -> AnyElement {
        let provider = self.snapshot.provider;
        theme::row()
            .flex_shrink_0()
            .gap_2()
            .mt_1()
            .text_size(theme::scaled(theme::TINY_TEXT))
            .text_color(rgb(theme::colors().muted))
            .child(icon(agent_icon(provider), agent_color(provider)))
            .child(provider.label())
            .into_any_element()
    }

    /// What a link in a reply does: web links open in the browser, project files in Code.
    fn link_opener(&self, cx: &Context<Self>) -> LinkOpener {
        let view = cx.weak_entity();
        std::rc::Rc::new(move |destination, cx| {
            match markdown::link_target(Path::new(""), destination) {
                LinkTarget::External(url) => cx.open_url(&url),
                LinkTarget::File(path) => {
                    if let Err(error) =
                        view.update(cx, |_, cx| cx.emit(AgentAction::OpenProjectFile(path)))
                    {
                        tracing::warn!(target: "orin::agents", %error, "link clicked after the session closed");
                    }
                }
                LinkTarget::None => {
                    tracing::debug!(target: "orin::agents", destination, "link has nothing to open");
                }
            }
        })
    }
}
