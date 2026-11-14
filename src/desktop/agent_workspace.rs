// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:46:34.926090
//+ Revised: 	2026/11/14 01:02:28.281985
// -----------------------------------------------------------------------------

//! The Agents column: one tab per session, the selected conversation, and agent launchers.

use super::{
    agent_actions::provider_id,
    agent_view::{status_color, status_label},
    assets::{self, Icon},
    controls::{ControlSpec, measured},
    theme,
    workspace::Workspace,
};
use crate::agents::Provider;
use gpui::{AnyElement, Context, MouseButton, div, img, prelude::*, px, rgb};

const MASCOT_SIZE: f32 = 72.0;
/// The width of the launch buttons in the empty Agents column.
const LAUNCHER_WIDTH: f32 = 190.0;

fn short_label(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "Claude",
        Provider::Codex => "Codex",
        Provider::Gemini => "Gemini",
    }
}

impl Workspace {
    /// One compact launcher per agent; unavailable agents are greyed out and explain why.
    pub fn agent_actions(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        self.providers
            .iter()
            .map(|(provider, status)| {
                let provider = *provider;
                let tooltip = if status.available {
                    format!("Start {}", provider.label())
                } else {
                    format!(
                        "{} isn't installed. {}",
                        provider.label(),
                        provider.install_hint()
                    )
                };
                ControlSpec::new(
                    format!("agent-{}", provider_id(provider)),
                    &self.controls,
                    cx,
                )
                .enabled(self.can_launch(provider))
                .tooltip(tooltip)
                .agent_text(provider, "", move |view, _, cx| {
                    view.launch_agent(provider, cx)
                })
            })
            .collect()
    }

    pub fn agent_workspace(&self, cx: &Context<Self>) -> AnyElement {
        let selected = (!self.agents.is_empty())
            .then(|| self.settings.active_session.min(self.agents.len() - 1));
        let body = match selected {
            Some(selected) => div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(self.agents[selected].view.clone())
                .into_any_element(),
            None => self.empty_agents(cx),
        };
        theme::column()
            .flex_1()
            .when_some(selected, |column, selected| {
                column.children(self.session_options(selected, cx))
            })
            .child(body)
            .into_any_element()
    }

    /// The agent's model, effort, and mode as drop-down chips, like a model picker.
    fn session_options(&self, selected: usize, cx: &Context<Self>) -> Option<AnyElement> {
        let snapshot = &self.agents[selected].view.read(cx).snapshot;
        let shown: Vec<_> = crate::agents::options::SHOWN_CATEGORIES
            .iter()
            .filter_map(|category| {
                snapshot
                    .config_options
                    .iter()
                    .find(|option| option.category.as_deref() == Some(*category))
            })
            .collect();
        if shown.is_empty() {
            return None;
        }
        let busy = snapshot.is_busy();
        Some(
            theme::row()
                .id("agent-options")
                .flex_shrink_0()
                .px_2()
                .py_1()
                .gap_1()
                .overflow_x_scroll()
                .border_b_1()
                .border_color(rgb(theme::colors().border))
                .children(shown.into_iter().map(|option| {
                    let id = format!("agent-option-chip-{}", option.id);
                    let menu = super::menus::Menu::AgentOption {
                        session: selected,
                        option: option.id.clone(),
                    };
                    let open = self.menu.as_ref().is_some_and(|open| open.menu == menu);
                    let anchor = id.clone();
                    ControlSpec::new(id, &self.controls, cx)
                        .enabled(!busy && !self.closing)
                        .emphasis(if open {
                            super::controls::Emphasis::Selected
                        } else {
                            super::controls::Emphasis::Normal
                        })
                        .tooltip(if busy {
                            format!("{} can change once the agent finishes", option.name)
                        } else {
                            format!("Choose the {}", option.name.to_lowercase())
                        })
                        .text_icon(
                            option.current_name().to_owned(),
                            Icon::ChevronDown,
                            move |view, window, cx| {
                                view.toggle_menu(menu.clone(), &anchor, window, cx)
                            },
                        )
                }))
                .into_any_element(),
        )
    }

    /// One tab per running session; scrolls sideways when there are more than fit.
    pub fn agent_tabs(&self, cx: &Context<Self>) -> AnyElement {
        if self.agents.is_empty() {
            return div().flex_1().into_any_element();
        }
        self.session_tabs(self.settings.active_session.min(self.agents.len() - 1), cx)
    }

    fn session_tabs(&self, selected: usize, cx: &Context<Self>) -> AnyElement {
        theme::row()
            .id("agent-tabs")
            .h(theme::scaled(theme::HEADER_HEIGHT))
            .flex_1()
            .min_w_0()
            .bg(rgb(theme::colors().panel))
            .border_b_1()
            .border_color(rgb(theme::colors().border))
            .px_1()
            .gap_1()
            .overflow_x_scroll()
            .children(self.agents.iter().enumerate().map(|(index, tab)| {
                let status = tab.view.read(cx).snapshot.status;
                let active = index == selected;
                let id = format!("agent-session-{index}");
                theme::row()
                    .id(gpui::SharedString::from(id.clone()))
                    .relative()
                    .h(theme::scaled(theme::HEADER_HEIGHT))
                    .flex_shrink_0()
                    .pl_2()
                    .pr_1()
                    .gap_2()
                    .border_r_1()
                    .border_color(rgb(theme::colors().border))
                    .bg(rgb(if active {
                        theme::colors().background
                    } else {
                        theme::colors().panel
                    }))
                    .text_size(theme::scaled(theme::SMALL_TEXT))
                    .text_color(rgb(if active {
                        theme::colors().text
                    } else {
                        theme::colors().muted
                    }))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(theme::colors().hover)))
                    .child(
                        div()
                            .size(theme::scaled(7.))
                            .rounded_full()
                            .bg(rgb(status_color(status))),
                    )
                    .child(super::controls::icon(
                        super::controls::agent_icon(tab.provider),
                        theme::colors().text,
                    ))
                    .child(format!("{} {}", short_label(tab.provider), index + 1))
                    .child(
                        ControlSpec::new(format!("close-agent-{index}"), &self.controls, cx)
                            .enabled(!self.closing)
                            .tooltip(format!("Close session ({})", status_label(status)))
                            .icon(Icon::Close, move |view, _, cx| view.close_agent(index, cx)),
                    )
                    .child(measured(id, !self.closing, &self.controls))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| {
                            view.settings.active_session = index;
                            view.changed(cx);
                        }),
                    )
            }))
            .into_any_element()
    }

    /// Nothing running yet: the mascot, then one prominent button per agent. Agents that can't
    /// start are greyed out, with a single explanation and the way to set them up.
    fn empty_agents(&self, cx: &Context<Self>) -> AnyElement {
        let colors = theme::colors();
        let missing = self.providers.iter().any(|(_, status)| !status.available);
        let launchers = self.providers.iter().map(|(provider, status)| {
            let provider = *provider;
            ControlSpec::new(
                format!("start-{}", provider_id(provider)),
                &self.controls,
                cx,
            )
            .enabled(self.can_launch(provider))
            .large()
            .full_width()
            .tooltip(if status.available {
                format!("Start a {} session", provider.label())
            } else {
                format!(
                    "{} isn't installed. {}",
                    provider.label(),
                    provider.install_hint()
                )
            })
            .agent_text(provider, provider.label(), move |view, _, cx| {
                view.launch_agent(provider, cx)
            })
        });
        theme::column()
            .id("agent-welcome")
            .flex_1()
            .overflow_y_scroll()
            .items_center()
            .justify_center()
            .p_4()
            .gap_2()
            .child(img(assets::mascot(self.device_pixels(MASCOT_SIZE))).size(px(MASCOT_SIZE)))
            .child(
                div()
                    .text_size(theme::scaled(theme::HEADING_TEXT))
                    .child("What will you create?"),
            )
            .child(
                div()
                    .text_size(theme::scaled(theme::SMALL_TEXT))
                    .text_color(rgb(colors.muted))
                    .text_center()
                    .child(if self.project.is_some() {
                        "Start an agent and give it an intention."
                    } else {
                        "Open a project, then start an agent to carry it forward."
                    }),
            )
            .child(
                // One per row at a shared width: a row of buttons would wrap unevenly in a
                // narrow column.
                theme::column()
                    .w(theme::scaled(LAUNCHER_WIDTH))
                    .flex_shrink_0()
                    .gap_2()
                    .pt_2()
                    .children(launchers),
            )
            .when(missing, |view| {
                view.child(
                    div()
                        .max_w(theme::scaled(360.))
                        .pt_2()
                        .text_size(theme::scaled(theme::SMALL_TEXT))
                        .text_color(rgb(colors.muted))
                        .whitespace_normal()
                        .text_center()
                        .child(
                            "A greyed-out agent needs its command-line tool and Node.js \
                             before it can start.",
                        ),
                )
                .child(
                    ControlSpec::new("setup-agents", &self.controls, cx)
                        .enabled(!self.closing)
                        .tooltip("Open Settings › Accounts")
                        .icon_text(Icon::Settings, "Set up agents", |view, window, cx| {
                            view.open_settings(
                                crate::settings::SettingsSection::Accounts,
                                window,
                                cx,
                            )
                        }),
                )
            })
            .into_any_element()
    }
}
