// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:58:14.099106
//+ Revised: 	2026/11/12 09:16:47.656814
// -----------------------------------------------------------------------------

use orin::project::{Project, ProjectError};
use std::{fs, path::Path};
use tempfile::TempDir;

#[test]
fn agent_absolute_and_relative_file_paths_resolve_to_the_same_project_file() {
    let directory = TempDir::new().unwrap();
    fs::create_dir(directory.path().join("src")).unwrap();
    let file = directory.path().join("src/árbore.rs");
    fs::write(&file, "🌱").unwrap();
    let project = Project::open(directory.path()).unwrap();
    for reported in [
        file.as_path(),
        Path::new("src/árbore.rs"),
        Path::new("./src/árbore.rs"),
    ] {
        assert_eq!(
            project.reported_file(reported).unwrap(),
            Path::new("src/árbore.rs")
        );
    }
}

#[test]
fn agent_paths_cannot_open_outside_files_or_parent_traversal() {
    let directory = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let file = outside.path().join("outside.rs");
    fs::write(&file, "outside").unwrap();
    let project = Project::open(directory.path()).unwrap();
    assert!(matches!(
        project.reported_file(&file),
        Err(ProjectError::OutsideRoot)
    ));
    assert!(matches!(
        project.reported_file(Path::new("../outside.rs")),
        Err(ProjectError::InvalidPath)
    ));
}

#[test]
fn missing_deleted_and_directory_reports_surface_io_errors() {
    let directory = TempDir::new().unwrap();
    let project = Project::open(directory.path()).unwrap();
    fs::write(directory.path().join("deleted.rs"), "before").unwrap();
    fs::remove_file(directory.path().join("deleted.rs")).unwrap();
    for reported in [
        Path::new("missing.rs"),
        Path::new("deleted.rs"),
        Path::new(""),
    ] {
        assert!(matches!(
            project.reported_file(reported),
            Err(ProjectError::Io { .. })
        ));
    }
}

#[cfg(windows)]
#[test]
fn non_extended_windows_agent_paths_match_the_extended_canonical_project_root() {
    let directory = TempDir::new().unwrap();
    let file = directory.path().join("file.rs");
    fs::write(&file, "text").unwrap();
    let project = Project::open(directory.path()).unwrap();
    let canonical = file.canonicalize().unwrap();
    let native = canonical.to_str().unwrap().strip_prefix(r"\\?\").unwrap();
    // Regression: stripping a non-extended agent path against a \\?\ root failed.
    assert_eq!(
        project.reported_file(Path::new(native)).unwrap(),
        Path::new("file.rs")
    );
    assert_eq!(
        project.reported_file(&canonical).unwrap(),
        Path::new("file.rs")
    );
}

#[cfg(unix)]
#[test]
fn reported_symlink_files_resolve_inside_the_root_and_reject_external_targets() {
    use std::os::unix::fs::symlink;
    let directory = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::write(directory.path().join("source.rs"), "inside").unwrap();
    fs::write(outside.path().join("private.rs"), "outside").unwrap();
    symlink("source.rs", directory.path().join("alias.rs")).unwrap();
    symlink(
        outside.path().join("private.rs"),
        directory.path().join("escape.rs"),
    )
    .unwrap();
    let project = Project::open(directory.path()).unwrap();
    assert_eq!(
        project.reported_file(Path::new("alias.rs")).unwrap(),
        Path::new("source.rs")
    );
    assert!(matches!(
        project.reported_file(Path::new("escape.rs")),
        Err(ProjectError::OutsideRoot)
    ));
}

#[cfg(windows)]
#[test]
fn agent_reports_through_external_junctions_cannot_escape_the_project() {
    use std::os::windows::process::CommandExt;
    let directory = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::write(outside.path().join("file.rs"), "outside").unwrap();
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(directory.path().join("escape"))
        .arg(outside.path())
        .creation_flags(0x0800_0000)
        .output()
        .unwrap();
    assert!(output.status.success());
    let project = Project::open(directory.path()).unwrap();
    assert!(matches!(
        project.reported_file(Path::new("escape/file.rs")),
        Err(ProjectError::OutsideRoot)
    ));
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(48))]
    #[test]
    fn reported_paths_always_resolve_back_to_the_exact_project_file(name in "[a-z]{1,20}") {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join(format!("{name}.rs"));
        fs::write(&path, "text").unwrap();
        let project = Project::open(directory.path()).unwrap();
        let relative = project.reported_file(&path).unwrap();
        proptest::prop_assert!(!relative.is_absolute());
        proptest::prop_assert_eq!(project.root().join(&relative).canonicalize().unwrap(), path.canonicalize().unwrap());
        proptest::prop_assert_eq!(project.reported_file(&relative).unwrap(), relative);
    }
}
