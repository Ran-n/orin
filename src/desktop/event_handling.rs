// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:28:14.881191
//+ Revised: 	2026/11/13 08:33:02.602556
// -----------------------------------------------------------------------------

//! Apply completed operations on the GPUI thread, rejecting stale file results.

use super::{events::Event, workspace::Workspace};
use gpui::{AppContext, Context, Window};
use std::path::PathBuf;

impl Workspace {
    pub fn apply_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            Event::Project { generation, result } if generation == self.project_generation => {
                self.loading = false;
                match result {
                    // Edits made while another folder was loading must not be dropped.
                    Ok((project, _, watch))
                        if self.editor_busy(cx)
                            && self
                                .project
                                .as_ref()
                                .is_some_and(|open| open.root() != project.root()) =>
                    {
                        tracing::warn!(target: "orin::project", "Project switch refused: the open file has unsaved edits");
                        self.submit(move || drop(watch));
                        // The open folder's in-flight results carried the old generation and are
                        // now ignored, so read it again.
                        self.refresh_project(cx);
                        self.error = Some(
                            "Save or reload the edited file, then open the other folder again."
                                .into(),
                        );
                    }
                    Ok((project, entries, watch)) => {
                        if let Some(old) = self.watch.take() {
                            self.submit(move || drop(old));
                        }
                        match watch {
                            Ok(watch) => self.watch = Some(watch),
                            Err(error) => {
                                tracing::warn!(%error,"Folder watcher could not start");
                                self.error=Some("Automatic file refresh is unavailable. Use Refresh to see changes.".into());
                            }
                        }
                        self.project_ready(project, entries, cx)
                    }
                    Err(error) => {
                        if self.project.is_none() {
                            self.settings.page = Some(crate::settings::Page::Welcome);
                        }
                        self.show_project_error(error)
                    }
                }
            }
            Event::Git { generation, result } if generation == self.project_generation => {
                match result {
                    Ok(status) => {
                        self.git = status;
                        self.git_failed = false;
                    }
                    Err(error) => {
                        // Decorations are optional: the status bar says Git is unavailable.
                        tracing::warn!(target: "orin::project", %error, "Git status unavailable");
                        self.git = None;
                        self.git_failed = true;
                    }
                }
            }
            Event::Directory {
                generation,
                path,
                result,
            } if generation == self.project_generation => match result {
                Ok(entries) => {
                    self.entries.insert(path, entries);
                }
                Err(error) => self.show_project_error(error),
            },
            Event::File { generation, result } if generation == self.file_generation => {
                match result {
                    Ok(file) => {
                        let controls = self.controls.clone();
                        let editor =
                            cx.new(|cx| super::editor::TextEditor::file(file, controls, cx));
                        self.document_generation += 1;
                        let offset = self.settings.editor_cursor;
                        editor.update(cx, |editor, cx| {
                            let text = editor.buffer().text();
                            let offset = crate::editor::utf16_to_byte(
                                text,
                                crate::editor::byte_to_utf16(text, offset.min(text.len())),
                            );
                            editor.select(offset..offset, cx);
                        });
                        cx.observe(&editor, |view, editor, cx| {
                            let cursor = editor.read(cx).buffer().selection().end;
                            if view.settings.editor_cursor != cursor {
                                view.settings.editor_cursor = cursor;
                                view.changed(cx);
                            } else {
                                cx.notify();
                            }
                        })
                        .detach();
                        cx.subscribe(&editor, |view, _, action, cx| {
                            if matches!(action, super::editor::EditorAction::Save) {
                                view.save_editor(cx);
                            }
                        })
                        .detach();
                        self.editor = Some(editor);
                        self.content.clear();
                    }
                    Err(error) => {
                        self.content.clear();
                        self.show_project_error(error);
                    }
                }
            }
            Event::Diff { generation, result } if generation == self.file_generation => {
                match result {
                    Ok(diff) => {
                        self.content = if diff.is_empty() {
                            "No changes in this view.".into()
                        } else {
                            diff
                        }
                    }
                    Err(error) => {
                        self.content.clear();
                        self.show_project_error(error);
                    }
                }
            }
            Event::Providers(providers) => {
                self.providers = providers;
                if let Some(path) = self.test_agent.clone() {
                    for (provider, status) in &mut self.providers {
                        status.available = *provider == crate::agents::Provider::Claude;
                        status.executable =
                            (*provider == crate::agents::Provider::Claude).then(|| path.clone());
                    }
                }
            }
            Event::Agent {
                generation,
                provider,
                result,
            } => self.agent_ready(generation, provider, result, window, cx),
            Event::Shell { generation, result } => self.shell_ready(generation, result, window, cx),
            Event::Accounts(accounts) => self.accounts_checked(accounts, cx),
            Event::FileSaved { generation, result } if generation == self.document_generation => {
                match result {
                    Ok(file) => {
                        if let Some(editor) = &self.editor {
                            editor.update(cx, |editor, cx| editor.accept_saved(file, cx));
                        }
                    }
                    Err(error) => {
                        if let Some(editor) = &self.editor {
                            editor.update(cx, |editor, cx| {
                                editor.saving = false;
                                editor.conflict =
                                    matches!(error, crate::project::ProjectError::Conflict);
                                cx.notify();
                            });
                        }
                        self.show_project_error(error);
                    }
                }
            }
            Event::ReportedFile { generation, result } if generation == self.project_generation => {
                match result {
                    Ok(path) => self.select_file(path, cx),
                    Err(error) => self.show_project_error(error),
                }
            }
            Event::FileObserved { generation, result }
                if generation == self.document_generation =>
            {
                self.observed_file(result, cx)
            }
            Event::UpdateChecked { manual, status, at } => {
                self.update_checked(manual, status, at, cx)
            }
            Event::UpdateProgress { read, total } => self.update_progress(read, total, cx),
            Event::UpdateInstalled(result) => self.update_installed(result, window, cx),
            Event::Stopped(Err(message)) => self.error = Some(message),
            Event::Stopped(Ok(())) => {}
            Event::Saved(result) => {
                self.save_inflight = false;
                if let Err(message) = result {
                    self.error = Some(message);
                    self.writable = false;
                }
            }
            Event::SettingsReread(result) => self.settings_reread(result, cx),
            _ => {}
        }
        cx.notify();
    }

    fn project_ready(
        &mut self,
        project: crate::project::Project,
        entries: Vec<crate::project::Entry>,
        cx: &mut Context<Self>,
    ) {
        let root = project.root().to_owned();
        if self.settings.project.as_ref() != Some(&root) {
            self.settings.selected_file = None;
            self.settings.expanded_folders.clear();
            self.settings.editor_cursor = 0;
            self.settings.viewer_mode = crate::settings::ViewerMode::File;
        }
        // Reopening the same folder keeps a file with unsaved edits open as it is.
        if !self.editor_busy(cx) {
            self.file_generation += 1;
            self.document_generation += 1;
            self.content.clear();
            self.editor = None;
        }
        self.settings
            .remember_project(&crate::process::launch::native_path(&root));
        if self.settings.page == Some(crate::settings::Page::Welcome) {
            self.settings.page = None;
        }
        self.settings.project = Some(root);
        self.project = Some(project.clone());
        self.entries.clear();
        self.entries.insert(PathBuf::new(), entries);
        self.git = None;
        self.git_failed = false;
        self.load_git_status(project.clone());
        for path in self.settings.expanded_folders.clone() {
            self.load_directory(path, project.clone(), cx);
        }
        if self.settings.selected_file.is_some() {
            self.load_selected_file(cx);
        }
        self.autostart_shell(cx);
        self.changed(cx);
    }
}
