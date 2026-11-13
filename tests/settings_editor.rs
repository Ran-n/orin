// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:45:40.453137
//+ Revised: 	2026/11/12 09:16:53.065003
// -----------------------------------------------------------------------------

use orin::{
    project::MAX_FILE_BYTES,
    settings::{SettingsError, SettingsStore, ViewerMode, WorkspaceSettings},
};
use std::fs;
use tempfile::TempDir;

#[test]
fn existing_schema_one_documents_default_editor_state_to_zero() {
    let settings = WorkspaceSettings::from_json(serde_json::json!({"schema_version": 1})).unwrap();
    assert_eq!(settings.viewer_mode, ViewerMode::File);
    assert_eq!(settings.editor_cursor, 0);
    settings.validate().unwrap();
}

#[test]
fn all_three_viewer_modes_and_cursor_boundaries_validate() {
    for viewer_mode in ViewerMode::ALL {
        for editor_cursor in [0, MAX_FILE_BYTES] {
            let settings = WorkspaceSettings {
                viewer_mode,
                editor_cursor,
                ..WorkspaceSettings::default()
            };
            settings.validate().unwrap();
        }
    }
    for unknown in [r#""diff""#, "3", "null"] {
        let json = format!(r#"{{"schema_version":2,"viewer_mode":{unknown}}}"#);
        assert!(matches!(
            WorkspaceSettings::from_json(serde_json::from_str(&json).unwrap()),
            Err(SettingsError::Json(_))
        ));
    }
    let settings = WorkspaceSettings {
        editor_cursor: MAX_FILE_BYTES + 1,
        ..WorkspaceSettings::default()
    };
    assert!(matches!(
        settings.validate(),
        Err(SettingsError::Invalid("editor cursor"))
    ));
}

#[test]
fn invalid_editor_state_cannot_replace_existing_settings() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    store.save(&WorkspaceSettings::default()).unwrap();
    let original = fs::read(store.path()).unwrap();
    let mut narrow = WorkspaceSettings::default();
    narrow.columns.code.width = 1.0;
    let beyond_file = WorkspaceSettings {
        editor_cursor: MAX_FILE_BYTES + 1,
        ..WorkspaceSettings::default()
    };
    for invalid in [beyond_file, narrow] {
        assert!(store.save(&invalid).is_err());
        assert_eq!(fs::read(store.path()).unwrap(), original);
    }
}
