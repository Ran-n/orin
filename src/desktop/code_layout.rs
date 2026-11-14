// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:46:34.725792
//+ Revised: 	2026/11/14 01:02:28.767845
// -----------------------------------------------------------------------------

//! The Code column: the selected file, editable in place, or its unstaged or staged Git diff.

use super::{
    assets::Icon,
    controls::{ControlSpec, Emphasis, hint, icon},
    file_icons::file_icon,
    layout::{empty_hint, empty_state},
    theme,
    workspace::Workspace,
};
use crate::settings::ViewerMode;
use gpui::{AnyElement, Context, div, prelude::*, px, rgb};

const DIFF_LINE_HEIGHT: f32 = 19.0;
/// What the diff views hold once Git answers that the file has no changes of that kind; an
/// empty text means the answer hasn't arrived yet.
pub const NO_CHANGES: &str = "No changes in this view.";
/// The editor tab strip's height.
const TAB_HEIGHT: f32 = 32.0;

impl Workspace {
    pub fn code_actions(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let previewing = self.previewing_markdown();
        let mut actions = Vec::new();
        if self.can_preview_markdown() {
            actions.push(
                ControlSpec::new("markdown-preview-toggle", &self.controls, cx)
                    .enabled(!self.closing)
                    .emphasis(if previewing {
                        Emphasis::Selected
                    } else {
                        Emphasis::Ghost
                    })
                    .tooltip(if previewing {
                        "Edit source (Ctrl+Shift+V)"
                    } else {
                        "Preview Markdown (Ctrl+Shift+V)"
                    })
                    .icon(
                        if previewing {
                            Icon::EditSource
                        } else {
                            Icon::Preview
                        },
                        |view, _, cx| view.toggle_markdown_preview(cx),
                    ),
            );
        }
        actions.push(
            ControlSpec::new("editor-actions", &self.controls, cx)
                .enabled(self.settings.selected_file.is_some() && !self.closing)
                .emphasis(Emphasis::Ghost)
                .tooltip("Editor actions")
                .icon(Icon::More, |view, window, cx| {
                    view.toggle_menu(super::menus::Menu::Editor, "editor-actions", window, cx)
                }),
        );
        actions
    }

    pub fn code_tab(&self, cx: &Context<Self>) -> AnyElement {
        let Some(path) = self.settings.selected_file.as_ref() else {
            return div().flex_1().into_any_element();
        };
        let dirty = self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).dirty());
        let colors = theme::colors();
        // Like a VS Code editor tab: the editor's ground, an accent line on top, and a dot
        // while there are unsaved changes.
        let tab = theme::row()
            .id("code-path")
            .h(theme::scaled(TAB_HEIGHT))
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .pl_3()
            .pr_1()
            .gap_2()
            .relative()
            .bg(rgb(colors.background))
            .border_r_1()
            .border_color(rgb(colors.border))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(2.))
                    .bg(rgb(colors.accent)),
            )
            .tooltip(hint(path.to_string_lossy().replace('\\', "/")))
            .child({
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_default();
                let (glyph, color) = file_icon(&name);
                icon(glyph, color)
            })
            .child(
                // The name alone fits narrow columns; the full project path is one hover away.
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .relative()
                    .whitespace_nowrap()
                    .text_size(theme::scaled(theme::SMALL_TEXT))
                    .text_color(rgb(colors.text))
                    .child(
                        path.file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ),
            )
            .when(dirty, |row| {
                row.child(
                    div()
                        .size(theme::scaled(7.))
                        .rounded_full()
                        .bg(rgb(colors.walnut))
                        .flex_shrink_0(),
                )
            })
            .child(
                ControlSpec::new("close-file", &self.controls, cx)
                    .enabled(!dirty && !self.closing)
                    .tooltip(if dirty {
                        "Save or reload before closing"
                    } else {
                        "Close"
                    })
                    .icon(Icon::Close, |view, _, cx| view.close_file(cx)),
            );
        tab.into_any_element()
    }

    pub fn code_pane(&self, cx: &Context<Self>) -> AnyElement {
        if self.settings.selected_file.is_none() {
            return empty_state(
                Icon::Code,
                "Select a file in Files to view or edit it here.",
            );
        }
        theme::column()
            .flex_1()
            .child(
                theme::row()
                    .h(theme::scaled(28.))
                    .px_2()
                    .border_b_1()
                    .border_color(rgb(theme::colors().border))
                    .child(self.viewer_modes(cx)),
            )
            .child(div().flex_1().min_h_0().child(self.code_content(cx)))
            .into_any_element()
    }

    /// File / Unstaged / Staged as one segmented control: every choice visible, the current one marked.
    fn viewer_modes(&self, cx: &Context<Self>) -> AnyElement {
        theme::row()
            .flex_shrink_0()
            .p(px(2.))
            .gap(px(2.))
            .rounded(theme::scaled(theme::RADIUS + 1.))
            .bg(rgb(theme::colors().background))
            .children(ViewerMode::ALL.map(|mode| {
                ControlSpec::new(format!("view-{}", mode.key()), &self.controls, cx)
                    .enabled(!self.closing)
                    .emphasis(if self.settings.viewer_mode == mode {
                        Emphasis::Selected
                    } else {
                        Emphasis::Normal
                    })
                    .text(mode.label(), move |view, _, cx| {
                        view.set_viewer_mode(mode, cx)
                    })
            }))
            .into_any_element()
    }

    fn code_content(&self, cx: &Context<Self>) -> AnyElement {
        if self.settings.viewer_mode == ViewerMode::File {
            if self.previewing_markdown()
                && let (Some(editor), Some(path)) = (&self.editor, &self.settings.selected_file)
            {
                return self.markdown_preview(path, editor.read(cx).buffer().text(), cx);
            }
            return match &self.editor {
                Some(editor) => div().size_full().child(editor.clone()).into_any_element(),
                None if self.content.is_empty() => empty_hint("Loading…"),
                None => div()
                    .p_3()
                    .text_size(theme::scaled(theme::SMALL_TEXT))
                    .text_color(rgb(theme::colors().muted))
                    .whitespace_normal()
                    .child(self.content.clone())
                    .into_any_element(),
            };
        }
        if self.content.is_empty() {
            return empty_hint("Loading…");
        }
        if self.content == NO_CHANGES {
            return empty_state(
                Icon::Check,
                match self.settings.viewer_mode {
                    ViewerMode::Staged => "This file has no staged changes.",
                    _ => "This file has no unstaged changes.",
                },
            );
        }
        div()
            .id("diff-content")
            .size_full()
            .overflow_scroll()
            .py_2()
            .font_family(theme::code_font())
            .text_size(theme::scaled(theme::TEXT_SIZE))
            .children(self.content.lines().map(|line| {
                let (color, background) = if line.starts_with("+++") || line.starts_with("---") {
                    (theme::colors().muted, None)
                } else if line.starts_with('+') {
                    (theme::colors().accent, Some(theme::colors().added_surface))
                } else if line.starts_with('-') {
                    (theme::colors().error, Some(theme::colors().removed_surface))
                } else if line.starts_with("@@") {
                    (theme::colors().walnut, None)
                } else {
                    (theme::colors().soft_text, None)
                };
                div()
                    .h(px(DIFF_LINE_HEIGHT))
                    .px_3()
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .text_color(rgb(color))
                    .when_some(background, |line, background| line.bg(rgb(background)))
                    .child(line.to_owned())
            }))
            .into_any_element()
    }
}
