// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:37.274084
//+ Revised: 	2026/11/12 23:40:35.983388
// -----------------------------------------------------------------------------

//! Self-updating: which build this is, finding newer releases, and installing them.
//!
//! GPUI-free, so every rule here is testable without a window.

/// The local release server the update tests share.
#[cfg(test)]
#[path = "../../tests/update/fixture.rs"]
mod fixture;

pub mod check;
pub mod download;
pub mod install;
pub mod release;
pub mod source;
pub mod target;
