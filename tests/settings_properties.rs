// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:12:37.516558
//+ Revised: 	2026/11/12 22:43:40.498296
// -----------------------------------------------------------------------------

use std::path::PathBuf;

use orin::settings::{Column, SettingsStore, ViewerMode, WorkspaceSettings};
use proptest::prelude::*;
use tempfile::TempDir;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn valid_settings_survive_serialization_and_atomic_restore(
        width in 640_u16..7680,
        height in 480_u16..4320,
        session in 0_usize..100,
        viewer_mode in 0_usize..3,
        editor_cursor in 0_usize..=orin::project::MAX_FILE_BYTES,
        collapsed in proptest::array::uniform4(any::<bool>()),
        extra_width in proptest::array::uniform4(0_u16..2000),
        name in "[a-z]{1,20}",
    ) {
        let mut settings = WorkspaceSettings {
            width: f32::from(width),
            height: f32::from(height),
            active_session: session,
            viewer_mode: ViewerMode::ALL[viewer_mode],
            editor_cursor,
            selected_file: Some(PathBuf::from(format!("src/{name}.rs"))),
            ..WorkspaceSettings::default()
        };
        for (index, column) in Column::ALL.into_iter().enumerate() {
            let state = settings.columns.get_mut(column);
            state.collapsed = collapsed[index];
            state.width = column.min_width() + f32::from(extra_width[index]);
        }
        let serialized = serde_json::to_string(&settings).unwrap();
        let decoded: WorkspaceSettings = serde_json::from_str(&serialized).unwrap();
        prop_assert_eq!(&decoded, &settings);
        let directory = TempDir::new().unwrap();
        let store = SettingsStore::new(directory.path().join("workspace.json"));
        store.save(&settings).unwrap();
        prop_assert_eq!(store.load().unwrap(), settings);
    }

    #[test]
    fn invalid_dimensions_never_validate(width in -100000_f32..=0.0_f32) {
        let settings = WorkspaceSettings { width, ..WorkspaceSettings::default() };
        prop_assert!(settings.validate().is_err());
    }

    #[test]
    fn skipped_version_validates_exactly_when_it_is_short_semver(text in "[0-9a-z.+-]{0,80}") {
        let mut settings = WorkspaceSettings::default();
        settings.updates.skipped_version = Some(text.clone());
        let expected = text.len() <= 64 && semver::Version::parse(&text).is_ok();
        prop_assert_eq!(settings.validate().is_ok(), expected);
    }

    #[test]
    fn real_versions_are_accepted_as_skipped(major in 0u64..100, minor in 0u64..100, patch in 0u64..100) {
        let mut settings = WorkspaceSettings::default();
        settings.updates.skipped_version = Some(format!("{major}.{minor}.{patch}"));
        prop_assert!(settings.validate().is_ok());
    }
}
