// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:27:01.491840
//+ Revised: 	2026/11/13 07:53:51.647504
// -----------------------------------------------------------------------------

//! Project selection and lazy directory expansion run away from the UI thread.

use super::{events::Event, workspace::Workspace};
use crate::project::Project;
use gpui::{App, Context, PathPromptOptions, Window};
use std::path::{Path, PathBuf};

impl Workspace {
    pub fn pick_project(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // The shortcut works everywhere; don't ask for a folder that would then be refused.
        if let Some(reason) = self.project_switch_blocked(cx) {
            self.error = Some(reason.into());
            cx.notify();
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open project".into()),
        });
        cx.spawn(async move |view, cx| {
            let outcome = prompt.await;
            let delivered = view.update(cx, |view, cx| match outcome {
                Ok(Ok(Some(paths))) => { if let Some(path) = paths.into_iter().next() { view.open_project(path, cx); } }
                Ok(Ok(None)) => {}
                result => { tracing::warn!(?result, "Project picker failed"); view.error = Some("The folder picker could not open. Try again or start Orin with --project PATH.".into()); cx.notify(); }
            });
            if let Err(error) = delivered {
                tracing::debug!(target: "orin::project", %error, "Workspace closed before the folder picker answered");
            }
        }).detach();
    }

    /// Why another folder can't be opened now, if it can't. Every Open control is disabled with
    /// this as its tooltip, and `open_project` refuses with it, so the two never disagree.
    pub fn project_switch_blocked(&self, cx: &App) -> Option<&'static str> {
        if self.closing {
            Some("Orin is closing.")
        } else if self.editor_busy(cx) {
            Some("Save or reload the edited file before changing projects.")
        } else if self.launching
            || self.agents.iter().any(|tab| tab.view.read(cx).running())
            || self
                .shell
                .as_ref()
                .is_some_and(|tab| tab.view.read(cx).running())
        {
            Some("Stop the running sessions before changing project folders.")
        } else {
            None
        }
    }

    /// Open controls are live when no folder is loading and nothing blocks a switch.
    pub fn can_open_project(&self, cx: &App) -> bool {
        !self.loading && self.project_switch_blocked(cx).is_none()
    }

    pub fn open_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(reason) = self.project_switch_blocked(cx) {
            tracing::info!(target: "orin::project", reason, "Project open refused");
            self.error = Some(reason.into());
            cx.notify();
            return;
        }
        let generation = self.project_generation + 1;
        let sender = self.sender.clone();
        if self.submit(move || {
            let result = Project::open(&path).and_then(|project| {
                project.list_dir(Path::new("")).map(|entries| {
                    let watch = crate::project::ProjectWatch::new(project.root());
                    (project, entries, watch)
                })
            });
            if sender.send(Event::Project { generation, result }).is_err() {
                tracing::debug!("Workspace closed during project open");
            }
        }) {
            self.project_generation = generation;
            self.loading = true;
            self.error = None;
            cx.notify();
        }
    }

    pub fn refresh_project(&mut self, cx: &mut Context<Self>) {
        if let Some(project) = self.project.clone() {
            self.load_directory(PathBuf::new(), project.clone(), cx);
            self.load_git_status(project);
        }
        let sender = self.sender.clone();
        self.submit(move || {
            let providers = super::providers::probe_all();
            if sender.send(Event::Providers(providers)).is_err() {
                tracing::debug!("Workspace closed during provider refresh");
            }
        });
        let paths = self.settings.expanded_folders.clone();
        for path in paths {
            if let Some(project) = self.project.clone() {
                self.load_directory(path, project, cx);
            }
        }
        if self.settings.selected_file.is_some() {
            self.load_selected_file(cx);
        }
    }

    pub fn toggle_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.settings.expanded_folders.contains(&path) {
            self.settings
                .expanded_folders
                .retain(|folder| *folder != path);
        } else {
            self.settings.expanded_folders.push(path.clone());
            if let Some(project) = self.project.clone() {
                self.load_directory(path, project, cx);
            }
        }
        self.changed(cx);
    }

    /// Read the branch and changed files in the background.
    pub fn load_git_status(&mut self, project: Project) {
        let sender = self.sender.clone();
        let generation = self.project_generation;
        self.submit(move || {
            let result = project.git_status();
            if sender.send(Event::Git { generation, result }).is_err() {
                tracing::debug!("Workspace closed during Git status");
            }
        });
    }

    pub fn load_directory(&mut self, path: PathBuf, project: Project, cx: &mut Context<Self>) {
        let sender = self.sender.clone();
        let generation = self.project_generation;
        self.submit(move || {
            let result = project.list_dir(&path);
            if sender
                .send(Event::Directory {
                    generation,
                    path,
                    result,
                })
                .is_err()
            {
                tracing::debug!("Workspace closed during directory load");
            }
        });
        cx.notify();
    }
}
