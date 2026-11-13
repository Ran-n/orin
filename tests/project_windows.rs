// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:42:07.279570
//+ Revised: 	2026/11/12 09:16:48.697729
// -----------------------------------------------------------------------------

#![cfg(windows)]

use orin::project::{Project, ProjectError};
use std::{fs, path::Path};
use tempfile::TempDir;
#[cfg(windows)]
#[test]
fn files_locked_by_another_application_report_io_errors() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("locked");
    fs::write(&path, "text").unwrap();
    let _lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let project = Project::open(directory.path()).unwrap();
    assert!(matches!(
        project.read_file(Path::new("locked")),
        Err(ProjectError::Io { .. })
    ));
}

#[cfg(windows)]
fn junction(link: &Path, target: &Path) {
    use std::os::windows::process::CommandExt;
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .creation_flags(0x0800_0000)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(windows)]
#[test]
fn external_junctions_are_excluded_and_cannot_escape_the_root() {
    let directory = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::write(outside.path().join("secret"), "private").unwrap();
    junction(&directory.path().join("escape"), outside.path());
    let project = Project::open(directory.path()).unwrap();
    assert!(project.list_dir(Path::new("")).unwrap().is_empty());
    assert!(matches!(
        project.list_dir(Path::new("escape")),
        Err(ProjectError::OutsideRoot)
    ));
    assert!(matches!(
        project.read_file(Path::new("escape/secret")),
        Err(ProjectError::OutsideRoot)
    ));
    assert!(matches!(
        project.git_diff(Path::new("escape/missing"), false),
        Err(ProjectError::OutsideRoot)
    ));
}

#[cfg(windows)]
#[test]
fn internal_junctions_keep_alias_paths_in_the_explorer() {
    let directory = TempDir::new().unwrap();
    fs::create_dir(directory.path().join("source")).unwrap();
    fs::write(directory.path().join("source/file"), "text").unwrap();
    junction(
        &directory.path().join("alias"),
        &directory.path().join("source"),
    );
    let project = Project::open(directory.path()).unwrap();
    let file = project.read_file(Path::new("alias/file")).unwrap();
    assert_eq!(file.relative_path, Path::new("alias/file"));
    assert_eq!(
        project.list_dir(Path::new("alias")).unwrap()[0].relative_path,
        file.relative_path
    );
}
