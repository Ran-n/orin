// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:12:36.609616
//+ Revised: 	2026/11/13 07:53:51.845513
// -----------------------------------------------------------------------------

//! Git diff and status access without shells, external diff tools, or text converters.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Project, ProjectError};

const GIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const MAX_GIT_OUTPUT: u64 = 8 * 1024 * 1024;

/// A repository's own config can name programs Git runs while reading (`core.fsmonitor` runs
/// one on every status). Opening a folder must never run code from it, so those are turned off.
const NO_REPOSITORY_PROGRAMS: [&str; 2] = ["-c", "core.fsmonitor=false"];

fn git_output(command: &mut Command) -> Result<std::process::Output, ProjectError> {
    crate::process::capture::output(command, GIT_TIMEOUT, MAX_GIT_OUTPUT).map_err(|_| {
        tracing::warn!(target: "orin::project", "Git command could not complete within its resource limits");
        ProjectError::Git("Git could not finish. Check the repository and try again.".into())
    })
}

impl Project {
    pub fn git_diff(&self, relative: &Path, staged: bool) -> Result<String, ProjectError> {
        let relative = self.resolve_missing(relative)?;
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(self.root())
            // Reading never refreshes `.git/index`, which would wake the project watcher.
            .arg("--no-optional-locks")
            .arg("--literal-pathspecs")
            .args(NO_REPOSITORY_PROGRAMS)
            .args(["diff", "--no-ext-diff", "--no-color", "--no-textconv"]);
        if staged {
            command.arg("--cached");
        }
        command.arg("--").arg(relative);
        let output = git_output(&mut command)?;
        if !output.status.success() {
            tracing::warn!(target: "orin::project", code = output.status.code(), "git diff failed");
            return Err(ProjectError::Git(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        tracing::debug!(target: "orin::project", staged, "git diff read");
        String::from_utf8(output.stdout).map_err(|_| ProjectError::Encoding)
    }
}

/// How a file differs from the last commit, as the Files column decorates it (like VS Code).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileStatus {
    /// Changed, staged or not.
    Modified,
    /// New and staged.
    Added,
    /// New and not yet known to Git.
    Untracked,
    Deleted,
    Renamed,
    /// Both sides changed it in a merge.
    Conflict,
}

impl FileStatus {
    /// VS Code's one-letter badge.
    pub fn letter(self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Untracked => "U",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Conflict => "!",
        }
    }
}

/// The project's branch and changed files, with paths relative to the project root.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitStatus {
    /// `None` when HEAD is detached.
    pub branch: Option<String>,
    pub files: BTreeMap<PathBuf, FileStatus>,
    /// Paths Git ignores; an ignored folder is listed once, not its contents. Kept apart from
    /// `files` because being ignored is not a change.
    pub ignored: BTreeSet<PathBuf>,
}

impl GitStatus {
    /// The most notable status of anything inside `folder` (a conflict outranks a change).
    ///
    /// Paths order by component, so everything inside `folder` follows it in one run; reading
    /// only that run keeps each explorer row cheap in a repository with many changes.
    pub fn folder(&self, folder: &Path) -> Option<FileStatus> {
        use std::ops::Bound::{Excluded, Unbounded};
        self.files
            .range::<Path, _>((Excluded(folder), Unbounded))
            .take_while(|(path, _)| path.starts_with(folder))
            .map(|(_, status)| *status)
            .max()
    }

    /// Whether `path` is ignored by Git, itself or through an ignored folder it is inside.
    /// Whole path components are compared, so `targets` is not inside `target`.
    pub fn is_ignored(&self, path: &Path) -> bool {
        !self.ignored.is_empty()
            && path
                .ancestors()
                .any(|ancestor| self.ignored.contains(ancestor))
    }

    /// Parse `git status --porcelain=v1 -z --branch` output. `prefix` is the project folder's
    /// path inside the repository (empty at the top); entries outside it are left out.
    pub fn parse(output: &str, prefix: &str) -> Self {
        let mut status = Self::default();
        let mut fields = output.split('\0').filter(|field| !field.is_empty());
        while let Some(field) = fields.next() {
            if let Some(header) = field.strip_prefix("## ") {
                status.branch = branch_name(header);
                continue;
            }
            let (Some(code), Some(path)) = (field.get(..2), field.get(3..)) else {
                continue;
            };
            let bytes = code.as_bytes();
            if bytes == b"!!" {
                if let Some(relative) = relative_entry(path, prefix) {
                    status.ignored.insert(relative);
                }
                continue;
            }
            let kind = match bytes {
                [b'?', b'?'] => FileStatus::Untracked,
                [b'U', _] | [_, b'U'] | [b'A', b'A'] | [b'D', b'D'] => FileStatus::Conflict,
                [b'R', _] | [_, b'R'] | [b'C', _] | [_, b'C'] => FileStatus::Renamed,
                [b'D', _] | [_, b'D'] => FileStatus::Deleted,
                [b'A', _] => FileStatus::Added,
                _ => FileStatus::Modified,
            };
            // A rename or copy is followed by the path it came from.
            if bytes.iter().any(|code| matches!(code, b'R' | b'C')) {
                fields.next();
            }
            if let Some(relative) = relative_entry(path, prefix) {
                status.files.insert(relative, kind);
            }
        }
        status
    }
}

/// An entry's path relative to the project folder, or `None` when it lies outside it.
fn relative_entry(path: &str, prefix: &str) -> Option<PathBuf> {
    let relative = path.strip_prefix(prefix)?.trim_end_matches('/');
    (!relative.is_empty()).then(|| PathBuf::from(relative))
}

fn branch_name(header: &str) -> Option<String> {
    let header = header
        .strip_prefix("No commits yet on ")
        .or_else(|| header.strip_prefix("Initial commit on "))
        .unwrap_or(header);
    if header.starts_with("HEAD (no branch)") {
        return None;
    }
    let name = header.split("...").next()?.split(' ').next()?;
    (!name.is_empty()).then(|| name.to_owned())
}

impl Project {
    /// The branch and changed files, or `None` when the project isn't in a Git repository.
    pub fn git_status(&self) -> Result<Option<GitStatus>, ProjectError> {
        let Some(prefix) = self.git(&["rev-parse", "--show-prefix"])? else {
            return Ok(None);
        };
        let output = self
            .git(&[
                "status",
                "--porcelain=v1",
                "-z",
                "--branch",
                "--untracked-files=all",
                // `matching` lists an ignored folder once (`!! target/`); the default mode would
                // list every file under `target/` and `node_modules/`.
                "--ignored=matching",
                "--",
                ".",
            ])?
            .unwrap_or_default();
        let status = GitStatus::parse(&output, prefix.trim_end_matches(['\r', '\n']));
        tracing::debug!(
            target: "orin::project",
            changed = status.files.len(),
            ignored = status.ignored.len(),
            "git status read"
        );
        Ok(Some(status))
    }

    /// Run a read-only Git command in the project; `None` outside a repository.
    fn git(&self, arguments: &[&str]) -> Result<Option<String>, ProjectError> {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(self.root())
            // A plain `git status` may rewrite `.git/index` to refresh cached timestamps; the
            // watcher treats that as a change, so reading status would trigger another refresh.
            .arg("--no-optional-locks")
            .args(["-c", "core.quotepath=off"])
            .args(NO_REPOSITORY_PROGRAMS)
            .args(arguments);
        let output = git_output(&mut command)?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if message.contains("not a git repository") {
                return Ok(None);
            }
            tracing::warn!(target: "orin::project", code = output.status.code(), "git command failed");
            return Err(ProjectError::Git(message));
        }
        String::from_utf8(output.stdout)
            .map(Some)
            .map_err(|_| ProjectError::Encoding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_folder_status_reads_only_its_own_descendants() {
        // Siblings that share a prefix (`src-old`, `src.bak`, `srcs`) must not leak into `src`.
        let mut status = GitStatus::default();
        for (path, file) in [
            ("src", FileStatus::Conflict),
            ("src-old/a.rs", FileStatus::Conflict),
            ("src.bak", FileStatus::Conflict),
            ("srcs/b.rs", FileStatus::Conflict),
            ("src/deep/c.rs", FileStatus::Added),
            ("src/d.rs", FileStatus::Modified),
            ("a/src/e.rs", FileStatus::Deleted),
        ] {
            status.files.insert(PathBuf::from(path), file);
        }
        let scan = |folder: &Path| {
            status
                .files
                .iter()
                .filter(|(path, _)| path.starts_with(folder) && path.as_path() != folder)
                .map(|(_, status)| *status)
                .max()
        };
        for folder in [
            "", "src", "src/deep", "src-old", "srcs", "a", "a/src", "missing",
        ] {
            let folder = Path::new(folder);
            assert_eq!(status.folder(folder), scan(folder), "{}", folder.display());
        }
        assert_eq!(status.folder(Path::new("src")), Some(FileStatus::Added));
    }

    #[test]
    fn unit_porcelain_output_becomes_badges_and_a_branch() {
        let output = "## main...origin/main [ahead 1]\0 M src/main.rs\0M  staged.rs\0A  new.rs\0?? notes.md\0 D gone.rs\0R  renamed.rs\0old.rs\0UU both.rs\0?? sub/deep/file.txt\0";
        let status = GitStatus::parse(output, "");
        assert_eq!(status.branch.as_deref(), Some("main"));
        let get = |path: &str| status.files.get(Path::new(path)).copied();
        assert_eq!(get("src/main.rs"), Some(FileStatus::Modified));
        assert_eq!(get("staged.rs"), Some(FileStatus::Modified));
        assert_eq!(get("new.rs"), Some(FileStatus::Added));
        assert_eq!(get("notes.md"), Some(FileStatus::Untracked));
        assert_eq!(get("gone.rs"), Some(FileStatus::Deleted));
        assert_eq!(get("renamed.rs"), Some(FileStatus::Renamed));
        assert_eq!(
            get("old.rs"),
            None,
            "a rename's source is not a separate entry"
        );
        assert_eq!(get("both.rs"), Some(FileStatus::Conflict));
        assert_eq!(status.folder(Path::new("sub")), Some(FileStatus::Untracked));
        assert_eq!(status.folder(Path::new("src")), Some(FileStatus::Modified));
        assert_eq!(status.folder(Path::new("docs")), None);
        assert_eq!(FileStatus::Untracked.letter(), "U");
        assert_eq!(FileStatus::Conflict.letter(), "!");
    }

    #[test]
    fn unit_branch_headers_and_a_project_inside_the_repository() {
        assert_eq!(
            branch_name("No commits yet on main").as_deref(),
            Some("main")
        );
        assert_eq!(branch_name("feature/x").as_deref(), Some("feature/x"));
        assert_eq!(branch_name("HEAD (no branch)"), None);
        let output = "## dev\0 M app/src/lib.rs\0 M other/readme.md\0";
        let status = GitStatus::parse(output, "app/");
        assert_eq!(status.files.len(), 1);
        assert!(status.files.contains_key(Path::new("src/lib.rs")));
    }

    #[test]
    fn unit_ignored_entries_are_kept_apart_from_changes() {
        let status = GitStatus::parse("## main\0!! target/\0!! .env\0 M src/main.rs\0", "");
        assert_eq!(
            status.ignored,
            BTreeSet::from([PathBuf::from("target"), PathBuf::from(".env")])
        );
        assert_eq!(status.files.len(), 1);
        assert!(status.files.contains_key(Path::new("src/main.rs")));
        assert_eq!(status.folder(Path::new("target")), None);
        assert_eq!(status.folder(Path::new("")), Some(FileStatus::Modified));
    }

    #[test]
    fn unit_paths_inside_an_ignored_folder_are_ignored() {
        let status = GitStatus::parse("## main\0!! target/\0!! .env\0", "");
        for path in ["target", "target/debug/app.bin", ".env"] {
            assert!(status.is_ignored(Path::new(path)), "{path}");
        }
        for path in ["src", "targets", "src/target.rs"] {
            assert!(!status.is_ignored(Path::new(path)), "{path}");
        }
        assert!(!GitStatus::default().is_ignored(Path::new("target")));
    }

    #[test]
    fn unit_ignored_entries_outside_the_project_prefix_are_dropped() {
        let status = GitStatus::parse("## main\0!! other/target/\0!! app/target/\0", "app/");
        assert_eq!(status.ignored, BTreeSet::from([PathBuf::from("target")]));
    }
}
