// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:27:01.652278
//+ Revised: 	2026/11/13 07:53:51.584496
// -----------------------------------------------------------------------------

//! File and diff presentation requests with stale-response protection.

use super::{events::Event, workspace::Workspace};
use crate::{
    project::ProjectError,
    settings::{Column, ViewerMode},
};
use gpui::Context;
use std::path::PathBuf;

/// Shown whether a save or the file watcher notices the external change first, so the
/// developer always reads the same explanation.
pub(super) const EXTERNAL_CHANGE: &str = "This file changed outside the editor. Your unsaved edits are preserved; reload to review the disk version before saving.";

impl Workspace {
    pub fn select_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.editor_busy(cx) {
            self.error = Some(
                "Save the edited file or reload it to discard changes before opening another file."
                    .into(),
            );
            cx.notify();
            return;
        }
        self.settings.selected_file = Some(path);
        self.settings.viewer_mode = ViewerMode::File;
        self.expand_column(Column::Code);
        self.settings.editor_cursor = 0;
        self.editor = None;
        self.load_selected_file(cx);
        self.changed(cx);
    }

    /// Close the open file, as closing its editor tab does; refused while it has unsaved edits.
    pub fn close_file(&mut self, cx: &mut Context<Self>) {
        if self.editor_busy(cx) {
            self.error = Some(
                "Save the edited file or reload it to discard changes before closing it.".into(),
            );
            cx.notify();
            return;
        }
        if self.settings.selected_file.take().is_some() {
            self.file_generation += 1;
            self.document_generation += 1;
            self.editor = None;
            self.settings.editor_cursor = 0;
            self.settings.viewer_mode = ViewerMode::File;
            tracing::info!(target: "orin::desktop", "file closed");
            self.changed(cx);
        }
    }

    /// True when the selected file is Markdown and is shown rendered rather than as source.
    pub fn previewing_markdown(&self) -> bool {
        self.settings.markdown_preview && self.can_preview_markdown()
    }

    /// The preview applies to the file itself (not its diffs) when it is a Markdown file.
    pub fn can_preview_markdown(&self) -> bool {
        self.settings.viewer_mode == ViewerMode::File
            && self
                .settings
                .selected_file
                .as_deref()
                .is_some_and(super::markdown::is_markdown)
    }

    pub fn toggle_markdown_preview(&mut self, cx: &mut Context<Self>) {
        if !self.can_preview_markdown() || self.closing {
            return;
        }
        self.settings.markdown_preview = !self.settings.markdown_preview;
        self.changed(cx);
    }

    pub fn set_viewer_mode(&mut self, mode: ViewerMode, cx: &mut Context<Self>) {
        if self.settings.viewer_mode == mode {
            return;
        }
        self.settings.viewer_mode = mode;
        self.changed(cx);
        if mode == ViewerMode::File && self.editor.is_some() {
            cx.notify();
            return;
        }
        self.load_selected_file(cx);
        cx.notify();
    }

    pub fn load_selected_file(&mut self, cx: &mut Context<Self>) {
        if self.settings.viewer_mode == ViewerMode::File && self.editor_busy(cx) {
            return;
        }
        let (Some(project), Some(path)) =
            (self.project.clone(), self.settings.selected_file.clone())
        else {
            return;
        };
        let generation = self.file_generation + 1;
        let sender = self.sender.clone();
        let mode = self.settings.viewer_mode;
        if self.submit(move || {
            let event = if mode == ViewerMode::File {
                Event::File {
                    generation,
                    result: project.read_file(&path),
                }
            } else {
                Event::Diff {
                    generation,
                    result: project.git_diff(&path, mode == ViewerMode::Staged),
                }
            };
            if sender.send(event).is_err() {
                tracing::debug!("Workspace closed during file load");
            }
        }) {
            self.file_generation = generation;
            self.content = "Loading…".into();
            cx.notify();
        }
    }

    pub fn show_project_error(&mut self, error: ProjectError) {
        tracing::warn!(%error, "Project operation could not complete");
        self.error = Some(match error {
            ProjectError::TooLarge => "This file is larger than the 1 MiB viewing limit. Open a smaller file.",
            ProjectError::Binary => "This is a binary file. The code viewer shows text files.",
            ProjectError::Encoding => "This file is not UTF-8 text and cannot be displayed here.",
            ProjectError::InvalidPath | ProjectError::OutsideRoot => "This path is outside the open project. Choose a file inside the project folder.",
            ProjectError::NotDirectory => "Choose a project folder rather than a file.",
            ProjectError::Git(_) => "The Git diff could not be read. Check that this folder is a Git repository and Git is installed.",
            ProjectError::Conflict => EXTERNAL_CHANGE,
            ProjectError::Io { .. } => "This file or folder could not be read. Check that it still exists and is accessible, then refresh.",
        }.into());
    }

    pub fn save_editor(&mut self, cx: &mut Context<Self>) {
        let (Some(project), Some(editor)) = (self.project.clone(), self.editor.clone()) else {
            return;
        };
        let view = editor.read(cx);
        if view.saving || !view.dirty() {
            return;
        }
        let Some(document) = view.document() else {
            return;
        };
        let original = document.original().clone();
        let text = document.buffer().text().to_owned();
        let sender = self.sender.clone();
        let generation = self.document_generation;
        if self.submit(move || {
            let result = project.save_file(&original, &text);
            if sender
                .send(Event::FileSaved { generation, result })
                .is_err()
            {
                tracing::debug!("Workspace closed during file save");
            }
        }) {
            editor.update(cx, |editor, cx| {
                editor.saving = true;
                cx.notify();
            });
        }
    }

    pub fn reload_editor(&mut self, cx: &mut Context<Self>) {
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).saving)
        {
            return;
        }
        self.editor = None;
        self.settings.viewer_mode = ViewerMode::File;
        self.error = None;
        self.load_selected_file(cx);
    }
}
