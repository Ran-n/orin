// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:38.676657
//+ Revised: 	2026/11/12 22:43:38.676657
// -----------------------------------------------------------------------------

//! Installing over a binary that is running, for real: on Windows a running `.exe` can't be
//! overwritten, so this proves the rename-aside path `self_replace` takes.

use std::{fs, path::Path, process::Command};

const CHILD: &str = "child_installs_the_new_binary";
const NEW_BINARY: &str = "ORIN_TEST_INSTALL_NEW_BINARY";

#[test]
fn integration_install_replaces_the_running_binary() {
    let dir = tempfile::tempdir().unwrap();
    // The "old" Orin is a copy of this test binary, so it can run `install` on itself.
    let old = dir
        .path()
        .join(format!("old-orin{}", std::env::consts::EXE_SUFFIX));
    fs::copy(std::env::current_exe().unwrap(), &old).unwrap();
    // The "new" one is another small binary from this crate.
    let new = Path::new(env!("CARGO_BIN_EXE_orin-asset-name"));
    let output = Command::new(&old)
        .args([
            "--exact",
            CHILD,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(NEW_BINARY, new)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(&old).unwrap(),
        fs::read(new).unwrap(),
        "the file on disk is now the new binary"
    );
    let replaced = Command::new(&old).arg("linux-x86_64").output().unwrap();
    assert!(replaced.status.success());
    assert!(
        String::from_utf8_lossy(&replaced.stdout).starts_with("orin-"),
        "the replaced file runs as the new binary"
    );
}

#[test]
#[ignore = "run as a child process by integration_install_replaces_the_running_binary"]
fn child_installs_the_new_binary() {
    let Some(new) = std::env::var_os(NEW_BINARY) else {
        return;
    };
    let current = std::env::current_exe().unwrap();
    let mut download = tempfile::Builder::new()
        .prefix("orin-update-")
        .tempfile_in(current.parent().unwrap())
        .unwrap();
    std::io::copy(&mut fs::File::open(new).unwrap(), &mut download).unwrap();
    let installed = orin::update::install::install(download).unwrap();
    assert_eq!(installed, current);
}
