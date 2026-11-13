// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:51.793187
//+ Revised: 	2026/11/12 09:16:41.988361
// -----------------------------------------------------------------------------

//! The developer's system shell: the only program the Terminal pane runs.
//! Agents are never wrapped in a terminal; they use structured ACP sessions (ADR-004).

use std::env;

use anyhow::{Context, Result};
use portable_pty::CommandBuilder;

use crate::process::launch::{pty_command, resolve, shell_program};

/// Build the PTY command for the user's shell (`ComSpec` on Windows, `SHELL` elsewhere).
pub(super) fn shell_command() -> Result<CommandBuilder> {
    let executable = resolve(&shell_program(), env::var_os("PATH").as_deref()).context(
        "Your system shell could not be found. Check the ComSpec (Windows) or SHELL setting.",
    )?;
    #[allow(unused_mut)]
    let mut command = pty_command(&executable)?;
    // Skip cmd.exe AutoRun scripts so a user's profile hook cannot hijack the session start.
    #[cfg(windows)]
    if executable
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("cmd.exe"))
    {
        command.arg("/D");
    }
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::launch::candidates;
    use std::{ffi::OsStr, path::Path};

    #[test]
    fn unit_system_shell_resolves() {
        assert!(shell_command().is_ok());
    }

    #[test]
    fn missing_program_has_no_candidate() {
        assert!(
            resolve(
                Path::new("orin-definitely-not-installed"),
                Some(OsStr::new(""))
            )
            .is_none()
        );
    }

    #[test]
    fn relative_path_entries_cannot_launch_working_directory_files() {
        assert!(resolve(Path::new("anything"), Some(OsStr::new("."))).is_none());
    }

    #[test]
    fn absolute_program_is_resolved_without_path() {
        let shell = resolve(&shell_program(), env::var_os("PATH").as_deref()).unwrap();
        assert_eq!(resolve(&shell, None), Some(shell));
    }

    #[test]
    fn executable_candidates_follow_native_platform() {
        let names = candidates(Path::new("codex"));
        #[cfg(windows)]
        assert_eq!(names[0], Path::new("codex.exe"));
        #[cfg(unix)]
        assert_eq!(names, vec![std::path::PathBuf::from("codex")]);
    }
}
