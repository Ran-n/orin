// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:37:14.156451
//+ Revised: 	2026/11/12 15:10:20.551862
// -----------------------------------------------------------------------------

use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(windows)]
use anyhow::Context;
use anyhow::Result;

/// The form of a canonical path that child processes accept as a working directory.
///
/// On Windows, `canonicalize` returns verbatim paths (`\\?\C:\...`, `\\?\UNC\server\...`).
/// `cmd.exe` refuses them as a current directory and falls back to the Windows folder, and agent
/// adapters expect ordinary absolute paths, so the prefix is removed. Other platforms are unchanged.
pub(crate) fn native_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    if let Some(text) = path.to_str() {
        if let Some(share) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{share}"));
        }
        if let Some(local) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(local);
        }
    }
    path.to_owned()
}

pub(crate) fn resolve(program: &Path, path: Option<&OsStr>) -> Option<PathBuf> {
    if program.is_absolute() {
        return executable(program).then(|| program.to_owned());
    }
    env::split_paths(path?)
        .filter(|directory| directory.is_absolute())
        .find_map(|directory| {
            candidates(program)
                .into_iter()
                .map(|name| directory.join(name))
                .find(|path| executable(path))
        })
}

pub(crate) fn candidates(program: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        if program.extension().is_some() {
            return vec![program.to_owned()];
        }
        ["exe", "com", "cmd", "bat"]
            .iter()
            .map(|extension| program.with_extension(extension))
            .collect()
    }
    #[cfg(unix)]
    {
        vec![program.to_owned()]
    }
}

pub(crate) fn executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(windows)]
    {
        path.is_file()
    }
}

pub(crate) fn shell_program() -> PathBuf {
    #[cfg(windows)]
    {
        windows_separators(&env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into()))
    }
    #[cfg(unix)]
    {
        env::var_os("SHELL")
            .map(PathBuf::from)
            .filter(|path| executable(path))
            .unwrap_or_else(|| "/bin/sh".into())
    }
}

/// cmd.exe parses its own unquoted program path, so `C:\Windows/System32/cmd.exe` would turn
/// `/System32/cmd.exe` into switches and exit with "The syntax of the command is incorrect."
#[cfg(windows)]
fn windows_separators(program: &OsStr) -> PathBuf {
    match program.to_str() {
        Some(text) => text.replace('/', "\\").into(),
        None => program.into(),
    }
}

pub(crate) fn command(program: &Path, args: &[OsString]) -> Result<Command> {
    #[cfg(windows)]
    if is_batch(program) {
        let mut command = Command::new(
            resolve(&shell_program(), env::var_os("PATH").as_deref())
                .context("Windows command interpreter is unavailable.")?,
        );
        // cmd.exe has distinct argument rules; paths remain a quoted environment expansion.
        command.args(["/D", "/V:OFF", "/S", "/C"]);
        use std::os::windows::process::CommandExt;
        command.raw_arg("\"\"%ORIN_CLI_LAUNCHER%\" %ORIN_CLI_ARGUMENTS%\"");
        command.env("ORIN_CLI_LAUNCHER", program);
        command.env("ORIN_CLI_ARGUMENTS", batch_args(args)?);
        return Ok(command);
    }
    let mut command = Command::new(program);
    command.args(args);
    Ok(command)
}

pub(crate) fn pty_command(program: &Path) -> Result<portable_pty::CommandBuilder> {
    #[cfg(windows)]
    if is_batch(program) {
        let shell = resolve(&shell_program(), env::var_os("PATH").as_deref())
            .context("Windows command interpreter is unavailable.")?;
        let mut command = portable_pty::CommandBuilder::new(shell);
        command.env("ORIN_CLI_LAUNCHER", format!("\"{}\"", program.display()));
        command.args(["/D", "/V:OFF", "/C", "%ORIN_CLI_LAUNCHER%"]);
        return Ok(command);
    }
    Ok(portable_pty::CommandBuilder::new(program))
}

#[cfg(windows)]
fn is_batch(program: &Path) -> bool {
    program.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
    })
}

#[cfg(windows)]
fn batch_args(args: &[OsString]) -> Result<String> {
    args.iter()
        .map(|argument| {
            let argument = argument
                .to_str()
                .context("Batch launcher arguments must be valid Unicode.")?;
            anyhow::ensure!(
                argument
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_./@".contains(&byte)),
                "Batch launcher argument contains unsupported characters."
            );
            Ok(argument)
        })
        .collect::<Result<Vec<_>>>()
        .map(|arguments| arguments.join(" "))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn integration_batch_launch_preserves_spaces_unicode_and_metacharacters_in_paths() {
        let directory = tempfile::Builder::new()
            .prefix("orin & %PATH% ! 智 spaces-")
            .tempdir()
            .unwrap();
        let launcher = directory.path().join("fixture adapter.cmd");
        std::fs::write(&launcher, "@echo launcher-ok\r\n").unwrap();
        let result = command(&launcher, &[]).unwrap().output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout).trim(),
            "launcher-ok"
        );
    }

    #[test]
    fn regression_comspec_with_forward_slashes_uses_backslashes() {
        assert_eq!(
            windows_separators(OsStr::new(r"C:\WINDOWS/System32/cmd.exe")),
            Path::new(r"C:\WINDOWS\System32\cmd.exe")
        );
        assert_eq!(
            windows_separators(OsStr::new("cmd.exe")),
            Path::new("cmd.exe")
        );
    }

    #[test]
    fn unit_native_path_removes_verbatim_drive_and_unc_prefixes() {
        assert_eq!(
            native_path(Path::new(r"\\?\C:\project")),
            Path::new(r"C:\project")
        );
        assert_eq!(
            native_path(Path::new(r"\\?\UNC\server\share\project")),
            Path::new(r"\\server\share\project")
        );
        assert_eq!(native_path(Path::new(r"D:\plain")), Path::new(r"D:\plain"));
    }

    #[test]
    fn regression_shell_starts_in_canonical_project_directory() {
        let directory = tempfile::tempdir().unwrap();
        let canonical = directory.path().canonicalize().unwrap();
        assert!(canonical.to_string_lossy().starts_with(r"\\?\"));
        let output = Command::new(shell_program())
            .args(["/D", "/C", "cd"])
            .current_dir(native_path(&canonical))
            .output()
            .unwrap();
        assert!(output.status.success());
        let reported = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            Path::new(reported.trim()).canonicalize().unwrap(),
            canonical
        );
    }

    /// `npx` is a `.cmd` launcher on Windows (nvm, nvm4w, the Node installer), so every
    /// provider's package arguments must pass the batch filter or no agent can start.
    #[test]
    fn regression_every_npx_adapter_package_passes_the_batch_launcher() {
        for provider in crate::agents::Provider::ALL {
            let arguments = provider.arguments(Some(provider.package()));
            assert!(
                batch_args(&arguments).is_ok(),
                "{provider:?}: {arguments:?}"
            );
        }
    }

    #[test]
    fn unit_batch_args_accept_only_static_switches() {
        assert_eq!(batch_args(&["--acp".into()]).unwrap(), "--acp");
        for argument in [
            "&calc",
            "%PATH%",
            "!secret!",
            "with spaces",
            "\"quoted\"",
            "^0",
        ] {
            assert!(batch_args(&[argument.into()]).is_err());
        }
    }
}
