// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:46:34.529301
//+ Revised: 	2026/11/13 07:53:51.782689
// -----------------------------------------------------------------------------

//! The Files column: lazy folder navigation for the open project.

use super::{
    assets::Icon,
    controls::{ControlSpec, icon, measured},
    file_icons::{FolderKind, file_icon, folder_icon, folder_kind_for},
    layout::primary_action,
    theme,
    workspace::Workspace,
};
use gpui::{AnyElement, Context, MouseButton, div, prelude::*, rgb};
use std::path::Path;

const MAX_ENTRIES: usize = 1000;
const MAX_DEPTH: usize = 24;
const INDENT: f32 = 12.0;

impl Workspace {
    pub fn explorer_actions(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        vec![
            ControlSpec::new("refresh-project", &self.controls, cx)
                .enabled(self.project.is_some() && !self.closing && !self.loading)
                .tooltip("Refresh (Ctrl+Shift+R)")
                .icon(Icon::Refresh, |view, _, cx| view.refresh_project(cx)),
        ]
    }

    pub fn explorer(&self, cx: &Context<Self>) -> AnyElement {
        if self.project.is_none() {
            return theme::column()
                .child(primary_action(
                    ControlSpec::new("explorer-open-project", &self.controls, cx)
                        .enabled(self.can_open_project(cx)),
                    Icon::OpenFolder,
                    "Open project",
                    |view, window, cx| view.pick_project(window, cx),
                ))
                .child(super::layout::empty_hint(
                    "Choose the folder your agents will work in.",
                ))
                .into_any_element();
        }
        theme::column()
            .id("project-tree")
            .flex_1()
            .overflow_y_scroll()
            .py_1()
            .children(self.tree_rows(Path::new(""), 0, cx))
            .into_any_element()
    }

    fn tree_rows(&self, parent: &Path, depth: usize, cx: &Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        let Some(entries) = self.entries.get(parent) else {
            return rows;
        };
        for entry in entries.iter().take(MAX_ENTRIES) {
            let path = entry.relative_path.clone();
            let path_id = path.to_string_lossy().replace('\\', "/");
            let id = format!("{}:{path_id}", if entry.is_dir { "folder" } else { "file" });
            let open = self.settings.expanded_folders.contains(&path);
            let is_dir = entry.is_dir;
            let selected = self.settings.selected_file.as_ref() == Some(&path);
            // Like VS Code: Git changes colour the name and add a letter (a dot on folders).
            let change = self.git.as_ref().and_then(|git| {
                if is_dir {
                    git.folder(&path)
                } else {
                    git.files.get(&path).copied()
                }
            });
            let change_color = change.map(|change| theme::colors().git.color(change));
            // Ignored paths are greyed like VS Code's `target`; a change colour always wins.
            let ignored =
                change.is_none() && self.git.as_ref().is_some_and(|git| git.is_ignored(&path));
            // Without Git to say what is ignored, well-known build output fades by name.
            let output =
                is_dir && self.git.is_none() && folder_kind_for(&entry.name) == FolderKind::Output;
            let (chevron, (glyph, color)) = if is_dir {
                let chevron = if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                };
                (Some(chevron), folder_icon(&entry.name, open))
            } else {
                (None, file_icon(&entry.name))
            };
            rows.push(
                div()
                    .id(gpui::SharedString::from(id.clone()))
                    .relative()
                    .h(theme::scaled(theme::ROW_HEIGHT))
                    .pl(theme::scaled(8. + depth as f32 * INDENT))
                    .pr_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink_0()
                    .when(selected, |view| view.bg(rgb(theme::colors().active)))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(theme::colors().hover)))
                    .child(match chevron {
                        Some(chevron) => icon(chevron, theme::colors().faint).into_any_element(),
                        None => div()
                            .w(theme::scaled(theme::ICON_SIZE))
                            .flex_shrink_0()
                            .into_any_element(),
                    })
                    .child(icon(glyph, color))
                    .child(
                        theme::clipped(entry.name.clone()).text_color(rgb(match change_color {
                            Some(color) => color,
                            None if ignored => theme::colors().git.ignored,
                            None if selected => theme::colors().text,
                            None if output => theme::colors().faint,
                            None => theme::colors().soft_text,
                        })),
                    )
                    .children(change.zip(change_color).map(|(change, color)| {
                        div()
                            .id(gpui::SharedString::from(format!("git-{path_id}")))
                            .relative()
                            .flex_shrink_0()
                            .pl_1()
                            .text_size(theme::scaled(theme::SMALL_TEXT))
                            .text_color(rgb(color))
                            .child(if is_dir { "•" } else { change.letter() })
                            .child(measured(format!("git-{path_id}"), true, &self.controls))
                    }))
                    .child(measured(id, !self.closing, &self.controls))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| {
                            if is_dir {
                                view.toggle_folder(path.clone(), cx);
                            } else {
                                view.select_file(path.clone(), cx);
                            }
                        }),
                    )
                    .into_any_element(),
            );
            if is_dir && open {
                if depth < MAX_DEPTH {
                    rows.extend(self.tree_rows(&entry.relative_path, depth + 1, cx));
                } else {
                    // An open folder that shows nothing would look empty; say why instead.
                    rows.push(super::layout::empty_hint(
                        "Folders this deep aren't shown here. Open a parent folder as the project to see them.",
                    ));
                }
            }
        }
        if entries.len() > MAX_ENTRIES {
            rows.push(super::layout::empty_hint(
                "Showing the first 1,000 entries in this folder.",
            ));
        }
        rows
    }
}
