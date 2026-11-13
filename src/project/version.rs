// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:18.194301
//+ Revised: 	2026/11/12 09:16:37.751854
// -----------------------------------------------------------------------------

//! File identities prevent saving over replaced or retargeted documents.

use super::{ProjectError, io_error};
use std::{
    fs::File,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileVersion {
    canonical_path: PathBuf,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    length: u64,
    identity: (u64, u64),
}

impl FileVersion {
    pub(super) fn capture(path: &Path, file: &File) -> Result<Self, ProjectError> {
        let metadata = file.metadata().map_err(|error| io_error(path, error))?;
        Ok(Self {
            canonical_path: path.into(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            length: metadata.len(),
            identity: identity(file, &metadata).map_err(|error| io_error(path, error))?,
        })
    }
}

#[cfg(unix)]
fn identity(_: &File, metadata: &std::fs::Metadata) -> std::io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn identity(file: &File, _: &std::fs::Metadata) -> std::io::Result<(u64, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: the open File owns a live handle; the OS initializes this output on success.
    let success =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) };
    if success == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: the successful API call initialized the complete information structure.
    let information = unsafe { information.assume_init() };
    let index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    Ok((u64::from(information.dwVolumeSerialNumber), index))
}
