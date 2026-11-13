// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:18.594815
//+ Revised: 	2026/11/12 09:16:37.416135
// -----------------------------------------------------------------------------

//! Optimistic saves validate the original immediately before atomic replacement.
//!
//! External writers need not honor application locks. A writer racing exactly between
//! final validation and rename cannot be made transactional by portable filesystem APIs.

use super::{FileContent, FileVersion, MAX_FILE_BYTES, Project, ProjectError, io_error};
use std::{fs, io::Write, path::Path};
use tempfile::NamedTempFile;

impl Project {
    /// Replaces an existing document only if its original identity and bytes still match.
    /// Callers retain their buffer and offer reload/review when this returns Conflict.
    pub fn save_file(
        &self,
        original: &FileContent,
        replacement: &str,
    ) -> Result<FileContent, ProjectError> {
        validate_replacement(replacement)?;
        self.verify_original(original)?;
        let path = self.resolve(&original.relative_path)?;
        let parent = path.parent().ok_or(ProjectError::InvalidPath)?;
        let temporary = prepare_replacement(&path, replacement)?;
        self.verify_original(original)?;
        let saved = persist(temporary, &path)?;
        let version = FileVersion::capture(&path, &saved)?;
        sync_directory(parent)?;
        tracing::info!(target: "orin::project", byte_len = replacement.len(), "document saved");
        Ok(FileContent {
            relative_path: original.relative_path.clone(),
            text: replacement.into(),
            byte_len: replacement.len(),
            version,
        })
    }

    fn verify_original(&self, original: &FileContent) -> Result<(), ProjectError> {
        let current = match self.read_file(&original.relative_path) {
            Err(ProjectError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                return Err(ProjectError::Conflict);
            }
            Err(
                ProjectError::OutsideRoot
                | ProjectError::Binary
                | ProjectError::Encoding
                | ProjectError::TooLarge,
            ) => return Err(ProjectError::Conflict),
            result => result?,
        };
        if current.text != original.text || current.version != original.version {
            tracing::warn!(target: "orin::project", "document save refused because the disk version changed");
            return Err(ProjectError::Conflict);
        }
        Ok(())
    }
}

fn prepare_replacement(path: &Path, replacement: &str) -> Result<NamedTempFile, ProjectError> {
    let parent = path.parent().ok_or(ProjectError::InvalidPath)?;
    let metadata = fs::metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.permissions().readonly() {
        return Err(io_error(path, std::io::ErrorKind::PermissionDenied.into()));
    }
    let mut temporary = NamedTempFile::new_in(parent).map_err(|error| io_error(parent, error))?;
    temporary
        .write_all(replacement.as_bytes())
        .map_err(|error| io_error(path, error))?;
    temporary
        .as_file()
        .set_permissions(metadata.permissions())
        .map_err(|error| io_error(path, error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| io_error(path, error))?;
    Ok(temporary)
}

fn validate_replacement(replacement: &str) -> Result<(), ProjectError> {
    if replacement.len() > MAX_FILE_BYTES {
        return Err(ProjectError::TooLarge);
    }
    if replacement.contains('\0') {
        return Err(ProjectError::Binary);
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), ProjectError> {
    #[cfg(unix)]
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| io_error(path, error))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(not(windows))]
fn persist(temporary: NamedTempFile, path: &Path) -> Result<fs::File, ProjectError> {
    temporary
        .persist(path)
        .map_err(|error| io_error(path, error.error))
}

#[cfg(windows)]
fn persist(temporary: NamedTempFile, path: &Path) -> Result<fs::File, ProjectError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;
    let temporary_path = temporary.into_temp_path();
    let source: Vec<u16> = temporary_path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let destination: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    crate::sharing::retry(|| {
        // SAFETY: both paths are live, NUL-terminated UTF-16 buffers; optional pointers are null.
        let success = unsafe {
            ReplaceFileW(
                destination.as_ptr(),
                source.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if success == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    })
    .map_err(|error| io_error(path, error))?;
    fs::File::open(path).map_err(|error| io_error(path, error))
}
