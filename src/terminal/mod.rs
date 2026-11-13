// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:51.655043
//+ Revised: 	2026/11/12 09:16:40.227508
// -----------------------------------------------------------------------------

//! Native PTY sessions for the developer's own shell commands.

mod process_tree;
mod queries;
mod screen;
mod session;
mod shell;
mod worker;

pub use screen::{ScreenRow, ScreenSpan, TerminalScreen, TerminalSnapshot};
pub use session::Session;
