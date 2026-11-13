// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 18:03:51.335583
//+ Revised: 	2026/11/12 13:59:46.935212
// -----------------------------------------------------------------------------

//! Coalesced native file notifications keep code and changes fresh without polling disk.

use super::{events::Event, workspace::Workspace};
use crate::project::{FileContent, ProjectError};
use gpui::Context;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

impl Workspace {
    pub fn poll_watch(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if let Some(watch) = &self.watch {
            match watch.take_changed() {
                Ok(true) => self.watch_pending = Some(Instant::now()),
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(%error,"File notifications failed");
                    self.error=Some("Automatic refresh could not read a folder change. Use Refresh to try again.".into());
                    cx.notify();
                }
            }
        }
        if self
            .watch_pending
            .is_none_or(|time| time.elapsed() < Duration::from_millis(250))
            || self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.read(cx).saving)
        {
            return;
        }
        self.watch_pending = None;
        let Some(project) = self.project.clone() else {
            return;
        };
        for path in std::iter::once(PathBuf::new()).chain(self.settings.expanded_folders.clone()) {
            self.load_directory(path, project.clone(), cx);
        }
        self.load_git_status(project.clone());
        if let Some(path) = self.settings.selected_file.clone() {
            let generation = self.document_generation;
            let sender = self.sender.clone();
            self.submit(move || {
                let result = project.read_file(&path);
                if sender
                    .send(Event::FileObserved { generation, result })
                    .is_err()
                {
                    tracing::debug!("Workspace closed during file observation");
                }
            });
        }
        if self.settings.viewer_mode != crate::settings::ViewerMode::File {
            self.load_selected_file(cx);
        }
    }
    pub fn observed_file(
        &mut self,
        result: Result<FileContent, ProjectError>,
        cx: &mut Context<Self>,
    ) {
        let file = match result {
            Ok(file) => file,
            Err(error) => {
                self.show_project_error(error);
                cx.notify();
                return;
            }
        };
        let Some(editor) = &self.editor else {
            return;
        };
        let view = editor.read(cx);
        let changed = view
            .document()
            .is_some_and(|document| document.original().version != file.version);
        if !changed {
            return;
        }
        if view.dirty() {
            editor.update(cx, |editor, cx| {
                editor.conflict = true;
                cx.notify();
            });
            self.error = Some(super::file_actions::EXTERNAL_CHANGE.into());
        } else if !view.saving {
            self.editor = None;
            if self.settings.viewer_mode == crate::settings::ViewerMode::File {
                self.load_selected_file(cx);
            }
        }
        cx.notify();
    }
}
