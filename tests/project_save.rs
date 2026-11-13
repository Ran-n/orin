// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:19.281382
//+ Revised: 	2026/11/12 09:16:48.020323
// -----------------------------------------------------------------------------

use orin::{
    editor::EditorDocument,
    project::{MAX_FILE_BYTES, Project, ProjectError},
};
use std::{fs, path::Path};
use tempfile::TempDir;

fn fixture() -> (TempDir, Project) {
    let directory = TempDir::new().unwrap();
    fs::write(directory.path().join("file.rs"), "before 🌱\n").unwrap();
    let project = Project::open(directory.path()).unwrap();
    (directory, project)
}

#[test]
fn atomic_save_updates_snapshot_and_supports_a_second_save() {
    let (directory, project) = fixture();
    let original = project.read_file(Path::new("file.rs")).unwrap();
    let saved = project.save_file(&original, "after é\n").unwrap();
    assert_eq!(saved.text, "after é\n");
    assert_eq!(saved.byte_len, saved.text.len());
    assert_eq!(
        fs::read_to_string(directory.path().join("file.rs")).unwrap(),
        saved.text
    );
    assert_eq!(
        project.read_file(Path::new("file.rs")).unwrap().version,
        saved.version
    );
    assert_ne!(saved.version, original.version);
    let saved_again = project.save_file(&saved, "third\n").unwrap();
    assert_eq!(saved_again.text, "third\n");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn stale_agent_changes_are_preserved_and_unsaved_buffer_is_kept() {
    let (directory, project) = fixture();
    let original = project.read_file(Path::new("file.rs")).unwrap();
    let mut document = EditorDocument::from_file(original);
    let length = document.buffer().text().len();
    document.buffer_mut().replace(0..length, "manual").unwrap();
    fs::write(directory.path().join("file.rs"), "agent change").unwrap();
    assert!(matches!(
        project.save_file(document.original(), document.buffer().text()),
        Err(ProjectError::Conflict)
    ));
    assert_eq!(
        fs::read_to_string(directory.path().join("file.rs")).unwrap(),
        "agent change"
    );
    assert_eq!(document.buffer().text(), "manual");
    assert!(document.is_dirty());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn deleted_or_replaced_files_cannot_be_silently_recreated_or_overwritten() {
    let (directory, project) = fixture();
    let original = project.read_file(Path::new("file.rs")).unwrap();
    let path = directory.path().join("file.rs");
    fs::rename(&path, directory.path().join("renamed.rs")).unwrap();
    assert!(matches!(
        project.save_file(&original, "manual"),
        Err(ProjectError::Conflict)
    ));
    assert!(!path.exists());
    fs::write(&path, &original.text).unwrap();
    assert!(matches!(
        project.save_file(&original, "manual"),
        Err(ProjectError::Conflict)
    ));
    assert_eq!(fs::read_to_string(path).unwrap(), original.text);
}

#[test]
fn invalid_replacements_and_readonly_files_leave_original_unchanged() {
    let (directory, project) = fixture();
    let original = project.read_file(Path::new("file.rs")).unwrap();
    assert!(matches!(
        project.save_file(&original, &"x".repeat(MAX_FILE_BYTES + 1)),
        Err(ProjectError::TooLarge)
    ));
    assert!(matches!(
        project.save_file(&original, "bad\0text"),
        Err(ProjectError::Binary)
    ));
    let path = directory.path().join("file.rs");
    let original_permissions = fs::metadata(&path).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    assert!(matches!(
        project.save_file(&original, "manual"),
        Err(ProjectError::Io { .. })
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), original.text);
    fs::set_permissions(path, original_permissions).unwrap();
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn atomic_save_preserves_unix_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let (directory, project) = fixture();
    let path = directory.path().join("file.rs");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let original = project.read_file(Path::new("file.rs")).unwrap();
    project.save_file(&original, "manual").unwrap();
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[cfg(unix)]
#[test]
fn retargeted_symlinks_are_conflicts_even_with_identical_bytes() {
    use std::os::unix::fs::symlink;
    let (directory, project) = fixture();
    fs::write(directory.path().join("other.rs"), "before 🌱\n").unwrap();
    let alias = directory.path().join("alias.rs");
    symlink("file.rs", &alias).unwrap();
    let original = project.read_file(Path::new("alias.rs")).unwrap();
    fs::remove_file(&alias).unwrap();
    symlink("other.rs", &alias).unwrap();
    assert!(matches!(
        project.save_file(&original, "manual"),
        Err(ProjectError::Conflict)
    ));
    assert_eq!(
        fs::read_to_string(directory.path().join("other.rs")).unwrap(),
        original.text
    );
}

#[cfg(windows)]
#[test]
fn retargeted_directory_junctions_are_conflicts_even_with_identical_bytes() {
    use std::os::windows::process::CommandExt;
    let directory = TempDir::new().unwrap();
    for name in ["first", "second"] {
        fs::create_dir(directory.path().join(name)).unwrap();
        fs::write(directory.path().join(name).join("file"), "same").unwrap();
    }
    let alias = directory.path().join("alias");
    let make_junction = |target: &str| {
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&alias)
            .arg(directory.path().join(target))
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(output.status.success());
    };
    make_junction("first");
    let project = Project::open(directory.path()).unwrap();
    let original = project.read_file(Path::new("alias/file")).unwrap();
    fs::remove_dir(&alias).unwrap();
    make_junction("second");
    assert!(matches!(
        project.save_file(&original, "manual"),
        Err(ProjectError::Conflict)
    ));
    assert_eq!(
        fs::read_to_string(directory.path().join("second/file")).unwrap(),
        "same"
    );
}

#[cfg(windows)]
#[test]
fn failed_atomic_replace_preserves_original_and_cleans_temporary_file() {
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};
    let (directory, project) = fixture();
    let path = directory.path().join("file.rs");
    let original = project.read_file(Path::new("file.rs")).unwrap();
    // Allow the two stale-version reads, but refuse deletion/rename at atomic commit.
    let _lock = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    assert!(matches!(
        project.save_file(&original, "manual"),
        Err(ProjectError::Io { .. })
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), original.text);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn saving_through_internal_symlinks_preserves_the_link() {
    use std::os::unix::fs::symlink;
    let (directory, project) = fixture();
    let alias = directory.path().join("alias.rs");
    symlink("file.rs", &alias).unwrap();
    let original = project.read_file(Path::new("alias.rs")).unwrap();
    let saved = project.save_file(&original, "manual").unwrap();
    assert!(
        fs::symlink_metadata(alias)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(saved.relative_path, Path::new("alias.rs"));
    assert_eq!(
        fs::read_to_string(directory.path().join("file.rs")).unwrap(),
        "manual"
    );
}
