// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:12:37.389561
//+ Revised: 	2026/11/13 07:53:52.244100
// -----------------------------------------------------------------------------

use std::fs;
use std::path::PathBuf;

use orin::settings::{
    Appearance, Column, MAX_AGENT_OPTION_TEXT, MAX_AGENT_OPTIONS, MAX_RECENT_PROJECTS, Page,
    SettingsError, SettingsSection, SettingsStore, Theme, Updates, ViewerMode, WorkspaceSettings,
};
use tempfile::TempDir;

fn fixed_settings() -> WorkspaceSettings {
    let mut settings = WorkspaceSettings {
        selected_file: Some(PathBuf::from("src/main.rs")),
        viewer_mode: ViewerMode::Staged,
        markdown_preview: true,
        editor_cursor: 17,
        selected_provider: "gemini".into(),
        active_session: 2,
        width: 1440.0,
        height: 900.0,
        position: Some((50.0, -12.0)),
        expanded_folders: vec![PathBuf::from("src"), PathBuf::from("docs")],
        ..WorkspaceSettings::default()
    };
    settings.columns.files.width = 260.0;
    settings.columns.code.width = 380.0;
    settings.columns.terminal.collapsed = true;
    settings.columns.move_column(Column::Terminal, 1);
    settings.appearance = Appearance {
        theme: Theme::GruvboxLight,
        text_size: 15.0,
        interface_font: Some("Inter".into()),
        code_font: Some("JetBrains Mono".into()),
    };
    settings.page = Some(Page::Settings(SettingsSection::Accounts));
    settings.updates = Updates {
        check_automatically: false,
        last_check: Some(1_795_000_000),
        skipped_version: Some("0.3.0".into()),
    };
    settings
        .agent_options
        .entry("claude".into())
        .or_default()
        .insert("model".into(), "opus".into());
    settings
}

#[test]
fn contract_viewer_column_and_theme_labels_have_stable_identifiers() {
    for (mode, key, label) in [
        (ViewerMode::File, "file", "File"),
        (ViewerMode::Unstaged, "unstaged", "Unstaged"),
        (ViewerMode::Staged, "staged", "Staged"),
    ] {
        assert_eq!(mode.key(), key);
        assert_eq!(mode.label(), label);
        assert_eq!(serde_json::to_value(mode).unwrap(), key);
    }
    for (column, key, title) in [
        (Column::Files, "files", "Files"),
        (Column::Code, "code", "Code"),
        (Column::Agents, "agents", "Agents"),
        (Column::Terminal, "terminal", "Terminal"),
    ] {
        assert_eq!(column.key(), key);
        assert_eq!(column.title(), title);
        assert_eq!(serde_json::to_value(column).unwrap(), key);
    }
    for (theme, dark) in [
        (Theme::GruvboxDark, true),
        (Theme::Dark, true),
        (Theme::GruvboxLight, false),
        (Theme::Light, false),
    ] {
        assert_eq!(theme.is_dark(), dark);
    }
}

fn golden(text: &str) -> String {
    text.trim_start_matches('\u{feff}').replace("\r\n", "\n")
}

#[test]
fn missing_file_uses_defaults_without_creating_files() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("missing/workspace.json"));
    assert_eq!(store.load().unwrap(), WorkspaceSettings::default());
    assert!(!store.path().exists());
    assert!(
        SettingsStore::default_path()
            .unwrap()
            .ends_with("workspace.json")
    );
}

#[test]
fn saves_exact_golden_schema_and_restores_into_fresh_store() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("nested/workspace.json"));
    let settings = fixed_settings();
    store.save(&settings).unwrap();
    let actual = fs::read_to_string(store.path())
        .unwrap()
        .replace("\r\n", "\n");
    let golden = golden(include_str!("golden/workspace-v2.json"));
    assert_eq!(actual, golden);
    let parsed: WorkspaceSettings = serde_json::from_str(&golden).unwrap();
    assert_eq!(parsed, settings);
    let restored = SettingsStore::new(directory.path().join("fresh/workspace.json"));
    restored.save(&parsed).unwrap();
    assert_eq!(restored.load().unwrap(), settings);
}

#[test]
fn updates_default_on_and_round_trip() {
    let defaults = WorkspaceSettings::default().updates;
    assert!(
        defaults.check_automatically,
        "update checks are on by default"
    );
    assert_eq!(defaults.last_check, None);
    assert_eq!(defaults.skipped(), None);
    // A schema 2 file from before Settings › Updates existed loads with the defaults.
    let mut value: serde_json::Value =
        serde_json::from_str(&golden(include_str!("golden/workspace-v2.json"))).unwrap();
    value.as_object_mut().unwrap().remove("updates");
    let older = WorkspaceSettings::from_json(value).unwrap();
    assert_eq!(older.updates, Updates::default());
    assert_eq!(older.selected_file, fixed_settings().selected_file);
    // Choices survive a save and a load.
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    let settings = fixed_settings();
    store.save(&settings).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.updates, settings.updates);
    assert_eq!(
        loaded.updates.skipped(),
        Some(semver::Version::new(0, 3, 0))
    );
}

#[test]
fn invalid_skipped_versions_are_rejected() {
    for version in ["latest", "", "1.2", &"9".repeat(80)] {
        let mut settings = WorkspaceSettings::default();
        settings.updates.skipped_version = Some(version.to_owned());
        assert!(
            matches!(settings.validate(), Err(SettingsError::Invalid(_))),
            "{version:?}"
        );
    }
}

#[test]
fn absent_fields_use_defaults_and_unknown_fields_are_ignored() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    fs::write(
        store.path(),
        r#"{"schema_version":1,"future_optional":"safe"}"#,
    )
    .unwrap();
    assert_eq!(store.load().unwrap(), WorkspaceSettings::default());
}

#[test]
fn incompatible_or_corrupt_settings_are_preserved_on_load_and_save() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    for contents in [
        r#"{"schema_version":3}"#,
        r#"{"schema_version":0}"#,
        r#"{"schema_version":1,"viewer_mode":7}"#,
        r#"{"schema_version":1,"terminal_visible":"no"}"#,
        r#"{"schema_version":2,"columns":{"files":{"width":10.0,"collapsed":false}}}"#,
        "not json",
    ] {
        fs::write(store.path(), contents).unwrap();
        assert!(store.load().is_err());
        assert!(store.save(&WorkspaceSettings::default()).is_err());
        assert_eq!(fs::read_to_string(store.path()).unwrap(), contents);
    }
    fs::write(store.path(), r#"{"schema_version":99}"#).unwrap();
    assert!(matches!(
        store.load(),
        Err(SettingsError::UnsupportedSchema(99))
    ));
}

#[test]
fn no_op_save_preserves_existing_formatting_and_leaves_no_temporary_files() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    // A current-schema file that already holds these settings is left byte-for-byte as it is.
    fs::write(store.path(), "{\"schema_version\":2}\n").unwrap();
    store.save(&WorkspaceSettings::default()).unwrap();
    assert_eq!(
        fs::read_to_string(store.path()).unwrap(),
        "{\"schema_version\":2}\n"
    );
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    store.save(&fixed_settings()).unwrap();
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn oversized_files_and_directory_paths_report_errors() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    fs::write(store.path(), vec![b' '; 1024 * 1024 + 1]).unwrap();
    assert!(matches!(store.load(), Err(SettingsError::Invalid(_))));
    let directory_store = SettingsStore::new(directory.path());
    assert!(directory_store.load().is_err());
    assert!(directory_store.save(&WorkspaceSettings::default()).is_err());
    assert!(directory.path().is_dir());
}

#[test]
fn regression_oversized_save_preserves_readable_settings_and_leaves_no_temp_files() {
    // Saving used to succeed with a file that the next load refused as too large.
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    store.save(&fixed_settings()).unwrap();
    let original = fs::read(store.path()).unwrap();
    let settings = WorkspaceSettings {
        expanded_folders: vec![PathBuf::from("folder"); 100_000],
        ..WorkspaceSettings::default()
    };
    assert!(matches!(
        store.save(&settings),
        Err(SettingsError::Invalid(_))
    ));
    assert_eq!(fs::read(store.path()).unwrap(), original);
    assert_eq!(store.load().unwrap(), fixed_settings());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    let fresh = SettingsStore::new(directory.path().join("absent.json"));
    assert!(fresh.save(&settings).is_err());
    assert!(!fresh.path().exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn contract_settings_save_and_load_share_the_exact_serialized_size_boundary() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    let mut settings = WorkspaceSettings {
        expanded_folders: vec![PathBuf::new()],
        ..WorkspaceSettings::default()
    };
    let overhead = serde_json::to_vec_pretty(&settings).unwrap().len() + 1;
    let name = "x".repeat(1024 * 1024 - overhead);
    settings.expanded_folders = vec![PathBuf::from(&name)];
    store.save(&settings).unwrap();
    assert_eq!(fs::metadata(store.path()).unwrap().len(), 1024 * 1024);
    assert_eq!(store.load().unwrap(), settings);
    let original = fs::read(store.path()).unwrap();
    settings.expanded_folders = vec![PathBuf::from(format!("{name}x"))];
    assert!(store.save(&settings).is_err());
    assert_eq!(fs::read(store.path()).unwrap(), original);
}

#[test]
fn invalid_new_settings_leave_existing_file_untouched() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    store.save(&fixed_settings()).unwrap();
    let original = fs::read(store.path()).unwrap();
    let invalid = WorkspaceSettings {
        width: f32::NAN,
        ..fixed_settings()
    };
    assert!(matches!(
        store.save(&invalid),
        Err(SettingsError::Invalid(_))
    ));
    assert_eq!(fs::read(store.path()).unwrap(), original);
}

#[test]
fn regression_an_empty_selected_file_is_rejected() {
    // "" and "." name the project folder, which the editor can't open as a file.
    for path in ["", "."] {
        let settings = WorkspaceSettings {
            selected_file: Some(PathBuf::from(path)),
            ..WorkspaceSettings::default()
        };
        assert!(
            matches!(settings.validate(), Err(SettingsError::Invalid(_))),
            "{path:?}"
        );
    }
}

#[test]
fn invalid_paths_providers_window_dimensions_and_schema_are_rejected() {
    let mut settings = WorkspaceSettings {
        selected_file: Some(PathBuf::from("../secret")),
        ..WorkspaceSettings::default()
    };
    assert!(settings.validate().is_err());
    settings.selected_file = None;
    settings.expanded_folders = vec![PathBuf::from("src/../../outside")];
    assert!(settings.validate().is_err());
    settings.expanded_folders.clear();
    settings.selected_provider = "unknown".into();
    assert!(settings.validate().is_err());
    settings.selected_provider = "claude".into();
    settings.position = Some((f32::INFINITY, 0.0));
    assert!(settings.validate().is_err());
    settings.position = None;
    settings.height = -1.0;
    assert!(settings.validate().is_err());
    settings.height = 800.0;
    settings.schema_version = 9;
    assert!(settings.validate().is_err());
}

#[test]
fn atomic_replacement_never_exposes_partial_json_to_readers() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    store.save(&WorkspaceSettings::default()).unwrap();
    let reader = store.clone();
    let handle = std::thread::spawn(move || {
        for _ in 0..100 {
            reader.load().unwrap();
        }
    });
    for active_session in 0..25 {
        store
            .save(&WorkspaceSettings {
                active_session,
                ..WorkspaceSettings::default()
            })
            .unwrap();
    }
    handle.join().unwrap();
    assert_eq!(store.load().unwrap().active_session, 24);
}

#[cfg(windows)]
#[test]
fn failed_atomic_replace_preserves_original_and_cleans_temporary_file() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    store.save(&WorkspaceSettings::default()).unwrap();
    let original = fs::read(store.path()).unwrap();
    let original_permissions = fs::metadata(store.path()).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(store.path(), permissions.clone()).unwrap();
    assert!(store.save(&fixed_settings()).is_err());
    assert_eq!(fs::read(store.path()).unwrap(), original);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    fs::set_permissions(store.path(), original_permissions).unwrap();
}

#[test]
fn migration_upgrades_the_v1_golden_file_without_losing_choices() {
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    let v1 = golden(include_str!("golden/workspace-v1.json"));
    fs::write(store.path(), &v1).unwrap();
    let migrated = store.load().unwrap();
    assert_eq!(migrated.schema_version, 2);
    assert_eq!(migrated.viewer_mode, ViewerMode::Staged);
    assert_eq!(migrated.selected_file, Some(PathBuf::from("src/main.rs")));
    assert_eq!(migrated.editor_cursor, 17);
    assert_eq!(migrated.selected_provider, "gemini");
    assert_eq!(migrated.active_session, 2);
    assert_eq!((migrated.width, migrated.height), (1440.0, 900.0));
    assert_eq!(migrated.position, Some((50.0, -12.0)));
    assert_eq!(
        migrated.expanded_folders,
        vec![PathBuf::from("src"), PathBuf::from("docs")]
    );
    // The hidden v1 terminal becomes a collapsed column; every other column stays open.
    for column in Column::ALL {
        assert_eq!(
            migrated.columns.get(column).collapsed,
            column == Column::Terminal,
            "{column:?}"
        );
    }
    // Loading never rewrites the file; the first save writes schema 2.
    assert_eq!(golden(&fs::read_to_string(store.path()).unwrap()), v1);
    store.save(&migrated).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(store.path()).unwrap()).unwrap();
    assert_eq!(saved["schema_version"], 2);
    assert!(saved.get("active_pane").is_none() && saved.get("terminal_visible").is_none());
    assert_eq!(store.load().unwrap(), migrated);
}

#[test]
fn migration_maps_every_v1_viewer_mode_and_terminal_state() {
    for (index, mode) in ViewerMode::ALL.into_iter().enumerate() {
        for visible in [true, false] {
            let json = serde_json::json!({
                "schema_version": 1,
                "viewer_mode": index,
                "terminal_visible": visible,
                "active_pane": "agents",
            });
            let settings = WorkspaceSettings::from_json(json).unwrap();
            assert_eq!(settings.viewer_mode, mode);
            assert_eq!(settings.columns.terminal.collapsed, !visible);
        }
    }
}

#[test]
fn column_widths_below_minimum_or_unbounded_are_rejected() {
    for column in Column::ALL {
        for width in [column.min_width() - 1.0, f32::NAN, f32::INFINITY, 1.0e6] {
            let mut settings = WorkspaceSettings::default();
            settings.columns.get_mut(column).width = width;
            assert!(
                matches!(
                    settings.validate(),
                    Err(SettingsError::Invalid("column width"))
                ),
                "{column:?} {width}"
            );
        }
        let mut settings = WorkspaceSettings::default();
        settings.columns.get_mut(column).width = column.min_width();
        settings.validate().unwrap();
    }
}

#[test]
fn early_v2_files_without_order_or_appearance_keep_the_defaults() {
    let json = serde_json::json!({
        "schema_version": 2,
        "columns": {"files": {"width": 250.0, "collapsed": false}},
    });
    let settings = WorkspaceSettings::from_json(json).unwrap();
    assert_eq!(settings.columns.order, Column::ALL);
    assert_eq!(settings.columns.files.width, 250.0);
    assert_eq!(settings.appearance, Appearance::default());
}

#[test]
fn column_order_must_name_every_column_once() {
    let mut settings = WorkspaceSettings::default();
    settings.columns.order = [Column::Files, Column::Files, Column::Code, Column::Terminal];
    assert!(matches!(
        settings.validate(),
        Err(SettingsError::Invalid("column order"))
    ));
    let json = serde_json::json!({
        "schema_version": 2,
        "columns": {"order": ["files", "agents", "code", "sidebar"]},
    });
    assert!(WorkspaceSettings::from_json(json).is_err());
}

#[test]
fn moving_a_column_reorders_without_losing_any() {
    let mut settings = WorkspaceSettings::default();
    settings.columns.move_column(Column::Files, 3);
    assert_eq!(
        settings.columns.order,
        [
            Column::Code,
            Column::Agents,
            Column::Terminal,
            Column::Files
        ]
    );
    settings.columns.move_column(Column::Terminal, 99);
    assert_eq!(
        settings.columns.order,
        [
            Column::Code,
            Column::Agents,
            Column::Files,
            Column::Terminal
        ]
    );
    settings.columns.move_column(Column::Terminal, 0);
    assert_eq!(settings.columns.order[0], Column::Terminal);
    settings.validate().unwrap();
}

#[test]
fn text_size_steps_stay_within_range_and_invalid_sizes_are_rejected() {
    let mut appearance = Appearance::default();
    for _ in 0..20 {
        appearance.step_text_size(1.0);
    }
    assert_eq!(appearance.text_size, Appearance::MAX_TEXT_SIZE);
    for _ in 0..20 {
        appearance.step_text_size(-1.0);
    }
    assert_eq!(appearance.text_size, Appearance::MIN_TEXT_SIZE);
    assert_eq!(Appearance::default().scale(), 1.0);
    for size in [9.0, 21.0, f32::NAN, f32::INFINITY] {
        let mut settings = WorkspaceSettings::default();
        settings.appearance.text_size = size;
        assert!(
            matches!(
                settings.validate(),
                Err(SettingsError::Invalid("text size"))
            ),
            "{size}"
        );
    }
}

#[test]
fn gruvbox_dark_is_the_default_and_every_theme_has_a_stable_key() {
    assert_eq!(
        WorkspaceSettings::default().appearance.theme,
        Theme::GruvboxDark
    );
    for theme in Theme::ALL {
        let key = serde_json::to_value(theme).unwrap();
        assert_eq!(key, serde_json::json!(theme.key()));
        assert_eq!(serde_json::from_value::<Theme>(key).unwrap(), theme);
        assert!(!theme.label().is_empty());
    }
    // Files saved before the theme chooser keep the theme they had.
    let json = serde_json::json!({"schema_version": 2, "appearance": {"theme": "light"}});
    let settings = WorkspaceSettings::from_json(json).unwrap();
    assert_eq!(settings.appearance.theme, Theme::Light);
    assert_eq!(settings.appearance.interface_font, None);
}

#[test]
fn font_names_must_be_plain_and_bounded() {
    let long = "F".repeat(Appearance::MAX_FONT_NAME + 1);
    for name in ["", "   ", "Bad\nName", long.as_str()] {
        let mut settings = WorkspaceSettings::default();
        settings.appearance.code_font = Some(name.to_owned());
        assert!(
            matches!(
                settings.validate(),
                Err(SettingsError::Invalid("font name"))
            ),
            "{name:?}"
        );
    }
    let mut settings = WorkspaceSettings::default();
    settings.appearance.interface_font = Some("Segoe UI Variable".into());
    settings.appearance.code_font = Some("Cascadia Code".into());
    settings.validate().unwrap();
}

#[test]
fn pages_round_trip_welcome_and_every_settings_section() {
    for section in SettingsSection::ALL {
        let json = serde_json::json!({"schema_version": 2, "page": {"settings": section.key()}});
        let settings = WorkspaceSettings::from_json(json).unwrap();
        assert_eq!(settings.page, Some(Page::Settings(section)));
        assert_eq!(settings.settings_section(), Some(section));
        assert!(!section.title().is_empty());
    }
    let json = serde_json::json!({"schema_version": 2, "page": "welcome"});
    let welcome = WorkspaceSettings::from_json(json).unwrap();
    assert_eq!(welcome.page, Some(Page::Welcome));
    assert_eq!(welcome.settings_section(), None);
    assert_eq!(WorkspaceSettings::default().page, None);
}

fn absolute(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

#[test]
fn recent_projects_put_the_newest_first_without_duplicates_and_keep_a_few() {
    let mut settings = WorkspaceSettings::default();
    settings.remember_project(&absolute("a"));
    settings.remember_project(&absolute("b"));
    settings.remember_project(&absolute("a"));
    assert_eq!(settings.recent_projects, vec![absolute("a"), absolute("b")]);
    // A relative path is not a project location and is not remembered.
    settings.remember_project(std::path::Path::new("relative"));
    assert_eq!(settings.recent_projects.len(), 2);
    for index in 0..MAX_RECENT_PROJECTS + 3 {
        settings.remember_project(&absolute(&format!("p{index}")));
    }
    assert_eq!(settings.recent_projects.len(), MAX_RECENT_PROJECTS);
    assert_eq!(
        settings.recent_projects[0],
        absolute(&format!("p{}", MAX_RECENT_PROJECTS + 2))
    );
    settings.validate().unwrap();
    // Saved and loaded back unchanged.
    let directory = TempDir::new().unwrap();
    let store = SettingsStore::new(directory.path().join("workspace.json"));
    store.save(&settings).unwrap();
    assert_eq!(
        store.load().unwrap().recent_projects,
        settings.recent_projects
    );
}

#[test]
fn damaged_recent_projects_are_rejected() {
    let relative = WorkspaceSettings {
        recent_projects: vec![PathBuf::from("relative")],
        ..WorkspaceSettings::default()
    };
    let too_many = WorkspaceSettings {
        recent_projects: (0..=MAX_RECENT_PROJECTS)
            .map(|i| absolute(&i.to_string()))
            .collect(),
        ..WorkspaceSettings::default()
    };
    for settings in [relative, too_many] {
        assert!(matches!(
            settings.validate(),
            Err(SettingsError::Invalid("recent projects"))
        ));
    }
}

#[test]
fn agent_options_are_bounded_and_only_for_known_agents() {
    let valid = fixed_settings();
    valid.validate().unwrap();
    let with = |provider: &str, id: String, value: String| {
        let mut settings = WorkspaceSettings::default();
        settings
            .agent_options
            .entry(provider.into())
            .or_default()
            .insert(id, value);
        settings
    };
    let long = "x".repeat(MAX_AGENT_OPTION_TEXT + 1);
    let mut too_many = WorkspaceSettings::default();
    let options = too_many.agent_options.entry("codex".into()).or_default();
    for index in 0..=MAX_AGENT_OPTIONS {
        options.insert(format!("option{index}"), "value".into());
    }
    for settings in [
        with("unknown", "model".into(), "x".into()),
        with("claude", String::new(), "x".into()),
        with("claude", "model".into(), String::new()),
        with("claude", "model".into(), long),
        too_many,
    ] {
        assert!(matches!(
            settings.validate(),
            Err(SettingsError::Invalid("agent options"))
        ));
    }
}
