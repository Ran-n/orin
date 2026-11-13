// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:12:36.732235
//+ Revised: 	2026/11/12 22:43:40.090821
// -----------------------------------------------------------------------------

//! Versioned workspace preferences, separate from conversations and credentials.

mod model;
mod store;

pub use model::{
    Appearance, Column, ColumnState, Columns, MAX_AGENT_OPTION_TEXT, MAX_AGENT_OPTIONS,
    MAX_COLUMN_WIDTH, MAX_RECENT_PROJECTS, Page, SCHEMA_VERSION, SettingsSection, Theme, Updates,
    ViewerMode, WorkspaceSettings,
};
pub use store::SettingsStore;

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("Workspace settings IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Workspace settings are not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Workspace settings schema {0} is unsupported; the original file was preserved")]
    UnsupportedSchema(u32),
    #[error("Workspace settings contain an invalid value: {0}")]
    Invalid(&'static str),
    #[error("No user configuration directory is available")]
    NoConfigDirectory,
}
