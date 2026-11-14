// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:26:59.738569
//+ Revised: 	2026/11/13 17:30:07.679171
// -----------------------------------------------------------------------------

//! Messages crossing from bounded IO workers to the GPUI thread.

use crate::{
    agents::{AgentSession, Provider, ProviderStatus, accounts::Account},
    project::{Entry, FileContent, Project, ProjectError},
    terminal::Session,
};
use std::path::PathBuf;

/// An opened project, its root listing, and its file watcher (which may fail independently).
pub type ProjectLoad = (
    Project,
    Vec<Entry>,
    Result<crate::project::ProjectWatch, ProjectError>,
);

pub enum Event {
    ProjectClosed {
        generation: u64,
        result: std::io::Result<()>,
    },
    Project {
        generation: u64,
        result: Result<ProjectLoad, ProjectError>,
    },
    Directory {
        generation: u64,
        path: PathBuf,
        result: Result<Vec<Entry>, ProjectError>,
    },
    File {
        generation: u64,
        result: Result<FileContent, ProjectError>,
    },
    Diff {
        generation: u64,
        result: Result<String, ProjectError>,
    },
    Providers {
        path: Option<std::ffi::OsString>,
        providers: Vec<(Provider, ProviderStatus)>,
    },
    Accounts {
        path: Option<std::ffi::OsString>,
        accounts: Vec<Account>,
    },
    Agent {
        generation: u64,
        provider: Provider,
        result: anyhow::Result<AgentSession>,
    },
    Shell {
        generation: u64,
        result: anyhow::Result<Session>,
    },
    FileSaved {
        generation: u64,
        result: Result<FileContent, ProjectError>,
    },
    FileObserved {
        generation: u64,
        result: Result<FileContent, ProjectError>,
    },
    ReportedFile {
        generation: u64,
        result: Result<PathBuf, ProjectError>,
    },
    /// The project's branch and changed files; `None` outside a Git repository.
    Git {
        generation: u64,
        result: Result<Option<crate::project::GitStatus>, ProjectError>,
    },
    /// A finished update check; `manual` when the user pressed Check now.
    UpdateChecked {
        manual: bool,
        status: crate::update::check::UpdateStatus,
        at: u64,
    },
    /// Bytes of the update downloaded so far, and its declared size.
    UpdateProgress {
        read: u64,
        total: Option<u64>,
    },
    /// The update was verified and installed (its version and the file to start), or why not.
    UpdateInstalled(Result<(semver::Version, PathBuf), crate::update::release::UpdateError>),
    Stopped(Result<(), String>),
    Saved(Result<(), String>),
    /// The settings file read again by Retry saving, after it couldn't be read at start.
    SettingsReread(Result<crate::settings::WorkspaceSettings, String>),
}
