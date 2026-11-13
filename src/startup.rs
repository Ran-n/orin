// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:17:17.437264
//+ Revised: 	2026/11/12 09:16:39.867078
// -----------------------------------------------------------------------------

//! Command-line options shared by the desktop app and its test driver.

use std::{ffi::OsString, path::PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub project: Option<PathBuf>,
    pub settings: Option<PathBuf>,
    pub test_driver: bool,
    pub help: bool,
    pub version: bool,
}

impl Options {
    pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, &'static str> {
        let mut options = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.to_str() {
                Some("--project") => options.project = Some(next_path(&mut arguments)?),
                Some("--settings") => options.settings = Some(next_path(&mut arguments)?),
                Some("--test-driver") => options.test_driver = true,
                Some("--help" | "-h") => options.help = true,
                Some("--version" | "-V") => options.version = true,
                _ => return Err("Unknown option. Run orin --help to see available options."),
            }
        }
        if options.test_driver && options.settings.is_none() {
            return Err("The test driver needs an isolated --settings path.");
        }
        Ok(options)
    }
}

fn next_path(arguments: &mut impl Iterator<Item = OsString>) -> Result<PathBuf, &'static str> {
    arguments
        .next()
        .filter(|value| !value.is_empty() && !value.to_string_lossy().starts_with("--"))
        .map(PathBuf::from)
        .ok_or("This option needs a file or folder path.")
}

pub const HELP: &str = "Orin — From intention to creation.\n\nUsage: orin [--project PATH] [--settings FILE]\n\n  --project PATH   Open a project folder\n  --settings FILE  Use a workspace settings file\n  --help           Show this help\n  --version        Show the application version\n";
