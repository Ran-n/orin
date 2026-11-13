// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:12:37.137197
//+ Revised: 	2026/11/13 07:53:52.046785
// -----------------------------------------------------------------------------

use std::fs;
use std::path::Path;

use orin::project::{MAX_FILE_BYTES, Project, ProjectError};
use tempfile::TempDir;

#[test]
fn directory_cannot_be_read_as_a_text_document() {
    let directory = TempDir::new().unwrap();
    let project = Project::open(directory.path()).unwrap();
    assert!(matches!(
        project.read_file(Path::new("")),
        Err(ProjectError::Io { .. })
    ));
}

#[cfg(unix)]
#[test]
fn regression_fifo_is_rejected_without_waiting_for_a_writer() {
    // File::open on a FIFO used to block a worker indefinitely, including at shutdown.
    use std::{os::unix::fs::OpenOptionsExt, sync::mpsc, time::Duration};
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("pipe");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let project = Project::open(directory.path()).unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader =
        std::thread::spawn(move || sender.send(project.read_file(Path::new("pipe"))).unwrap());
    let first = receiver.recv_timeout(Duration::from_secs(2));
    let prompt = first.is_ok();
    let result = match first {
        Ok(result) => result,
        Err(_) => {
            // Unblock the old implementation too, so a failing regression leaves no thread.
            drop(
                fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(&path)
                    .unwrap(),
            );
            receiver.recv_timeout(Duration::from_secs(5)).unwrap()
        }
    };
    reader.join().unwrap();
    assert!(prompt, "opening a FIFO waited for a writer");
    assert!(matches!(result, Err(ProjectError::Io { .. })));
}

#[test]
fn opens_canonical_directory_and_lists_directories_first() {
    let directory = TempDir::new().unwrap();
    for name in ["zeta", "alpha", ".git", "target", "build"] {
        fs::create_dir(directory.path().join(name)).unwrap();
    }
    fs::write(directory.path().join("b.rs"), "b").unwrap();
    fs::write(directory.path().join("a.rs"), "a").unwrap();
    let project = Project::open(directory.path()).unwrap();
    assert_eq!(project.root(), directory.path().canonicalize().unwrap());
    let entries = project.list_dir(Path::new("")).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "build", "target", "zeta", "a.rs", "b.rs"],
        "only .git is hidden; build output is listed (and dimmed by the Files column)"
    );
    assert!(entries[0].is_dir);
    assert!(!entries[5].is_dir);
}

#[test]
fn reads_unicode_text_preserving_bytes_and_relative_paths() {
    let directory = TempDir::new().unwrap();
    fs::create_dir(directory.path().join("niño")).unwrap();
    let text = "Orixe e intelixencia 🌱\r\n";
    fs::write(directory.path().join("niño/árbore.rs"), text).unwrap();
    let project = Project::open(directory.path()).unwrap();
    let file = project.read_file(Path::new("./niño/árbore.rs")).unwrap();
    assert_eq!(file.text, text);
    assert_eq!(file.byte_len, text.len());
    assert_eq!(file.relative_path, Path::new("niño/árbore.rs"));
    assert_eq!(
        project.list_dir(Path::new("niño")).unwrap()[0].relative_path,
        file.relative_path
    );
}

#[test]
fn rejects_absolute_paths_and_parent_traversal_for_every_operation() {
    let directory = TempDir::new().unwrap();
    let project = Project::open(directory.path()).unwrap();
    for path in [
        Path::new("../outside"),
        Path::new("sub/../../outside"),
        directory.path(),
    ] {
        assert!(matches!(
            project.read_file(path),
            Err(ProjectError::InvalidPath)
        ));
        assert!(matches!(
            project.list_dir(path),
            Err(ProjectError::InvalidPath)
        ));
        assert!(matches!(
            project.git_diff(path, false),
            Err(ProjectError::InvalidPath)
        ));
    }
}

#[test]
fn reports_missing_files_and_non_directory_roots() {
    let directory = TempDir::new().unwrap();
    let project = Project::open(directory.path()).unwrap();
    assert!(matches!(
        project.read_file(Path::new("missing")),
        Err(ProjectError::Io { .. })
    ));
    assert!(matches!(
        Project::open(&directory.path().join("missing")),
        Err(ProjectError::Io { .. })
    ));
    fs::write(directory.path().join("file"), "text").unwrap();
    assert!(matches!(
        Project::open(&directory.path().join("file")),
        Err(ProjectError::NotDirectory)
    ));
    assert!(matches!(
        project.list_dir(Path::new("file")),
        Err(ProjectError::Io { .. })
    ));
}

#[test]
fn rejects_binary_non_utf8_and_oversized_files() {
    let directory = TempDir::new().unwrap();
    let project = Project::open(directory.path()).unwrap();
    for (name, content) in [
        ("binary", vec![0, 65]),
        ("encoding", vec![255]),
        ("large", vec![65; MAX_FILE_BYTES + 1]),
    ] {
        fs::write(directory.path().join(name), content).unwrap();
    }
    assert!(matches!(
        project.read_file(Path::new("binary")),
        Err(ProjectError::Binary)
    ));
    assert!(matches!(
        project.read_file(Path::new("encoding")),
        Err(ProjectError::Encoding)
    ));
    assert!(matches!(
        project.read_file(Path::new("large")),
        Err(ProjectError::TooLarge)
    ));
    fs::write(directory.path().join("boundary"), vec![65; MAX_FILE_BYTES]).unwrap();
    assert_eq!(
        project.read_file(Path::new("boundary")).unwrap().byte_len,
        MAX_FILE_BYTES
    );
}

#[cfg(unix)]
#[test]
fn external_symlinks_are_hidden_and_cannot_be_read_or_diffed() {
    use std::os::unix::fs::symlink;
    let directory = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::write(outside.path().join("secret"), "private").unwrap();
    symlink(outside.path(), directory.path().join("escape")).unwrap();
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
        project.git_diff(Path::new("escape/secret"), false),
        Err(ProjectError::OutsideRoot)
    ));
}

#[cfg(unix)]
#[test]
fn internal_symlinks_remain_project_relative() {
    use std::os::unix::fs::symlink;
    let directory = TempDir::new().unwrap();
    fs::write(directory.path().join("source"), "text").unwrap();
    symlink("source", directory.path().join("alias")).unwrap();
    let file = Project::open(directory.path())
        .unwrap()
        .read_file(Path::new("alias"))
        .unwrap();
    assert_eq!(file.relative_path, Path::new("alias"));
    assert_eq!(file.text, "text");
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(48))]
    #[test]
    fn text_files_round_trip_without_modification(text in "[^\\x00]{0,256}") {
        let directory = TempDir::new().unwrap();
        fs::write(directory.path().join("text"), &text).unwrap();
        let file = Project::open(directory.path()).unwrap().read_file(Path::new("text")).unwrap();
        proptest::prop_assert_eq!(file.byte_len, text.len());
        proptest::prop_assert_eq!(file.text, text);
    }

    #[test]
    fn parent_components_are_never_resolved(name in "[a-z]{1,16}") {
        let directory = TempDir::new().unwrap();
        let project = Project::open(directory.path()).unwrap();
        let path = format!("{name}/../file");
        proptest::prop_assert!(matches!(project.read_file(Path::new(&path)), Err(ProjectError::InvalidPath)));
    }
}

#[test]
fn regression_a_broken_symlink_does_not_hide_the_folder() {
    let directory = TempDir::new().unwrap();
    fs::write(directory.path().join("kept"), "text").unwrap();
    let link = directory.path().join("dangling");
    #[cfg(unix)]
    std::os::unix::fs::symlink("deleted-target", &link).unwrap();
    #[cfg(windows)]
    if std::os::windows::fs::symlink_file("deleted-target", &link).is_err() {
        // Creating symlinks needs Developer Mode or elevation on Windows; nothing to test then.
        return;
    }
    let entries = Project::open(directory.path())
        .unwrap()
        .list_dir(Path::new(""))
        .unwrap();
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, ["kept"]);
}
