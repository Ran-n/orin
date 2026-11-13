// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:46:34.725792
//+ Revised: 	2026/11/12 22:43:39.256288
// -----------------------------------------------------------------------------

//! The Code column: the selected file, editable in place, or its unstaged or staged Git diff.

use super::{
    assets::Icon,
    controls::{ControlSpec, Emphasis, hint, icon},
    file_icons::file_icon,
    layout::empty_hint,
    theme,
    workspace::Workspace,
};
use crate::settings::ViewerMode;
use gpui::{AnyElement, Context, div, prelude::*, px, rgb};

const DIFF_LINE_HEIGHT: f32 = 19.0;
/// The editor tab strip's height.
const TAB_HEIGHT: f32 = 32.0;

impl Workspace {
    pub fn code_actions(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let editor = self.editor.as_ref().map(|editor| editor.read(cx));
        let previewing = self.previewing_markdown();
        let editing = self.settings.viewer_mode == ViewerMode::File && !previewing;
        let dirty = editor.is_some_and(|editor| editor.dirty());
        let saving = editor.is_some_and(|editor| editor.saving);
        let preview = self.can_preview_markdown().then(|| {
            ControlSpec::new("markdown-preview-toggle", &self.controls, cx)
                .enabled(!self.closing)
                .emphasis(if previewing {
                    Emphasis::Selected
                } else {
                    Emphasis::Normal
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
                )
        });
        preview
            .into_iter()
            .chain([
                ControlSpec::new("editor-undo", &self.controls, cx)
                    .enabled(
                        editing
                            && editor.is_some_and(|editor| editor.buffer().can_undo())
                            && !self.closing,
                    )
                    .tooltip("Undo (Ctrl+Z)")
                    .icon(Icon::Undo, |view, _, cx| {
                        if let Some(editor) = &view.editor {
                            editor.update(cx, |editor, cx| editor.undo(cx));
                        }
                    }),
                ControlSpec::new("editor-redo", &self.controls, cx)
                    .enabled(
                        editing
                            && editor.is_some_and(|editor| editor.buffer().can_redo())
                            && !self.closing,
                    )
                    .tooltip("Redo (Ctrl+Y)")
                    .icon(Icon::Redo, |view, _, cx| {
                        if let Some(editor) = &view.editor {
                            editor.update(cx, |editor, cx| editor.redo(cx));
                        }
                    }),
                ControlSpec::new("editor-reload", &self.controls, cx)
                    .enabled(self.settings.selected_file.is_some() && !saving && !self.closing)
                    .tooltip("Reload from disk")
                    .icon(Icon::Reload, |view, _, cx| view.reload_editor(cx)),
                ControlSpec::new("editor-save", &self.controls, cx)
                    .enabled(editing && dirty && !saving && !self.closing)
                    .emphasis(if dirty {
                        Emphasis::Primary
                    } else {
                        Emphasis::Normal
                    })
                    .tooltip(if saving { "Saving…" } else { "Save (Ctrl+S)" })
                    .icon(Icon::Save, |view, _, cx| view.save_editor(cx)),
            ])
            .collect()
    }

    pub fn code_pane(&self, cx: &Context<Self>) -> AnyElement {
        let Some(path) = self.settings.selected_file.as_ref() else {
            return empty_hint(
                "Select a file in Files, or open one an agent changed, to read and edit it here.",
            );
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
            .flex_shrink_0()
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
                    .flex_shrink_0()
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
        // The tab keeps its name; in a narrow column the view switch wraps onto its own line.
        let path_bar = theme::row()
            .flex_wrap()
            .min_h(theme::scaled(TAB_HEIGHT))
            .flex_shrink_0()
            .pr_1()
            .gap_x_2()
            .bg(rgb(colors.panel))
            .border_b_1()
            .border_color(rgb(colors.border))
            .child(tab)
            .child(div().flex_1())
            .child(
                theme::row()
                    .h(theme::scaled(TAB_HEIGHT))
                    .flex_shrink_0()
                    .child(self.viewer_modes(cx)),
            );
        theme::column()
            .flex_1()
            .child(path_bar)
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
            return empty_hint("No changes in this view.");
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
