// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 10:31:31.014652
//+ Revised: 	2026/11/14 01:02:32.487721
// -----------------------------------------------------------------------------

//! The Welcome page, like VS Code's: ways to start, recent folders, and a few first steps.
//!
//! It opens on a start without a project and from the Orin logo in the title bar.

use super::{
    assets::{self, Icon},
    controls::{ControlSpec, Emphasis, hint, icon, measured},
    theme,
    workspace::Workspace,
};
use crate::settings::{Page, SettingsSection};
use gpui::{
    AnyElement, Context, FontWeight, MouseButton, SharedString, div, img, prelude::*, px, rgb,
};
use std::path::{Path, PathBuf};

const MAX_WIDTH: f32 = 960.0;
const GAP: f32 = 48.0;

impl Workspace {
    pub fn show_welcome(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.close_menu(window, cx);
        if self.project.is_some() {
            self.close_project(cx);
            return;
        }
        if self.settings.page != Some(Page::Welcome) {
            self.settings.page = Some(Page::Welcome);
            self.changed(cx);
        }
    }

    /// Leave the Welcome page for the columns, when a project is open to show.
    pub fn close_welcome(&mut self, cx: &mut Context<Self>) {
        if self.settings.page == Some(Page::Welcome) && self.project.is_some() {
            self.settings.page = None;
            self.changed(cx);
        }
    }

    pub fn forget_recent(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.settings
            .recent_projects
            .retain(|recent| recent != path);
        tracing::info!(target: "orin::project", "folder removed from the recent list");
        self.changed(cx);
    }

    /// The page for a window `total` logical pixels wide.
    pub fn welcome_page(&self, total: f32, cx: &Context<Self>) -> AnyElement {
        let colors = theme::colors();
        // GPUI wraps text only inside widths known in pixels, so the columns are measured here.
        let inner = (total - theme::metric(2. * GAP))
            .min(theme::metric(MAX_WIDTH))
            .max(0.);
        let column_width = ((inner - theme::metric(GAP)) / 2.).max(0.);
        let header = theme::row()
            .gap_4()
            .child(
                img(assets::mascot(self.device_pixels(theme::metric(64.))))
                    .size(theme::scaled(64.)),
            )
            .child(
                theme::column()
                    .gap_1()
                    .child(
                        div()
                            .text_size(theme::scaled(30.))
                            .text_color(rgb(colors.text))
                            .child("Orin"),
                    )
                    .child(
                        div()
                            .text_size(theme::scaled(15.))
                            .text_color(rgb(colors.muted))
                            .child("Supervise your coding agents"),
                    ),
            );
        let start = section("Start").children(self.start_links(cx));
        let recent = section("Recent").children(self.recent_rows(column_width, cx));
        let steps = section("Get started").children(self.get_started(column_width, cx));
        div()
            .id("welcome-page")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .bg(rgb(colors.background))
            .child(
                theme::column()
                    .w(px(inner))
                    .mx_auto()
                    .pt(theme::scaled(56.))
                    .pb_8()
                    .gap(theme::scaled(40.))
                    .child(header)
                    .child(
                        div()
                            .flex()
                            .gap(theme::scaled(GAP))
                            .child(
                                theme::column()
                                    .w(px(column_width))
                                    .flex_shrink_0()
                                    .gap(theme::scaled(32.))
                                    .child(start)
                                    .child(recent),
                            )
                            .child(
                                theme::column()
                                    .w(px(column_width))
                                    .flex_shrink_0()
                                    .child(steps),
                            ),
                    ),
            )
            .child(measured("welcome-page", true, &self.controls))
            .into_any_element()
    }

    fn start_links(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        // Opening a folder is what this page is for, so it is the one prominent button; the
        // other ways to start are quieter links below it.
        let open = theme::row()
            .gap_3()
            .pb_1()
            .child(
                ControlSpec::new("welcome-open-folder", &self.controls, cx)
                    .enabled(self.can_open_project(cx))
                    .emphasis(Emphasis::Primary)
                    .large()
                    .tooltip(
                        self.project_switch_blocked(cx)
                            .unwrap_or("Choose a project folder"),
                    )
                    .icon_text(Icon::OpenFolder, "Open Folder…", |view, window, cx| {
                        view.pick_project(window, cx)
                    }),
            )
            .child(
                div()
                    .text_size(theme::scaled(theme::SMALL_TEXT))
                    .text_color(rgb(theme::colors().faint))
                    .child("Ctrl+Shift+O"),
            )
            .into_any_element();
        let mut links = vec![
            open,
            link(
                Link {
                    id: "welcome-settings",
                    glyph: Icon::Settings,
                    label: "Settings".into(),
                    shortcut: Some("Ctrl+,"),
                    enabled: !self.closing,
                },
                &self.controls,
                cx,
                |view, window, cx| view.open_settings(SettingsSection::Appearance, window, cx),
            ),
        ];
        if let Some(project) = &self.project {
            let name = folder_name(project.root());
            links.insert(
                0,
                link(
                    Link {
                        id: "welcome-back",
                        glyph: Icon::Files,
                        label: format!("Back to {name}").into(),
                        shortcut: None,
                        enabled: !self.closing,
                    },
                    &self.controls,
                    cx,
                    |view, _, cx| view.close_welcome(cx),
                ),
            );
        }
        links
    }

    fn recent_rows(&self, width: f32, cx: &Context<Self>) -> Vec<AnyElement> {
        let colors = theme::colors();
        if self.settings.recent_projects.is_empty() {
            return vec![
                div()
                    .text_color(rgb(colors.muted))
                    .child("Folders you open appear here.")
                    .into_any_element(),
            ];
        }
        let openable = self.can_open_project(cx);
        let blocked = self.project_switch_blocked(cx);
        self.settings
            .recent_projects
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let id = format!("recent-{index}");
                let open: PathBuf = path.clone();
                let forget: PathBuf = path.clone();
                theme::row()
                    .id(SharedString::from(id.clone()))
                    .relative()
                    .w(px(width))
                    .h(theme::scaled(28.))
                    .gap_2()
                    .px_1()
                    .rounded(theme::scaled(theme::RADIUS))
                    // A row that can't open now neither looks nor acts clickable.
                    .when(openable, |row| {
                        row.cursor_pointer()
                            .hover(|style| style.bg(rgb(theme::colors().hover)))
                    })
                    .tooltip(hint(blocked.map_or_else(
                        || path.display().to_string(),
                        |reason| format!("{}: {reason}", path.display()),
                    )))
                    .child(icon(
                        Icon::Folder,
                        if openable {
                            colors.walnut
                        } else {
                            colors.muted
                        },
                    ))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(rgb(if openable {
                                colors.accent
                            } else {
                                colors.muted
                            }))
                            .child(folder_name(path)),
                    )
                    .child(
                        theme::clipped(
                            path.parent()
                                .map(|parent| parent.display().to_string())
                                .unwrap_or_default(),
                        )
                        .text_color(rgb(colors.muted)),
                    )
                    .child(
                        ControlSpec::new(format!("recent-remove-{index}"), &self.controls, cx)
                            .enabled(!self.closing)
                            .tooltip("Remove from Recent")
                            .icon(Icon::Close, move |view, _, cx| {
                                view.forget_recent(&forget, cx)
                            }),
                    )
                    .child(measured(id, openable, &self.controls))
                    .when(openable, |row| {
                        row.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, _, _, cx| view.open_project(open.clone(), cx)),
                        )
                    })
                    .into_any_element()
            })
            .collect()
    }

    fn get_started(&self, width: f32, cx: &Context<Self>) -> Vec<AnyElement> {
        let steps: [(&str, Icon, &str, &str, SettingsSection); 3] = [
            (
                "welcome-step-theme",
                Icon::Preview,
                "Make it yours",
                "Pick a theme, the interface and code fonts, and the text size.",
                SettingsSection::Appearance,
            ),
            (
                "welcome-step-agents",
                Icon::Account,
                "Connect your agents",
                "Sign in to Claude Code and Codex with their own tools; Orin starts them for you.",
                SettingsSection::Accounts,
            ),
            (
                "welcome-step-layout",
                Icon::ResetLayout,
                "Arrange the columns",
                "Drag headers to move Files, Code, Agents, and Terminal; drag dividers to resize.",
                SettingsSection::Layout,
            ),
        ];
        let colors = theme::colors();
        steps
            .into_iter()
            .map(|(id, glyph, title, description, target)| {
                div()
                    .id(id)
                    .relative()
                    .w(px(width))
                    .flex()
                    .gap_3()
                    .p_3()
                    .mb_2()
                    .rounded(theme::scaled(6.))
                    .bg(rgb(colors.panel))
                    .border_1()
                    .border_color(rgb(colors.border))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(theme::colors().hover)))
                    .child(div().pt(px(2.)).child(icon(glyph, colors.accent)))
                    .child(
                        theme::column()
                            .w(px((width - theme::metric(56.)).max(0.)))
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(colors.text))
                                    .child(title),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .whitespace_normal()
                                    .text_size(theme::scaled(theme::SMALL_TEXT))
                                    .text_color(rgb(colors.muted))
                                    .child(description),
                            ),
                    )
                    .child(measured(id, !self.closing, &self.controls))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, window, cx| {
                            view.open_settings(target, window, cx)
                        }),
                    )
                    .into_any_element()
            })
            .collect()
    }
}

fn section(title: &'static str) -> gpui::Div {
    theme::column().gap_2().child(
        div()
            .pb_1()
            .text_size(theme::scaled(17.))
            .text_color(rgb(theme::colors().text))
            .child(title),
    )
}

/// What a Start link shows.
struct Link {
    id: &'static str,
    glyph: Icon,
    label: SharedString,
    shortcut: Option<&'static str>,
    enabled: bool,
}

/// A link-styled action with its icon and, optionally, its shortcut.
fn link(
    spec: Link,
    controls: &super::controls::Controls,
    cx: &Context<Workspace>,
    action: impl Fn(&mut Workspace, &mut gpui::Window, &mut Context<Workspace>) + 'static,
) -> AnyElement {
    let Link {
        id,
        glyph,
        label,
        shortcut,
        enabled,
    } = spec;
    let colors = theme::colors();
    theme::row()
        .id(id)
        .relative()
        .h(theme::scaled(28.))
        .gap_2()
        .px_1()
        .rounded(theme::scaled(theme::RADIUS))
        .text_color(rgb(if enabled {
            colors.accent
        } else {
            colors.disabled_text
        }))
        .child(icon(
            glyph,
            if enabled {
                colors.accent
            } else {
                colors.disabled_text
            },
        ))
        .child(label)
        .children(shortcut.map(|keys| {
            div()
                .ml_2()
                .text_size(theme::scaled(theme::SMALL_TEXT))
                .text_color(rgb(colors.faint))
                .child(keys)
        }))
        .child(measured(id, enabled, controls))
        .when(enabled, |row| {
            row.cursor_pointer()
                .hover(|style| style.bg(rgb(theme::colors().hover)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, window, cx| {
                        cx.stop_propagation();
                        action(view, window, cx)
                    }),
                )
        })
        .into_any_element()
}

fn folder_name(path: &Path) -> String {
    let native = crate::process::launch::native_path(path);
    native
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| native.display().to_string())
}
