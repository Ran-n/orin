// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 18:00:39.211800
//+ Revised: 	2026/11/12 09:16:48.362264
// -----------------------------------------------------------------------------

use orin::project::{ProjectError, ProjectWatch};
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

fn wait_for_change(watch: &ProjectWatch) {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if watch.take_changed().unwrap() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "native watcher did not observe the mutation"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn settle(watch: &ProjectWatch) {
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut quiet_since = Instant::now();
    while quiet_since.elapsed() < Duration::from_millis(150) {
        if watch.take_changed().unwrap() {
            quiet_since = Instant::now();
        }
        assert!(
            Instant::now() < deadline,
            "native watcher did not become quiet"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn native_watcher_observes_create_edit_rename_and_delete_in_nested_directories() {
    let directory = TempDir::new().unwrap();
    fs::create_dir(directory.path().join("src")).unwrap();
    let watcher = ProjectWatch::new(directory.path()).unwrap();
    let original = directory.path().join("src/file.rs");
    fs::write(&original, "first").unwrap();
    wait_for_change(&watcher);
    settle(&watcher);
    fs::write(&original, "edited").unwrap();
    wait_for_change(&watcher);
    settle(&watcher);
    let renamed = directory.path().join("src/renamed.rs");
    fs::rename(original, &renamed).unwrap();
    wait_for_change(&watcher);
    settle(&watcher);
    fs::remove_file(renamed).unwrap();
    wait_for_change(&watcher);
    settle(&watcher);
    assert!(!watcher.take_changed().unwrap());
}

#[test]
fn reading_files_and_listing_directories_do_not_trigger_refresh_loops() {
    let directory = TempDir::new().unwrap();
    let file = directory.path().join("file.rs");
    fs::write(&file, "text").unwrap();
    let watcher = ProjectWatch::new(directory.path()).unwrap();
    settle(&watcher);
    for _ in 0..20 {
        assert_eq!(fs::read_to_string(&file).unwrap(), "text");
        fs::read_dir(directory.path())
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        thread::sleep(Duration::from_millis(10));
        assert!(
            !watcher.take_changed().unwrap(),
            "read-only access must not refresh the workspace"
        );
    }
}

#[test]
fn nonexistent_and_non_directory_watch_roots_report_errors() {
    let directory = TempDir::new().unwrap();
    assert!(matches!(
        ProjectWatch::new(&directory.path().join("missing")),
        Err(ProjectError::Io { .. })
    ));
    let file = directory.path().join("file.rs");
    fs::write(&file, "text").unwrap();
    assert!(matches!(
        ProjectWatch::new(&file),
        Err(ProjectError::NotDirectory)
    ));
}

#[test]
fn dropping_a_native_watcher_releases_its_project_directory() {
    let directory = TempDir::new().unwrap();
    let watched = directory.path().join("watched");
    fs::create_dir(&watched).unwrap();
    let watcher = ProjectWatch::new(&watched).unwrap();
    drop(watcher);
    fs::remove_dir(&watched).unwrap();
    assert!(!Path::new(&watched).exists());
}

#[test]
fn generated_artifacts_and_git_objects_are_quiet_but_git_index_refreshes_diffs() {
    let directory = TempDir::new().unwrap();
    for path in [
        "target/debug",
        "node_modules/package",
        ".git/objects",
        ".git/logs",
    ] {
        fs::create_dir_all(directory.path().join(path)).unwrap();
    }
    let watcher = ProjectWatch::new(directory.path()).unwrap();
    settle(&watcher);
    for path in [
        "target/debug/app",
        "node_modules/package/file",
        ".git/objects/object",
        ".git/logs/HEAD",
    ] {
        fs::write(directory.path().join(path), "generated").unwrap();
    }
    for _ in 0..25 {
        thread::sleep(Duration::from_millis(10));
        assert!(
            !watcher.take_changed().unwrap(),
            "generated paths must not starve refresh debounce"
        );
    }
    fs::write(directory.path().join(".git/index"), "new index").unwrap();
    wait_for_change(&watcher);
}
