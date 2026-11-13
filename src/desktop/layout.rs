// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:46:34.369798
//+ Revised: 	2026/11/13 07:53:50.902689
// -----------------------------------------------------------------------------

//! Window framing: title bar, the four workspace columns side by side, and the status bar.

use super::{
    DefaultTextSize, LargerText, OpenProject, OpenSettings, Quit, RefreshProject, ResetLayout,
    ShowWelcome, SmallerText, ToggleMarkdownPreview, ToggleTerminal,
    assets::{self, Icon},
    controls::{ControlSpec, Emphasis, button, hint, icon, measured},
    theme,
    workspace::Workspace,
};
use crate::settings::{Column, Page, SettingsSection};
use gpui::{AnyElement, Context, MouseButton, MouseMoveEvent, Window, div, img, prelude::*, rgb};

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.controls.borrow_mut().clear();
        self.scale_factor = window.scale_factor();
        theme::apply(&self.settings.appearance);
        // GPUI spacing helpers are in rems, so the whole layout follows the chosen text size.
        window.set_rem_size(theme::scaled(theme::REM));
        // The width beside the activity bar.
        let page = Self::page_width(window);
        theme::column()
            .size_full()
            .bg(rgb(theme::colors().background))
            .text_color(rgb(theme::colors().text))
            .font_family(theme::interface_font())
            .text_size(theme::scaled(theme::TEXT_SIZE))
            .track_focus(&self.focus)
            .on_action(cx.listener(|view, _: &Quit, window, cx| view.begin_shutdown(window, cx)))
            .on_action(
                cx.listener(|view, _: &OpenProject, window, cx| view.pick_project(window, cx)),
            )
            .on_action(cx.listener(|view, _: &RefreshProject, _, cx| view.refresh_project(cx)))
            .on_action(cx.listener(|view, _: &ToggleTerminal, _, cx| view.toggle_terminal(cx)))
            .on_action(cx.listener(|view, _: &LargerText, _, cx| view.step_text_size(1., cx)))
            .on_action(cx.listener(|view, _: &SmallerText, _, cx| view.step_text_size(-1., cx)))
            .on_action(cx.listener(|view, _: &DefaultTextSize, _, cx| view.default_text_size(cx)))
            .on_action(
                cx.listener(|view, _: &ShowWelcome, window, cx| view.show_welcome(window, cx)),
            )
            .on_action(cx.listener(|view, _: &OpenSettings, window, cx| {
                view.open_settings(SettingsSection::Appearance, window, cx)
            }))
            .on_action(cx.listener(|view, _: &ResetLayout, _, cx| view.reset_layout(cx)))
            .on_action(cx.listener(|view, _: &ToggleMarkdownPreview, _, cx| {
                view.toggle_markdown_preview(cx)
            }))
            .on_mouse_move(
                cx.listener(|view, event: &MouseMoveEvent, _, cx| view.drag_divider(event, cx)),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| view.end_pointer_drag(cx)),
            )
            .child(self.title_bar(cx))
            .children(self.error_banner(cx))
            .children(self.update_banner(cx))
            .child(
                // The activity bar runs down the left edge beside whatever page is shown.
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.activity_bar(cx))
                    .child(match self.settings.page {
                        Some(Page::Settings(section)) => self.settings_page(section, page, cx),
                        Some(Page::Welcome) => self.welcome_page(page, cx),
                        None => self.columns_row(page, Self::column_content, cx),
                    }),
            )
            .child(self.status_bar(cx))
            .children(self.menu_overlay(cx))
    }
}

impl Workspace {
    /// Header actions and body for one expanded column.
    fn column_content(&self, column: Column, cx: &Context<Self>) -> (Vec<AnyElement>, AnyElement) {
        match column {
            Column::Files => (self.explorer_actions(cx), self.explorer(cx)),
            Column::Agents => (self.agent_actions(cx), self.agent_workspace(cx)),
            Column::Code => (self.code_actions(cx), self.code_pane(cx)),
            Column::Terminal => (self.terminal_actions(cx), self.terminal_pane()),
        }
    }

    /// Device-pixel size for an image shown at `logical` pixels, so it is never resampled by the GPU.
    pub fn device_pixels(&self, logical: f32) -> u32 {
        (logical * self.scale_factor).round() as u32
    }

    /// Like VS Code's title bar: the logo (Welcome) on the left, a command-center box with the
    /// project in the middle, and Open on the right. Accounts and Settings are in the activity bar.
    fn title_bar(&self, cx: &Context<Self>) -> AnyElement {
        let colors = theme::colors();
        let project = self
            .project
            .as_ref()
            .map(|project| crate::process::launch::native_path(project.root()))
            .and_then(|root| {
                root.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            });
        let welcome_open = self.settings.page == Some(Page::Welcome);
        let logo = theme::row()
            .id("open-welcome")
            .relative()
            .h(theme::scaled(26.))
            .px_2()
            .gap_2()
            .rounded(theme::scaled(theme::RADIUS))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(theme::colors().hover)))
            .tooltip(hint("Welcome"))
            .child(
                img(assets::mascot(self.device_pixels(theme::metric(18.))))
                    .size(theme::scaled(18.)),
            )
            .child(div().text_color(rgb(colors.text)).child("Orin"))
            .child(measured("open-welcome", !self.closing, &self.controls))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, window, cx| {
                    cx.stop_propagation();
                    view.show_welcome(window, cx)
                }),
            );
        let center = theme::row()
            .id("command-center")
            .relative()
            .w(theme::scaled(420.))
            .min_w(theme::scaled(160.))
            .h(theme::scaled(24.))
            .px_2()
            .gap_2()
            .justify_center()
            .rounded(theme::scaled(6.))
            .border_1()
            .border_color(rgb(if welcome_open {
                colors.accent
            } else {
                colors.border
            }))
            .bg(rgb(colors.background))
            .text_size(theme::scaled(theme::SMALL_TEXT))
            .text_color(rgb(colors.muted))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(theme::colors().hover)))
            .tooltip(hint("Recent folders and ways to start"))
            .child(icon(Icon::Search, colors.muted))
            .child(
                div()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(project.unwrap_or_else(|| "Open a folder to start".into())),
            )
            .child(measured("command-center", !self.closing, &self.controls))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, window, cx| {
                    cx.stop_propagation();
                    view.show_welcome(window, cx)
                }),
            );
        theme::row()
            .h(theme::scaled(theme::TITLE_BAR_HEIGHT))
            .flex_shrink_0()
            .px_2()
            .gap_1()
            .bg(rgb(colors.panel))
            .border_b_1()
            .border_color(rgb(colors.border))
            .child(div().flex_1().flex().child(logo))
            .child(center)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .child(
                        ControlSpec::new("open-project", &self.controls, cx)
                            .enabled(self.can_open_project(cx))
                            .tooltip(
                                self.project_switch_blocked(cx)
                                    .unwrap_or("Open folder (Ctrl+Shift+O)"),
                            )
                            .icon_text(
                                Icon::OpenFolder,
                                if self.loading { "Opening…" } else { "Open" },
                                |view, window, cx| view.pick_project(window, cx),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// "Orin 0.3.0 is available.", its download progress, or its installed state; styled like
    /// the error banner, in the accent colours.
    fn update_banner(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if !self.update.banner_visible() {
            return None;
        }
        let colors = theme::colors();
        let mut banner = theme::row()
            .id("update-banner")
            .flex_shrink_0()
            .px_3()
            .py_1()
            .gap_2()
            .bg(rgb(colors.added_surface))
            .text_color(rgb(colors.text))
            .child(icon(Icon::Refresh, colors.accent));
        if let Some((version, _)) = &self.update.installed {
            banner = banner
                .child(div().flex_1().whitespace_normal().child(
                    if self.update.relaunch_after_quit.is_some() {
                        format!("Orin {version} is installed. It starts when you close Orin.")
                    } else {
                        format!(
                            "Orin {version} is installed. It starts the next time you open Orin."
                        )
                    },
                ))
                .child(button(
                    "update-restart",
                    "Restart now",
                    !self.closing,
                    true,
                    &self.controls,
                    cx,
                    |view, window, cx| view.restart_into_update(window, cx),
                ));
        } else if let Some(downloading) = &self.update.downloading {
            banner = banner
                .child(
                    div()
                        .flex_1()
                        .whitespace_normal()
                        .child(Self::download_text(downloading)),
                )
                .child(self.install_button(cx));
        } else {
            let version = self
                .update
                .status
                .as_ref()
                .and_then(crate::update::check::UpdateStatus::offered)?;
            banner = banner
                .child(
                    div()
                        .flex_1()
                        .whitespace_normal()
                        .child(format!("Orin {version} is available.")),
                )
                .child(self.install_button(cx))
                .child(button(
                    "update-skip",
                    "Skip this version",
                    !self.closing,
                    false,
                    &self.controls,
                    cx,
                    |view, _, cx| view.skip_update(cx),
                ))
                .child(
                    ControlSpec::new("update-dismiss", &self.controls, cx)
                        .tooltip("Hide until Orin starts again")
                        .icon(Icon::Close, |view, _, cx| view.dismiss_update(cx)),
                );
        }
        Some(banner.into_any_element())
    }

    fn error_banner(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        self.error
            .iter()
            .map(|message| {
                theme::row()
                    .flex_shrink_0()
                    .px_3()
                    .py_1()
                    .gap_2()
                    .bg(rgb(theme::colors().error_surface))
                    .text_color(rgb(theme::colors().error))
                    .child(icon(Icon::Error, theme::colors().error))
                    .child(div().flex_1().whitespace_normal().child(message.clone()))
                    .when(!self.writable, |view| {
                        view.child(button(
                            "retry-saving",
                            "Retry saving",
                            true,
                            false,
                            &self.controls,
                            cx,
                            |view, _, cx| view.retry_saving(cx),
                        ))
                    })
                    .when(!self.discard_on_quit && self.editor_busy(cx), |view| {
                        view.child(button(
                            "discard-close",
                            "Discard & close",
                            true,
                            false,
                            &self.controls,
                            cx,
                            |view, window, cx| {
                                view.discard_on_quit = true;
                                view.begin_shutdown(window, cx);
                            },
                        ))
                    })
                    .child(
                        ControlSpec::new("dismiss-error", &self.controls, cx)
                            .tooltip("Dismiss")
                            .icon(Icon::Close, |view, _, cx| {
                                view.error = None;
                                cx.notify();
                            }),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn terminal_actions(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let running = self
            .shell
            .as_ref()
            .is_some_and(|tab| tab.view.read(cx).running());
        vec![
            ControlSpec::new("terminal-start", &self.controls, cx)
                .enabled(self.project.is_some() && !running && !self.launching && !self.closing)
                .tooltip("Start shell")
                .icon(Icon::Start, |view, _, cx| view.launch_shell(cx)),
            ControlSpec::new("terminal-stop", &self.controls, cx)
                .enabled(running && !self.closing)
                .tooltip("Stop shell")
                .icon(Icon::Stop, |view, _, cx| view.stop_shell(cx)),
        ]
    }

    fn terminal_pane(&self) -> AnyElement {
        match &self.shell {
            Some(tab) => div()
                .size_full()
                .p_2()
                .bg(rgb(theme::colors().background))
                .child(tab.view.clone())
                .into_any_element(),
            None => empty_hint(if self.project.is_some() {
                "Start a shell to run tests, inspect your environment, or build the project."
            } else {
                "Open a project to start a shell in its folder."
            }),
        }
    }

    /// Like VS Code's: state on the left; the open file's language mode, sessions, and version
    /// on the right.
    fn status_bar(&self, cx: &Context<Self>) -> AnyElement {
        let language = self
            .settings
            .selected_file
            .as_ref()
            .and(self.editor.as_ref())
            .map(|editor| {
                editor.read(cx).highlighter.as_ref().map_or_else(
                    || "Plain Text".to_owned(),
                    |highlighter| highlighter.language().to_owned(),
                )
            });
        let status = if self.closing {
            "Stopping sessions and saving workspace…"
        } else if self.loading {
            "Opening project…"
        } else if !self.writable {
            "Workspace settings are not being saved"
        } else {
            "Ready"
        };
        let sessions = match self.agents.len() {
            1 => "1 agent".to_owned(),
            count => format!("{count} agents"),
        };
        theme::row()
            .h(theme::scaled(theme::STATUS_BAR_HEIGHT))
            .flex_shrink_0()
            .px_3()
            .gap_3()
            .bg(rgb(theme::colors().panel))
            .border_t_1()
            .border_color(rgb(theme::colors().border))
            .text_size(theme::scaled(theme::TINY_TEXT))
            .text_color(rgb(theme::colors().muted))
            .children(self.git.as_ref().map(|git| {
                theme::row()
                    .id("status-branch")
                    .relative()
                    .gap_1()
                    .child(measured("status-branch", true, &self.controls))
                    .child(icon(Icon::GitBranch, theme::colors().muted))
                    .child(git.branch.clone().unwrap_or_else(|| "detached".into()))
                    .tooltip(hint(format!(
                        "Git branch · {} changed file{}",
                        git.files.len(),
                        if git.files.len() == 1 { "" } else { "s" }
                    )))
            }))
            .when(self.git_failed, |bar| {
                bar.child(
                    div()
                        .id("status-git-failed")
                        .child("Git status unavailable"),
                )
            })
            .child(status)
            .child(div().flex_1())
            .children(language.map(|language| {
                div()
                    .id("status-language")
                    .relative()
                    .child(language)
                    .child(measured("status-language", true, &self.controls))
            }))
            .child(sessions)
            .child(format!("Orin {}", env!("CARGO_PKG_VERSION")))
            .into_any_element()
    }
}

/// Muted guidance shown in an empty column.
pub fn empty_hint(text: &'static str) -> AnyElement {
    div()
        .p_3()
        .text_size(theme::scaled(theme::SMALL_TEXT))
        .text_color(rgb(theme::colors().muted))
        .whitespace_normal()
        .child(text)
        .into_any_element()
}

/// The single main action of an empty area, given its own prominent row.
pub fn primary_action<T: 'static>(
    spec: ControlSpec<'_, '_, T>,
    glyph: Icon,
    label: &'static str,
    action: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
) -> AnyElement {
    div()
        .px_3()
        .pt_3()
        .child(
            spec.emphasis(Emphasis::Primary)
                .icon_text(glyph, label, action),
        )
        .into_any_element()
}
