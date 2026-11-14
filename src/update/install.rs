// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:38.076712
//+ Revised: 	2026/11/14 12:09:09.597830
// -----------------------------------------------------------------------------

//! Swapping a verified download in for the running binary, and starting it afterwards.

use super::{
    download::{MAX_DOWNLOAD_TIME, PREFIX},
    release::UpdateError,
};
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};
use tempfile::NamedTempFile;

/// Replace the running executable with `file` and return the path to start it from.
///
/// `self_replace` handles Windows, where a running `.exe` can't be overwritten: it moves the old
/// one aside and deletes it once Orin exits. The path is read before the swap, because afterwards
/// the running image may report the name it was moved to.
pub fn install(file: NamedTempFile) -> Result<PathBuf, UpdateError> {
    let executable = std::env::current_exe().map_err(|error| {
        UpdateError::Io(format!(
            "Orin couldn't find its own file ({})",
            error.kind()
        ))
    })?;
    install_at(file, &executable)?;
    Ok(executable)
}

fn install_at(file: NamedTempFile, executable: &Path) -> Result<(), UpdateError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o755))
            .map_err(|error| UpdateError::Io(error.to_string()))?;
    }
    self_replace::self_replace(file.path()).map_err(|error| {
        tracing::error!(target: "orin::update", %error, "Update could not replace the running binary");
        UpdateError::Io(format!(
            "Orin couldn't replace itself at {}: {error}",
            executable.display()
        ))
    })?;
    tracing::info!(target: "orin::update", "Update installed");
    Ok(())
}

/// How to start the new version after quitting: the same project and settings file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Relaunch {
    pub executable: PathBuf,
    pub project: Option<PathBuf>,
    pub settings: Option<PathBuf>,
    /// Kept under the UI test driver, so a restarted test window never takes focus or checks
    /// for updates on its own.
    pub test_driver: bool,
}

impl Relaunch {
    /// The arguments that reopen the same project with the same settings file.
    pub fn arguments(&self) -> Vec<OsString> {
        let mut arguments = Vec::new();
        if let Some(project) = &self.project {
            arguments.push("--project".into());
            arguments.push(project.as_os_str().to_owned());
        }
        if let Some(settings) = &self.settings {
            arguments.push("--settings".into());
            arguments.push(settings.as_os_str().to_owned());
        }
        if self.test_driver {
            arguments.push("--test-driver".into());
        }
        arguments
    }
}

/// Start the new version detached, so closing this Orin doesn't take it along.
pub fn relaunch(relaunch: &Relaunch) -> io::Result<()> {
    let mut command = Command::new(&relaunch.executable);
    command
        .args(relaunch.arguments())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    command.spawn().map(drop)
}

/// A download younger than this may be one another Orin window is still making: a download is
/// given up on after `MAX_DOWNLOAD_TIME` and installed at once, so only an older file is a
/// leftover.
pub const LEFTOVER_AGE: Duration = Duration::from_secs(MAX_DOWNLOAD_TIME.as_secs() + 10 * 60);

/// Delete downloads a crashed run left in `dir`; other files there are kept, and so is a
/// download young enough to still be in progress in another Orin window. One file that can't
/// be deleted (still open elsewhere, say) is logged and skipped, so it doesn't keep the rest.
pub fn clean_leftovers(dir: &Path) -> io::Result<usize> {
    clean_leftovers_older_than(dir, LEFTOVER_AGE)
}

/// How long ago `entry` was created; where the file system doesn't record that, last written.
/// A time in the future (the clock was set back) counts as just now.
fn age(entry: &std::fs::DirEntry) -> io::Result<Duration> {
    let metadata = entry.metadata()?;
    let since = metadata.created().or_else(|_| metadata.modified())?;
    Ok(SystemTime::now()
        .duration_since(since)
        .unwrap_or(Duration::ZERO))
}

fn clean_leftovers_older_than(dir: &Path, minimum: Duration) -> io::Result<usize> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    let mut removed = 0;
    for entry in entries {
        let result = entry.and_then(|entry| {
            let leftover = entry.file_name().to_string_lossy().starts_with(PREFIX)
                && entry.file_type()?.is_file()
                && age(&entry)? >= minimum;
            if leftover {
                std::fs::remove_file(entry.path())?;
            }
            Ok(leftover)
        });
        match result {
            Ok(true) => removed += 1,
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(target: "orin::update", %error, "A leftover update download could not be deleted");
            }
        }
    }
    Ok(removed)
}

/// Where downloads wait until they are installed: `<cache>/updates`. Debug builds honour
/// `ORIN_UPDATE_DIR`, so tests never write to the real cache.
pub fn download_dir() -> Option<PathBuf> {
    if cfg!(debug_assertions)
        && let Some(dir) = std::env::var_os("ORIN_UPDATE_DIR")
    {
        return Some(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("com", "Breren", "Orin")
        .map(|dirs| dirs.cache_dir().join("updates"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_relaunch_arguments() {
        let project = Path::new("/work/app");
        let settings = Path::new("/tmp/s.json");
        let text = |arguments: Vec<OsString>| {
            arguments
                .into_iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        let arguments = |project: Option<&Path>, settings: Option<&Path>, test_driver: bool| {
            text(
                Relaunch {
                    executable: PathBuf::from("orin"),
                    project: project.map(Path::to_path_buf),
                    settings: settings.map(Path::to_path_buf),
                    test_driver,
                }
                .arguments(),
            )
        };
        assert!(arguments(None, None, false).is_empty());
        assert_eq!(
            arguments(Some(project), None, false),
            ["--project", "/work/app"]
        );
        assert_eq!(
            arguments(None, Some(settings), false),
            ["--settings", "/tmp/s.json"]
        );
        assert_eq!(
            arguments(Some(project), Some(settings), false),
            ["--project", "/work/app", "--settings", "/tmp/s.json"]
        );
        assert_eq!(
            arguments(None, Some(settings), true),
            ["--settings", "/tmp/s.json", "--test-driver"]
        );
    }

    #[test]
    fn unit_clean_leftovers_keeps_other_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(format!("{PREFIX}abc")), "partial").unwrap();
        std::fs::write(dir.path().join(format!("{PREFIX}def")), "partial").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "keep").unwrap();
        std::fs::create_dir(dir.path().join(format!("{PREFIX}folder"))).unwrap();
        assert_eq!(
            clean_leftovers_older_than(dir.path(), Duration::ZERO).unwrap(),
            2
        );
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["notes.txt", "orin-update-folder"]);
        assert_eq!(clean_leftovers(&dir.path().join("missing")).unwrap(), 0);
    }

    #[test]
    fn unit_a_download_another_window_may_still_be_making_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let download = dir.path().join(format!("{PREFIX}in-progress"));
        std::fs::write(&download, "half a binary").unwrap();
        // Just created: it could be a second Orin's download, which used to be deleted under it.
        assert_eq!(clean_leftovers(dir.path()).unwrap(), 0);
        assert!(download.exists());
        assert!(
            LEFTOVER_AGE > MAX_DOWNLOAD_TIME,
            "no live download is that old"
        );
        // Once it is old enough to be a crashed run's, it goes.
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(
            clean_leftovers_older_than(dir.path(), Duration::from_millis(20)).unwrap(),
            1
        );
        assert!(!download.exists());
    }

    #[test]
    fn unit_clean_leftovers_reports_an_unreadable_folder() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert!(
            clean_leftovers(file.path()).is_err(),
            "a file is not a folder"
        );
    }

    #[test]
    fn unit_relaunch_starts_the_program_and_reports_a_missing_one() {
        // A harmless program that exits at once with no input: `cmd` on Windows, `true` elsewhere.
        // `/usr/bin/true` exists on both Linux and macOS; macOS has no `/bin/true`.
        let harmless = if cfg!(windows) {
            PathBuf::from(std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into()))
        } else {
            PathBuf::from("/usr/bin/true")
        };
        let relaunched = Relaunch {
            executable: harmless,
            project: Some(PathBuf::from("project")),
            settings: None,
            test_driver: false,
        };
        assert!(relaunch(&relaunched).is_ok());
        let missing = Relaunch {
            executable: PathBuf::from("/no/such/orin-binary"),
            ..relaunched
        };
        assert!(relaunch(&missing).is_err());
    }

    #[test]
    fn unit_download_dir_is_the_cache_updates_folder() {
        if let Some(dir) = download_dir() {
            assert!(dir.ends_with("updates"));
        }
    }
}
