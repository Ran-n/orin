// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:12:36.486646
//+ Revised: 	2026/11/13 07:53:51.908907
// -----------------------------------------------------------------------------

//! Bounded text reads and lazy directory enumeration.

use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::{FileVersion, Project, ProjectError, io_error};

pub const MAX_FILE_BYTES: usize = 1024 * 1024;
/// Folders the Files column never lists. Like VS Code, only Git's own store is hidden; build
/// output such as `target/` is listed and drawn dimmed, so it fades instead of vanishing.
const HIDDEN_DIRECTORIES: &[&str] = &[".git"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub relative_path: PathBuf,
    pub is_dir: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileContent {
    pub relative_path: PathBuf,
    pub text: String,
    pub byte_len: usize,
    pub version: FileVersion,
}

impl Project {
    pub fn list_dir(&self, relative: &Path) -> Result<Vec<Entry>, ProjectError> {
        let path = self.resolve(relative)?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&path).map_err(|error| io_error(&path, error))? {
            let entry = entry.map_err(|error| io_error(&path, error))?;
            if let Some(entry) = self.directory_entry(relative, entry)? {
                entries.push(entry);
            }
        }
        entries.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.cmp(&right.name))
        });
        tracing::debug!(target: "orin::project", count = entries.len(), "directory listed");
        Ok(entries)
    }

    fn directory_entry(
        &self,
        parent: &Path,
        entry: fs::DirEntry,
    ) -> Result<Option<Entry>, ProjectError> {
        let name = entry.file_name().to_string_lossy().into_owned();
        // A link to something deleted can't be opened; one such link must not hide the folder.
        if entry.file_type().is_ok_and(|kind| kind.is_symlink())
            && fs::metadata(entry.path()).is_err()
        {
            tracing::debug!(target: "orin::project", "broken symlink excluded from explorer");
            return Ok(None);
        }
        let relative_path = self.relative_path(&parent.join(entry.file_name()))?;
        let resolved = match self.resolve(&relative_path) {
            Ok(path) => path,
            Err(ProjectError::OutsideRoot) => {
                tracing::warn!(target: "orin::project", "external symlink excluded from explorer");
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let is_dir = resolved.is_dir();
        if is_dir && HIDDEN_DIRECTORIES.contains(&name.as_str()) {
            return Ok(None);
        }
        Ok(Some(Entry {
            name,
            relative_path,
            is_dir,
        }))
    }

    pub fn read_file(&self, relative: &Path) -> Result<FileContent, ProjectError> {
        let path = self.resolve(relative)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // A repository may contain FIFOs. Opening one must never wait for a writer.
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = options
            .open(&path)
            .map_err(|error| io_error(&path, error))?;
        if !file
            .metadata()
            .map_err(|error| io_error(&path, error))?
            .is_file()
        {
            return Err(io_error(
                &path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Only regular text files can be opened",
                ),
            ));
        }
        let version = FileVersion::capture(&path, &file)?;
        let bytes = read_bounded(&file, &path)?;
        let byte_len = bytes.len();
        let text = decode_text(bytes)?;
        if FileVersion::capture(&path, &file)? != version {
            return Err(ProjectError::Conflict);
        }
        tracing::debug!(target: "orin::project", byte_len, "file read");
        Ok(FileContent {
            relative_path: self.relative_path(relative)?,
            text,
            byte_len,
            version,
        })
    }
}

fn read_bounded(file: &File, path: &Path) -> Result<Vec<u8>, ProjectError> {
    if file
        .metadata()
        .map_err(|error| io_error(path, error))?
        .len()
        > MAX_FILE_BYTES as u64
    {
        return Err(ProjectError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, error))?;
    Ok(bytes)
}

fn decode_text(bytes: Vec<u8>) -> Result<String, ProjectError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(ProjectError::TooLarge);
    }
    if bytes.contains(&0) {
        return Err(ProjectError::Binary);
    }
    String::from_utf8(bytes).map_err(|_| ProjectError::Encoding)
}

#[cfg(test)]
mod tests {
    use super::{MAX_FILE_BYTES, ProjectError, decode_text};

    #[test]
    fn accepts_empty_text() {
        assert_eq!(decode_text(Vec::new()).unwrap(), "");
    }

    #[test]
    fn gives_size_limit_precedence_over_binary_detection() {
        assert!(matches!(
            decode_text(vec![0; MAX_FILE_BYTES + 1]),
            Err(ProjectError::TooLarge)
        ));
    }

    #[test]
    fn distinguishes_binary_from_invalid_encoding() {
        assert!(matches!(decode_text(vec![0]), Err(ProjectError::Binary)));
        assert!(matches!(
            decode_text(vec![255]),
            Err(ProjectError::Encoding)
        ));
    }
}
