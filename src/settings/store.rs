// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:12:37.003729
//+ Revised: 	2026/11/12 15:01:44.490245
// -----------------------------------------------------------------------------

//! Atomic preference replacement; invalid existing files are never reset implicitly.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use tempfile::NamedTempFile;

use super::{SCHEMA_VERSION, SettingsError, WorkspaceSettings};
use serde_json::Value;

const MAX_SETTINGS_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn default_path() -> Result<PathBuf, SettingsError> {
        let directories =
            ProjectDirs::from("com", "Breren", "Orin").ok_or(SettingsError::NoConfigDirectory)?;
        Ok(directories.config_dir().join("workspace.json"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<WorkspaceSettings, SettingsError> {
        let settings = match self.read_json()? {
            Some(value) => WorkspaceSettings::from_json(value)?,
            None => return Ok(WorkspaceSettings::default()),
        };
        tracing::debug!(target: "orin::settings", "workspace settings restored");
        Ok(settings)
    }

    /// The stored JSON as written, or `None` when there is no settings file yet.
    fn read_json(&self) -> Result<Option<Value>, SettingsError> {
        let file = match crate::sharing::retry(|| File::open(&self.path)) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_SETTINGS_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_SETTINGS_BYTES {
            return Err(SettingsError::Invalid("settings file exceeds 1 MiB"));
        }
        Ok(Some(serde_json::from_slice(&bytes)?))
    }

    pub fn save(&self, settings: &WorkspaceSettings) -> Result<(), SettingsError> {
        settings.validate()?;
        // Skip identical writes, but only for a current-schema file: an older file that migrates
        // to the same settings must still be rewritten, or it would be upgraded on every start.
        if let Some(stored) = self.read_json()? {
            let current = stored.get("schema_version") == Some(&Value::from(SCHEMA_VERSION));
            if WorkspaceSettings::from_json(stored)? == *settings && current {
                return Ok(());
            }
        }
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temporary = NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, settings)?;
        temporary.write_all(b"\n")?;
        // Use the same byte limit as the loader, including JSON escaping and formatting.
        // Reject before replacement so the last readable workspace survives.
        if temporary.as_file().metadata()?.len() > MAX_SETTINGS_BYTES {
            return Err(SettingsError::Invalid("settings file exceeds 1 MiB"));
        }
        temporary.as_file().sync_all()?;
        // A failed attempt hands the temporary file back; it is deleted if every attempt fails.
        let mut pending = Some(temporary);
        crate::sharing::retry(|| {
            let file = pending
                .take()
                .expect("temporary file is returned by each failed attempt");
            file.persist(&self.path).map(drop).map_err(|error| {
                pending = Some(error.file);
                error.error
            })
        })?;
        sync_directory(parent)?;
        tracing::debug!(target: "orin::settings", "workspace settings saved");
        Ok(())
    }
}

fn sync_directory(path: &Path) -> Result<(), SettingsError> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
