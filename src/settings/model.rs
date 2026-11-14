// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:12:36.872262
//+ Revised: 	2026/11/13 17:30:08.523896
// -----------------------------------------------------------------------------

//! Stable schema for workspace choices that survive restarts.

use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::SettingsError;

pub const SCHEMA_VERSION: u32 = 2;

/// What the Code column shows for the selected file.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewerMode {
    /// The file itself, editable.
    #[default]
    File,
    /// Working-tree changes not yet staged.
    Unstaged,
    /// Changes staged for the next commit.
    Staged,
}

impl ViewerMode {
    pub const ALL: [Self; 3] = [Self::File, Self::Unstaged, Self::Staged];

    pub fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Unstaged => "Unstaged",
            Self::Staged => "Staged",
        }
    }

    /// Stable identifier used by persisted files, the UI driver, and control ids.
    pub fn key(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Unstaged => "unstaged",
            Self::Staged => "staged",
        }
    }
}

/// The four side-by-side workspace columns.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Column {
    Files,
    Code,
    Agents,
    Terminal,
}

impl Column {
    /// Every column, in the default left-to-right order: Files, Code (the editor), Agents, Terminal.
    pub const ALL: [Self; 4] = [Self::Files, Self::Code, Self::Agents, Self::Terminal];

    pub fn title(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Agents => "Agents",
            Self::Code => "Code",
            Self::Terminal => "Terminal",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Agents => "agents",
            Self::Code => "code",
            Self::Terminal => "terminal",
        }
    }

    /// Narrowest usable width in logical pixels.
    pub fn min_width(self) -> f32 {
        match self {
            Self::Files => 140.0,
            Self::Agents => 300.0,
            Self::Code => 220.0,
            Self::Terminal => 200.0,
        }
    }

    fn default_width(self) -> f32 {
        match self {
            Self::Files => 200.0,
            Self::Agents => 480.0,
            Self::Code => 360.0,
            Self::Terminal => 280.0,
        }
    }
}

pub const MAX_COLUMN_WIDTH: f32 = 4_000.0;

/// One column's preferred width and whether it is folded into a narrow rail.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct ColumnState {
    pub width: f32,
    pub collapsed: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Columns {
    /// Left-to-right display order; always each column exactly once.
    pub order: [Column; 4],
    pub files: ColumnState,
    pub agents: ColumnState,
    pub code: ColumnState,
    pub terminal: ColumnState,
}

impl Default for Columns {
    fn default() -> Self {
        let state = |column: Column| ColumnState {
            width: column.default_width(),
            collapsed: false,
        };
        Self {
            order: Column::ALL,
            files: state(Column::Files),
            agents: state(Column::Agents),
            code: state(Column::Code),
            terminal: state(Column::Terminal),
        }
    }
}

impl Columns {
    pub fn get(&self, column: Column) -> ColumnState {
        match column {
            Column::Files => self.files,
            Column::Agents => self.agents,
            Column::Code => self.code,
            Column::Terminal => self.terminal,
        }
    }

    pub fn get_mut(&mut self, column: Column) -> &mut ColumnState {
        match column {
            Column::Files => &mut self.files,
            Column::Agents => &mut self.agents,
            Column::Code => &mut self.code,
            Column::Terminal => &mut self.terminal,
        }
    }

    /// Move `column` so that it ends up at `index` in the display order (clamped to the end).
    pub fn move_column(&mut self, column: Column, index: usize) {
        let mut order: Vec<Column> = self
            .order
            .into_iter()
            .filter(|other| *other != column)
            .collect();
        order.insert(index.min(order.len()), column);
        self.order = order
            .try_into()
            .expect("moving a column keeps all four columns");
    }

    /// True when the order lists every column exactly once.
    pub fn order_is_complete(&self) -> bool {
        Column::ALL
            .iter()
            .all(|column| self.order.iter().filter(|other| *other == column).count() == 1)
    }
}

/// The interface colour themes. Gruvbox Dark (medium contrast) is the default.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    GruvboxDark,
    GruvboxLight,
    /// Orin's own dark theme, from the mascot's walnut and oak-leaf colours.
    Dark,
    Light,
}

impl Theme {
    pub const ALL: [Self; 4] = [
        Self::GruvboxDark,
        Self::GruvboxLight,
        Self::Dark,
        Self::Light,
    ];

    /// Stable identifier used by persisted files, the UI driver, and control ids.
    pub fn key(self) -> &'static str {
        match self {
            Self::GruvboxDark => "gruvbox_dark",
            Self::GruvboxLight => "gruvbox_light",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::GruvboxDark => "Gruvbox Dark",
            Self::GruvboxLight => "Gruvbox Light",
            Self::Dark => "Orin Dark",
            Self::Light => "Orin Light",
        }
    }

    pub fn is_dark(self) -> bool {
        matches!(self, Self::GruvboxDark | Self::Dark)
    }
}

/// A full-window page shown in place of the columns.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Page {
    /// Recent projects and ways to start, like VS Code's Welcome page.
    Welcome,
    Settings(SettingsSection),
}

/// Most recent projects remembered for the Welcome page.
pub const MAX_RECENT_PROJECTS: usize = 10;
/// Limits on remembered agent choices, so a damaged file can't grow without bound.
pub const MAX_AGENT_OPTIONS: usize = 16;
pub const MAX_AGENT_OPTION_TEXT: usize = 200;

/// The sections of the Settings page.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingsSection {
    #[default]
    Appearance,
    Accounts,
    Layout,
    Updates,
}

impl SettingsSection {
    pub const ALL: [Self; 4] = [
        Self::Appearance,
        Self::Accounts,
        Self::Layout,
        Self::Updates,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Appearance => "appearance",
            Self::Accounts => "accounts",
            Self::Layout => "layout",
            Self::Updates => "updates",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Accounts => "Accounts",
            Self::Layout => "Layout",
            Self::Updates => "Updates",
        }
    }
}

/// Personal look of the interface: colours, fonts, and the base text size everything scales with.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Appearance {
    pub theme: Theme,
    /// Interface text size in logical pixels; code, rows, and controls scale in proportion.
    pub text_size: f32,
    /// Font family for the interface; `None` uses the system's interface font.
    pub interface_font: Option<String>,
    /// Font family for code, the terminal, and code blocks; `None` uses the system's monospace font.
    pub code_font: Option<String>,
}

impl Appearance {
    pub const DEFAULT_TEXT_SIZE: f32 = 13.0;
    pub const MIN_TEXT_SIZE: f32 = 10.0;
    pub const MAX_TEXT_SIZE: f32 = 20.0;
    /// Longest font family name accepted from the settings file.
    pub const MAX_FONT_NAME: usize = 256;

    /// How much larger than the default design everything is drawn.
    pub fn scale(&self) -> f32 {
        self.text_size / Self::DEFAULT_TEXT_SIZE
    }

    /// The text size one step larger or smaller, kept within the supported range.
    pub fn step_text_size(&mut self, steps: f32) {
        self.text_size =
            (self.text_size.round() + steps).clamp(Self::MIN_TEXT_SIZE, Self::MAX_TEXT_SIZE);
    }
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            text_size: Self::DEFAULT_TEXT_SIZE,
            interface_font: None,
            code_font: None,
        }
    }
}

/// Longest version text accepted from the settings file.
pub const MAX_VERSION_TEXT: usize = 64;

/// The update check's choices and memory (Settings › Updates).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Updates {
    /// Check once a day without being asked; on by default.
    pub check_automatically: bool,
    /// When the last check succeeded, in seconds since the Unix epoch.
    pub last_check: Option<u64>,
    /// A release the user chose to skip; newer ones are offered again.
    pub skipped_version: Option<String>,
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            check_automatically: true,
            last_check: None,
            skipped_version: None,
        }
    }
}

impl Updates {
    /// The skipped version, if it is one.
    pub fn skipped(&self) -> Option<semver::Version> {
        semver::Version::parse(self.skipped_version.as_deref()?).ok()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct WorkspaceSettings {
    pub schema_version: u32,
    pub project: Option<PathBuf>,
    pub selected_file: Option<PathBuf>,
    pub viewer_mode: ViewerMode,
    /// Show Markdown files rendered instead of as editable source.
    pub markdown_preview: bool,
    pub editor_cursor: usize,
    pub selected_provider: String,
    pub active_session: usize,
    pub columns: Columns,
    pub appearance: Appearance,
    /// The page shown in place of the columns, or `None` when the columns are shown.
    pub page: Option<Page>,
    /// Absolute paths of recently opened projects, most recent first.
    pub recent_projects: Vec<PathBuf>,
    /// Each agent's last chosen session options (model, effort, mode), by provider id and option id.
    pub agent_options: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_directories: Vec<PathBuf>,
    pub width: f32,
    pub height: f32,
    pub position: Option<(f32, f32)>,
    pub expanded_folders: Vec<PathBuf>,
    /// Added in schema 2 with defaults, so older files load unchanged.
    pub updates: Updates,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            project: None,
            selected_file: None,
            viewer_mode: ViewerMode::File,
            markdown_preview: false,
            editor_cursor: 0,
            selected_provider: "codex".into(),
            active_session: 0,
            columns: Columns::default(),
            appearance: Appearance::default(),
            page: None,
            recent_projects: Vec::new(),
            agent_options: BTreeMap::new(),
            tool_directories: Vec::new(),
            width: 1440.0,
            height: 860.0,
            position: None,
            expanded_folders: Vec::new(),
            updates: Updates::default(),
        }
    }
}

impl WorkspaceSettings {
    /// Read any supported schema version, upgrading older files in memory.
    /// The file on disk is only rewritten by a later save, so a failed upgrade never loses data.
    pub fn from_json(value: Value) -> Result<Self, SettingsError> {
        let version = value
            .get("schema_version")
            .and_then(Value::as_u64)
            .map_or(Ok(SCHEMA_VERSION), u32::try_from)
            .map_err(|_| SettingsError::Invalid("schema version"))?;
        let value = match version {
            1 => migrate_v1(value)?,
            SCHEMA_VERSION => value,
            other => return Err(SettingsError::UnsupportedSchema(other)),
        };
        let settings: Self = serde_json::from_value(value)?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(SettingsError::UnsupportedSchema(self.schema_version));
        }
        self.validate_window()?;
        self.validate_columns()?;
        self.validate_appearance()?;
        self.validate_paths()?;
        self.validate_recent()?;
        self.validate_agent_options()?;
        self.validate_updates()?;
        if self.tool_directories.len() > 32
            || self.tool_directories.iter().any(|path| !path.is_absolute())
        {
            return Err(SettingsError::Invalid("agent tool folders"));
        }
        if self.editor_cursor > crate::project::MAX_FILE_BYTES {
            return Err(SettingsError::Invalid("editor cursor"));
        }
        if !["claude", "codex", "gemini"].contains(&self.selected_provider.as_str()) {
            return Err(SettingsError::Invalid("selected_provider"));
        }
        Ok(())
    }

    fn validate_window(&self) -> Result<(), SettingsError> {
        if !self.width.is_finite()
            || !self.height.is_finite()
            || self.width <= 0.0
            || self.height <= 0.0
        {
            return Err(SettingsError::Invalid("window size"));
        }
        if self
            .position
            .is_some_and(|(x, y)| !x.is_finite() || !y.is_finite())
        {
            return Err(SettingsError::Invalid("window position"));
        }
        Ok(())
    }

    fn validate_columns(&self) -> Result<(), SettingsError> {
        let valid = Column::ALL.into_iter().all(|column| {
            let width = self.columns.get(column).width;
            width.is_finite() && (column.min_width()..=MAX_COLUMN_WIDTH).contains(&width)
        });
        if !valid {
            return Err(SettingsError::Invalid("column width"));
        }
        if !self.columns.order_is_complete() {
            return Err(SettingsError::Invalid("column order"));
        }
        Ok(())
    }

    fn validate_appearance(&self) -> Result<(), SettingsError> {
        let size = self.appearance.text_size;
        if !size.is_finite()
            || !(Appearance::MIN_TEXT_SIZE..=Appearance::MAX_TEXT_SIZE).contains(&size)
        {
            return Err(SettingsError::Invalid("text size"));
        }
        let fonts = [&self.appearance.interface_font, &self.appearance.code_font];
        let valid = fonts.into_iter().flatten().all(|name| {
            !name.trim().is_empty()
                && name.len() <= Appearance::MAX_FONT_NAME
                && !name.chars().any(char::is_control)
        });
        if valid {
            Ok(())
        } else {
            Err(SettingsError::Invalid("font name"))
        }
    }

    fn validate_recent(&self) -> Result<(), SettingsError> {
        let valid = self.recent_projects.len() <= MAX_RECENT_PROJECTS
            && self
                .recent_projects
                .iter()
                .all(|path| path.is_absolute() && path.as_os_str().len() <= 4_096);
        if valid {
            Ok(())
        } else {
            Err(SettingsError::Invalid("recent projects"))
        }
    }

    fn validate_agent_options(&self) -> Result<(), SettingsError> {
        let text = |value: &String| !value.is_empty() && value.len() <= MAX_AGENT_OPTION_TEXT;
        let valid = self.agent_options.iter().all(|(provider, options)| {
            ["claude", "codex", "gemini"].contains(&provider.as_str())
                && options.len() <= MAX_AGENT_OPTIONS
                && options.iter().all(|(id, value)| text(id) && text(value))
        });
        if valid {
            Ok(())
        } else {
            Err(SettingsError::Invalid("agent options"))
        }
    }

    fn validate_updates(&self) -> Result<(), SettingsError> {
        let valid = self
            .updates
            .skipped_version
            .as_deref()
            .is_none_or(|version| {
                version.len() <= MAX_VERSION_TEXT && semver::Version::parse(version).is_ok()
            });
        if valid {
            Ok(())
        } else {
            Err(SettingsError::Invalid("skipped update version"))
        }
    }

    /// Put `path` first in the recent projects, without duplicates, keeping the newest few.
    pub fn remember_project(&mut self, path: &Path) {
        if !path.is_absolute() {
            return;
        }
        self.recent_projects.retain(|recent| recent != path);
        self.recent_projects.insert(0, path.to_owned());
        self.recent_projects.truncate(MAX_RECENT_PROJECTS);
    }

    /// The open Settings section, if the Settings page is shown.
    pub fn settings_section(&self) -> Option<SettingsSection> {
        match self.page {
            Some(Page::Settings(section)) => Some(section),
            _ => None,
        }
    }

    fn validate_paths(&self) -> Result<(), SettingsError> {
        let paths = self
            .selected_file
            .iter()
            .chain(self.expanded_folders.iter());
        for path in paths {
            if !is_relative(path) {
                return Err(SettingsError::Invalid("project-relative path"));
            }
        }
        // An empty or `.` file path names the project folder itself, which can't be opened.
        if self.selected_file.as_ref().is_some_and(|file| {
            !file
                .components()
                .any(|part| matches!(part, Component::Normal(_)))
        }) {
            return Err(SettingsError::Invalid("selected file path"));
        }
        Ok(())
    }
}

/// Version 1 had one pane switch (agents or code), a terminal below it, and a numeric viewer mode.
/// Version 2 shows every area side by side, so the switch is dropped and a hidden terminal becomes
/// a collapsed Terminal column.
fn migrate_v1(mut value: Value) -> Result<Value, SettingsError> {
    let object = value
        .as_object_mut()
        .ok_or(SettingsError::Invalid("settings must be an object"))?;
    let viewer_mode = match object.remove("viewer_mode") {
        None => ViewerMode::File,
        Some(Value::Number(number)) => match number.as_u64() {
            Some(index @ 0..=2) => ViewerMode::ALL[index as usize],
            _ => return Err(SettingsError::Invalid("viewer mode")),
        },
        Some(_) => return Err(SettingsError::Invalid("viewer mode")),
    };
    let terminal_visible = match object.remove("terminal_visible") {
        None => true,
        Some(Value::Bool(visible)) => visible,
        Some(_) => return Err(SettingsError::Invalid("terminal visibility")),
    };
    object.remove("active_pane");
    let mut columns = Columns::default();
    columns.terminal.collapsed = !terminal_visible;
    object.insert("viewer_mode".into(), serde_json::to_value(viewer_mode)?);
    object.insert("columns".into(), serde_json::to_value(columns)?);
    object.insert("schema_version".into(), SCHEMA_VERSION.into());
    tracing::info!(target: "orin::settings", "workspace settings upgraded from schema 1");
    Ok(value)
}

fn is_relative(path: &Path) -> bool {
    path.components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}
