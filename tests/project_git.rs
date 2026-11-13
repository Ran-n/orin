// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:12:37.263702
//+ Revised: 	2026/11/13 07:53:52.115533
// -----------------------------------------------------------------------------

use std::fs;
use std::path::Path;
use std::process::Command;

use orin::project::{FileStatus, Project, ProjectError};
use tempfile::TempDir;

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Orin Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=",
        ])
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn repository() -> TempDir {
    let directory = TempDir::new().unwrap();
    git(directory.path(), &["init"]);
    git(
        directory.path(),
        &["symbolic-ref", "HEAD", "refs/heads/main"],
    );
    fs::write(directory.path().join("example.rs"), "before\n").unwrap();
    git(directory.path(), &["add", "--", "example.rs"]);
    git(directory.path(), &["commit", "-m", "fixture"]);
    directory
}

#[test]
fn distinguishes_unstaged_and_staged_file_changes() {
    let directory = repository();
    let project = Project::open(directory.path()).unwrap();
    fs::write(directory.path().join("example.rs"), "after\n").unwrap();
    let diff = project.git_diff(Path::new("example.rs"), false).unwrap();
    assert!(diff.contains("-before"));
    assert!(diff.contains("+after"));
    assert!(!diff.contains("\u{1b}["));
    assert!(
        project
            .git_diff(Path::new("example.rs"), true)
            .unwrap()
            .is_empty()
    );
    git(directory.path(), &["add", "--", "example.rs"]);
    assert!(
        project
            .git_diff(Path::new("example.rs"), false)
            .unwrap()
            .is_empty()
    );
    assert!(
        project
            .git_diff(Path::new("example.rs"), true)
            .unwrap()
            .contains("+after")
    );
}

#[test]
fn diffs_deleted_files_and_keeps_pathspecs_literal() {
    let directory = repository();
    fs::write(directory.path().join("[file].rs"), "before\n").unwrap();
    git(directory.path(), &["add", "--", "[file].rs"]);
    git(directory.path(), &["commit", "-m", "literal filename"]);
    fs::write(directory.path().join("[file].rs"), "literal\n").unwrap();
    let project = Project::open(directory.path()).unwrap();
    assert!(
        project
            .git_diff(Path::new("[file].rs"), false)
            .unwrap()
            .contains("+literal")
    );
    fs::remove_file(directory.path().join("example.rs")).unwrap();
    assert!(
        project
            .git_diff(Path::new("example.rs"), false)
            .unwrap()
            .contains("-before")
    );
}

#[test]
fn reports_non_git_projects_without_silent_empty_diff() {
    let directory = TempDir::new().unwrap();
    fs::write(directory.path().join("file"), "text").unwrap();
    let project = Project::open(directory.path()).unwrap();
    assert!(matches!(
        project.git_diff(Path::new("file"), false),
        Err(ProjectError::Git(_))
    ));
}

#[test]
fn external_diff_configuration_is_never_executed() {
    let directory = repository();
    git(
        directory.path(),
        &["config", "diff.external", "a-command-that-does-not-exist"],
    );
    fs::write(directory.path().join("example.rs"), "safe\n").unwrap();
    let project = Project::open(directory.path()).unwrap();
    assert!(
        project
            .git_diff(Path::new("example.rs"), false)
            .unwrap()
            .contains("+safe")
    );
}

#[test]
fn integration_status_reports_branch_and_changed_files_like_vs_code() {
    let repository = repository();
    let root = repository.path();
    fs::write(root.join("example.rs"), "changed\n").unwrap();
    fs::write(root.join("new.txt"), "new\n").unwrap();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub/staged.txt"), "staged\n").unwrap();
    git(root, &["add", "sub/staged.txt"]);
    let project = Project::open(root).unwrap();
    let status = project.git_status().unwrap().expect("a repository");
    assert_eq!(status.branch.as_deref(), Some("main"));
    assert_eq!(
        status.files.get(Path::new("example.rs")),
        Some(&FileStatus::Modified)
    );
    assert_eq!(
        status.files.get(Path::new("new.txt")),
        Some(&FileStatus::Untracked)
    );
    assert_eq!(
        status.files.get(Path::new("sub/staged.txt")),
        Some(&FileStatus::Added)
    );
    assert_eq!(status.folder(Path::new("sub")), Some(FileStatus::Added));

    // A project opened in a subfolder sees paths relative to itself.
    let nested = Project::open(&root.join("sub")).unwrap();
    let status = nested.git_status().unwrap().unwrap();
    assert_eq!(status.files.len(), 1);
    assert_eq!(
        status.files.get(Path::new("staged.txt")),
        Some(&FileStatus::Added)
    );
}

#[test]
fn integration_a_folder_outside_git_has_no_status() {
    let directory = TempDir::new().unwrap();
    let project = Project::open(directory.path()).unwrap();
    assert_eq!(project.git_status().unwrap(), None);
}

#[test]
fn regression_worktree_rename_consumes_source_before_parsing_next_entry() {
    // Both index and worktree renames have a second NUL-delimited path.
    // A source beginning with a status-like prefix used to become a phantom change.
    let status =
        orin::project::GitStatus::parse("## main\0 R renamed.rs\0 M phantom.rs\0?? real.rs\0", "");
    assert_eq!(status.files.len(), 2);
    assert_eq!(
        status.files.get(Path::new("renamed.rs")),
        Some(&FileStatus::Renamed)
    );
    assert_eq!(
        status.files.get(Path::new("real.rs")),
        Some(&FileStatus::Untracked)
    );
    assert!(!status.files.contains_key(Path::new("phantom.rs")));
}

#[test]
fn integration_status_reports_ignored_folders_once() {
    let directory = repository();
    fs::write(directory.path().join(".gitignore"), "target/\n").unwrap();
    let deep = directory.path().join("target/a/b");
    fs::create_dir_all(&deep).unwrap();
    for index in 0..50 {
        fs::write(deep.join(format!("{index}.o")), "object").unwrap();
    }
    let project = Project::open(directory.path()).unwrap();
    let status = project.git_status().unwrap().expect("a repository");
    assert_eq!(
        status.ignored,
        std::collections::BTreeSet::from([std::path::PathBuf::from("target")]),
        "an ignored folder is one entry, not one per file"
    );
    assert!(status.is_ignored(Path::new("target/a/b/7.o")));
    assert!(!status.files.keys().any(|path| path.starts_with("target")));
    assert_eq!(
        status.files.get(Path::new(".gitignore")),
        Some(&FileStatus::Untracked)
    );
}

#[test]
fn integration_status_never_rewrites_the_index() {
    let directory = repository();
    // Same content but a newer timestamp: the index entry is only stale, which a plain
    // `git status` would refresh by rewriting `.git/index`.
    let file = fs::File::options()
        .write(true)
        .open(directory.path().join("example.rs"))
        .unwrap();
    file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(60))
        .unwrap();
    drop(file);
    let index = directory.path().join(".git/index");
    let before = fs::metadata(&index).unwrap().modified().unwrap();
    let project = Project::open(directory.path()).unwrap();
    let status = project.git_status().unwrap().expect("a repository");
    assert!(status.files.is_empty(), "{:?}", status.files);
    assert_eq!(fs::metadata(&index).unwrap().modified().unwrap(), before);
}

#[test]
fn regression_a_repositorys_fsmonitor_program_never_runs() {
    // `core.fsmonitor` names a program Git runs on every status; opening a folder must not run
    // code from it.
    let directory = repository();
    let marker = directory.path().join("monitor-ran");
    let script = directory.path().join("monitor.sh");
    fs::write(
        &script,
        "#!/bin/sh\ntouch \"$(dirname \"$0\")/monitor-ran\"\n",
    )
    .unwrap();
    let command = format!("sh '{}'", script.display().to_string().replace('\\', "/"));
    git(directory.path(), &["config", "core.fsmonitor", &command]);
    // The hook works as configured: a plain status runs it.
    Command::new("git")
        .arg("-C")
        .arg(directory.path())
        .arg("status")
        .output()
        .unwrap();
    if !marker.exists() {
        // This Git build ignores hook-style fsmonitor values; nothing to prove.
        return;
    }
    fs::remove_file(&marker).unwrap();
    let project = Project::open(directory.path()).unwrap();
    project.git_status().unwrap();
    project.git_diff(Path::new("example.rs"), false).unwrap();
    assert!(
        !marker.exists(),
        "Orin ran the repository's fsmonitor program"
    );
}
