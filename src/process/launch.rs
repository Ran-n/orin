// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:37:14.156451
//+ Revised: 	2026/11/14 23:31:53.783121
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

/// Search configured tool folders first, then PATH and standard user installations.
pub(crate) fn tool_path(extra: &[PathBuf], discover: bool) -> Option<OsString> {
    let mut directories = extra.to_vec();
    directories.extend(
        env::var_os("PATH")
            .iter()
            .flat_map(|path| env::split_paths(path)),
    );
    if discover {
        #[cfg(windows)]
        directories.extend(windows_tool_directories());
        if let Some(base) = directories::BaseDirs::new() {
            directories.push(base.home_dir().join(".local/bin"));
            directories.push(base.home_dir().join(".npm-global/bin"));
            #[cfg(windows)]
            {
                directories.push(base.home_dir().join("AppData/Roaming/npm"));
                directories.push(base.home_dir().join("scoop/shims"));
            }
        }
        #[cfg(windows)]
        for (variable, suffix) in [
            ("NVM_SYMLINK", ""),
            ("NVM_HOME", ""),
            ("ProgramFiles", "nodejs"),
            ("LOCALAPPDATA", "Programs/nodejs"),
            ("APPDATA", "npm"),
        ] {
            if let Some(root) = env::var_os(variable) {
                directories.push(PathBuf::from(root).join(suffix));
            }
        }
        #[cfg(unix)]
        directories.extend([
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/homebrew/bin"),
        ]);
    }
    let mut unique = Vec::new();
    for directory in directories {
        if directory.is_absolute() && !unique.contains(&directory) {
            unique.push(directory);
        }
    }
    match env::join_paths(unique) {
        Ok(path) => Some(path),
        Err(error) => {
            tracing::error!(%error, "Tool search path could not be constructed");
            None
        }
    }
}

#[cfg(windows)]
fn windows_tool_directories() -> Vec<PathBuf> {
    use windows_sys::Win32::System::Registry::*;
    fn value(root: HKEY, key: &str, name: &str) -> Option<String> {
        let key: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut buffer = vec![0u16; 32_768];
        let mut bytes = (buffer.len() * 2) as u32;
        // The key/value strings are terminated and the output length describes this buffer.
        let result = unsafe {
            RegGetValueW(
                root,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if result != 0 {
            if ![2, 3].contains(&result) {
                tracing::warn!(result, "Windows tool environment could not be read");
            }
            return None;
        }
        let units = (bytes as usize / 2).min(buffer.len());
        Some(
            String::from_utf16_lossy(&buffer[..units])
                .trim_end_matches('\0')
                .to_owned(),
        )
    }
    let keys = [
        (HKEY_CURRENT_USER, "Environment"),
        (
            HKEY_LOCAL_MACHINE,
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
        ),
    ];
    let lookup = |name: &str| {
        keys.iter()
            .find_map(|(root, key)| value(*root, key, name))
            .or_else(|| env::var(name).ok())
    };
    let expand = |text: &str| expand_environment(text, lookup);
    let mut directories = Vec::new();
    for (root, key) in keys {
        if let Some(path) = value(root, key, "Path") {
            directories.extend(env::split_paths(&expand(&path)));
        }
        for name in ["NVM_SYMLINK", "NVM_HOME"] {
            if let Some(path) = value(root, key, name) {
                directories.push(PathBuf::from(expand(&path)));
            }
        }
    }
    directories
}

#[cfg(any(windows, test))]
fn expand_environment(text: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut expanded = text.to_owned();
    for _ in 0..4 {
        let previous = expanded.clone();
        let mut remaining = previous.as_str();
        expanded.clear();
        while let Some(start) = remaining.find('%') {
            expanded.push_str(&remaining[..start]);
            remaining = &remaining[start + 1..];
            let Some(end) = remaining.find('%') else {
                expanded.push('%');
                break;
            };
            match lookup(&remaining[..end]) {
                Some(value) => expanded.push_str(&value),
                None => {
                    expanded.push('%');
                    expanded.push_str(&remaining[..=end]);
                }
            }
            remaining = &remaining[end + 1..];
        }
        expanded.push_str(remaining);
        if expanded == previous {
            break;
        }
    }
    expanded
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

#[cfg(test)]
mod tool_path_tests {
    use super::*;

    #[test]
    fn platform_tool_discovery_retains_only_absolute_folders() {
        let path = tool_path(&[], true).unwrap();
        assert!(env::split_paths(&path).all(|folder| folder.is_absolute()));
        #[cfg(windows)]
        assert!(
            !windows_tool_directories().is_empty(),
            "Windows' registered tool paths must be discoverable without Explorer's PATH"
        );
    }

    #[test]
    fn nested_environment_paths_expand_and_unknown_variables_remain_literal() {
        let lookup = |name: &str| match name {
            "NODE" => Some("%ROOT%/node".into()),
            "ROOT" => Some("C:/Tools Ω".into()),
            "LOOP" => Some("%LOOP%/again".into()),
            _ => None,
        };
        assert_eq!(
            expand_environment("%NODE%/bin", lookup),
            "C:/Tools Ω/node/bin"
        );
        assert_eq!(expand_environment("%MISSING%/bin", lookup), "%MISSING%/bin");
        assert_eq!(expand_environment("folder%tail", lookup), "folder%tail");
        assert_eq!(
            expand_environment("%LOOP%", lookup),
            "%LOOP%/again/again/again/again"
        );
    }

    #[test]
    fn configured_tool_folders_take_priority_and_survive_spaces_and_unicode() {
        let folder = tempfile::Builder::new()
            .prefix("orin tools Ω ")
            .tempdir()
            .unwrap();
        let executable = folder.path().join(if cfg!(windows) {
            "orin-tool-probe.exe"
        } else {
            "orin-tool-probe"
        });
        std::fs::write(&executable, "fixture").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = tool_path(&[folder.path().to_owned(), folder.path().to_owned()], false).unwrap();
        assert_eq!(
            resolve(Path::new("orin-tool-probe"), Some(&path)),
            Some(executable)
        );
        assert_eq!(
            env::split_paths(&path)
                .filter(|entry| entry == folder.path())
                .count(),
            1
        );
    }
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
