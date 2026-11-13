// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:38.398387
//+ Revised: 	2026/11/12 22:43:38.398387
// -----------------------------------------------------------------------------

//! The release workflow and the updater share one naming rule: `orin-asset-name`, built the way
//! the workflow builds it, prints what `asset_name` computes.

use orin::update::target::{asset_name, current_version};
use std::{path::PathBuf, process::Command};

#[test]
fn integration_release_build_prints_its_own_asset_name() {
    // A folder of its own, so the normal build isn't rebuilt without the variable; it is kept
    // between runs so only the first one compiles from scratch.
    let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release-target-test");
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "--quiet",
            "--no-default-features",
            "--bin",
            "orin-asset-name",
        ])
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("ORIN_RELEASE_TARGET", "linux-x86_64")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("run cargo");
    assert!(status.success());
    let binary = target_dir
        .join("debug")
        .join(format!("orin-asset-name{}", std::env::consts::EXE_SUFFIX));
    let run = |arguments: &[&str]| {
        let output = Command::new(&binary)
            .args(arguments)
            .env_remove("ORIN_RELEASE_TARGET")
            .output()
            .expect("run orin-asset-name");
        (
            output.status.success(),
            String::from_utf8(output.stdout).unwrap().trim().to_owned(),
        )
    };
    let version = current_version();
    assert_eq!(
        run(&[]),
        (true, asset_name(&version, "linux-x86_64")),
        "the compiled-in target"
    );
    assert_eq!(
        run(&["windows-x86_64"]),
        (true, asset_name(&version, "windows-x86_64")),
        "the workflow's explicit target"
    );
    assert!(!run(&["plan9-mips"]).0, "unknown targets are refused");
}
