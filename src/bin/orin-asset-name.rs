// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:38.264226
//+ Revised: 	2026/11/12 22:43:38.264226
// -----------------------------------------------------------------------------

//! Print a release asset's file name, so the release workflow never spells out the naming rule.
//!
//! `orin-asset-name <target>` prints this version's asset for `<target>`; with no argument it
//! prints the asset of the target this binary was built for (`ORIN_RELEASE_TARGET`).

use orin::update::target::{TARGETS, asset_name, current_version, release_target};
use std::process::ExitCode;

fn main() -> ExitCode {
    let target = std::env::args().nth(1);
    let Some(target) = target.as_deref().or(release_target()) else {
        eprintln!(
            "Usage: orin-asset-name <target>   (one of {})",
            TARGETS.join(", ")
        );
        return ExitCode::FAILURE;
    };
    if !TARGETS.contains(&target) {
        eprintln!(
            "Unknown target {target}; expected one of {}",
            TARGETS.join(", ")
        );
        return ExitCode::FAILURE;
    }
    println!("{}", asset_name(&current_version(), target));
    ExitCode::SUCCESS
}
