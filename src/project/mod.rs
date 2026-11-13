// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:12:36.363777
//+ Revised: 	2026/11/12 13:59:47.937037
// -----------------------------------------------------------------------------

//! Local project access confined to a canonical workspace root.

mod files;
mod git;
mod saving;
mod version;
mod watch;

use std::path::{Component, Path, PathBuf};

pub use files::{Entry, FileContent, MAX_FILE_BYTES};
pub use git::{FileStatus, GitStatus};
pub use version::FileVersion;
pub use watch::ProjectWatch;

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("Project IO failed for {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("A project path must be relative and cannot traverse parent directories")]
    InvalidPath,
    #[error("The requested path escapes the project root")]
    OutsideRoot,
    #[error("The project root is not a directory")]
    NotDirectory,
    #[error("This file exceeds the {MAX_FILE_BYTES}-byte viewing limit")]
    TooLarge,
    #[error("This file contains binary data")]
    Binary,
    #[error("This file is not valid UTF-8")]
    Encoding,
    #[error("This document changed on disk. Review or reload the latest version before saving")]
    Conflict,
    #[error("Git failed: {0}")]
    Git(String),
}

pub(crate) fn io_error(path: &Path, source: std::io::Error) -> ProjectError {
    ProjectError::Io {
        path: path.to_owned(),
        source,
    }
}

#[derive(Clone, Debug)]
pub struct Project {
    root: PathBuf,
}

impl Project {
    pub fn open(path: &Path) -> Result<Self, ProjectError> {
        let root = path.canonicalize().map_err(|error| io_error(path, error))?;
        if !root.is_dir() {
            return Err(ProjectError::NotDirectory);
        }
        tracing::info!(target: "orin::project", "project opened");
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Normalizes an agent-reported file to its canonical project-relative path.
    /// Performs filesystem IO and must run on a background worker.
    pub fn reported_file(&self, reported: &Path) -> Result<PathBuf, ProjectError> {
        let path = if reported.is_absolute() {
            reported.to_owned()
        } else {
            self.root.join(self.relative_path(reported)?)
        };
        let canonical = path
            .canonicalize()
            .map_err(|error| io_error(&path, error))?;
        let relative = canonical
            .strip_prefix(&self.root)
            .map_err(|_| ProjectError::OutsideRoot)?;
        let metadata = std::fs::metadata(&canonical).map_err(|error| io_error(&path, error))?;
        if !metadata.is_file() {
            return Err(io_error(
                &path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "The reported path is a folder, not a file",
                ),
            ));
        }
        Ok(relative.to_owned())
    }

    fn relative_path(&self, relative: &Path) -> Result<PathBuf, ProjectError> {
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            return Err(ProjectError::InvalidPath);
        }
        Ok(relative
            .components()
            .filter_map(|part| match part {
                Component::Normal(value) => Some(value),
                _ => None,
            })
            .collect())
    }

    fn resolve(&self, relative: &Path) -> Result<PathBuf, ProjectError> {
        let path = self.root.join(self.relative_path(relative)?);
        let canonical = path
            .canonicalize()
            .map_err(|error| io_error(&path, error))?;
        if !canonical.starts_with(&self.root) {
            return Err(ProjectError::OutsideRoot);
        }
        Ok(canonical)
    }

    fn resolve_missing(&self, relative: &Path) -> Result<PathBuf, ProjectError> {
        let relative = self.relative_path(relative)?;
        let full = self.root.join(&relative);
        let mut ancestor = full.as_path();
        while !ancestor
            .try_exists()
            .map_err(|error| io_error(ancestor, error))?
        {
            ancestor = ancestor.parent().ok_or(ProjectError::OutsideRoot)?;
        }
        let canonical = ancestor
            .canonicalize()
            .map_err(|error| io_error(ancestor, error))?;
        if !canonical.starts_with(&self.root) {
            return Err(ProjectError::OutsideRoot);
        }
        Ok(relative)
    }
}
