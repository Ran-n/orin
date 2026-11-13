// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:10:30.188312
//+ Revised: 	2026/11/12 22:43:40.162290
// -----------------------------------------------------------------------------

//! Orin's workspace services and native desktop interface.

pub mod agents;
pub mod background;
pub mod editor;
pub mod keyboard;
pub mod logging;
mod process;
pub mod project;
pub mod settings;
mod sharing;
pub mod startup;
pub mod terminal;
pub mod update;

#[cfg(feature = "desktop")]
pub mod desktop;
