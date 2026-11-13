// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 15:12:10.485713
//+ Revised: 	2026/11/12 15:12:44.273721
// -----------------------------------------------------------------------------

#![cfg(feature = "desktop")]

use std::process::Command;

fn run(argument: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_orin"))
        .arg(argument)
        .env_remove("ORIN_LOG")
        .output()
        .unwrap()
}

#[test]
fn contract_real_entrypoint_reports_help_and_manifest_version_without_opening_a_window() {
    let output = run("--version");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("Orin {}\n", env!("CARGO_PKG_VERSION"))
    );
    let help = run("--help");
    assert!(help.status.success());
    assert_eq!(String::from_utf8(help.stdout).unwrap(), orin::startup::HELP);
}

#[test]
fn integration_invalid_startup_has_a_failure_status_and_structured_diagnostics() {
    let output = run("--unknown");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    let logged: serde_json::Value = serde_json::from_str(error.lines().next().unwrap()).unwrap();
    assert_eq!(logged["level"], "ERROR");
    assert_eq!(logged["target"], "orin::startup");
    assert!(error.contains("Run orin --help"));
}
