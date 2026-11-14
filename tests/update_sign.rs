// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/14 12:12:23.354398
//+ Revised: 	2026/11/14 21:46:04.632727
// -----------------------------------------------------------------------------

//! `orin-sign` as the release steps run it: a real process, real files, and the same statement
//! the updater verifies.

use orin::update::{
    signature::{self, RELEASE_KEYS},
    target::current_version,
};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

fn sign_tool(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_orin-sign"))
        .args(arguments)
        .output()
        .expect("run orin-sign")
}

/// `orin-sign` with `input` piped in, as a password manager hands over the key.
fn sign_tool_with_input(arguments: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_orin-sign"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run orin-sign");
    child
        .stdin
        .take()
        .expect("piped input")
        .write_all(input.as_bytes())
        .expect("write the key");
    child.wait_with_output().expect("orin-sign finishes")
}

fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 temporary path")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_owned()
}

#[test]
fn integration_a_generated_key_signs_an_asset_the_updater_accepts() {
    let directory = tempfile::tempdir().unwrap();
    let secret = directory.path().join("release.key");
    let generated = sign_tool(&["keygen", text(&secret)]);
    assert!(generated.status.success(), "{}", stderr(&generated));
    let public = stdout(&generated);
    assert_eq!(public.len(), 64, "a public key is 32 bytes of hex");
    assert_eq!(stdout(&sign_tool(&["public", text(&secret)])), public);

    let version = current_version();
    let name = format!("orin-{version}-linux-x86_64");
    let asset = directory.path().join(&name);
    fs::write(&asset, b"a release binary").unwrap();
    let signed = sign_tool(&["sign", text(&secret), text(&asset)]);
    assert!(signed.status.success(), "{}", stderr(&signed));

    // The updater's own check: this version, this name, this file's hash, the new key.
    let signature_file = directory.path().join(format!("{name}.sig"));
    let signature_text = fs::read_to_string(&signature_file).unwrap();
    let hash = blake3::hash(b"a release binary").to_hex();
    let statement = signature::statement(&version, &name, hash.as_str());
    assert_eq!(
        signature::verify(&public, &statement, &signature_text),
        Ok(())
    );
    // Changed after signing, the file no longer matches its signature.
    let changed = signature::statement(&version, &name, blake3::hash(b"other").to_hex().as_str());
    assert!(signature::verify(&public, &changed, &signature_text).is_err());

    // `verify` trusts only the key Orin is built with, never one made a moment ago.
    assert!(!RELEASE_KEYS.contains(&public.as_str()));
    let verified = sign_tool(&["verify", text(&asset)]);
    assert!(!verified.status.success());
    assert!(
        stderr(&verified).contains("isn't signed with Orin's release key"),
        "{}",
        stderr(&verified)
    );
}

#[test]
fn integration_a_key_piped_in_signs_without_resting_in_a_file() {
    let directory = tempfile::tempdir().unwrap();
    // The key as a password manager prints it: the hex, then a line break.
    let secret = format!("{}\n", "5a".repeat(32));
    let public = signature::public_key(secret.trim()).unwrap();
    let shown = sign_tool_with_input(&["public", "-"], &secret);
    assert!(shown.status.success(), "{}", stderr(&shown));
    assert_eq!(stdout(&shown), public);

    let version = current_version();
    let name = format!("orin-{version}-windows-x86_64.exe");
    let asset = directory.path().join(&name);
    fs::write(&asset, b"a release binary").unwrap();
    let signed = sign_tool_with_input(&["sign", "-", text(&asset)], &secret);
    assert!(signed.status.success(), "{}", stderr(&signed));
    let signature_text = fs::read_to_string(directory.path().join(format!("{name}.sig"))).unwrap();
    let hash = blake3::hash(b"a release binary").to_hex();
    assert_eq!(
        signature::verify(
            &public,
            &signature::statement(&version, &name, hash.as_str()),
            &signature_text
        ),
        Ok(())
    );
    // Only the asset and its signature exist: the key was never written anywhere.
    let mut files: Vec<String> = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, [name.clone(), format!("{name}.sig")]);

    // Nothing piped in, or something that isn't a key, is refused by name.
    let empty = sign_tool_with_input(&["sign", "-", text(&asset)], "");
    assert!(!empty.status.success());
    assert!(
        stderr(&empty).contains("the secret key"),
        "{}",
        stderr(&empty)
    );
}

#[test]
fn integration_keygen_never_overwrites_a_key() {
    let directory = tempfile::tempdir().unwrap();
    let secret = directory.path().join("release.key");
    assert!(sign_tool(&["keygen", text(&secret)]).status.success());
    let first = fs::read_to_string(&secret).unwrap();
    let again = sign_tool(&["keygen", text(&secret)]);
    assert!(!again.status.success(), "a second keygen must refuse");
    assert!(stderr(&again).contains("couldn't be created"));
    assert_eq!(fs::read_to_string(&secret).unwrap(), first);
}

#[test]
fn integration_wrong_inputs_are_refused_with_a_reason() {
    let directory = tempfile::tempdir().unwrap();
    let secret = directory.path().join("release.key");
    assert!(sign_tool(&["keygen", text(&secret)]).status.success());

    // An asset of another version: signing it from this checkout would vouch for the wrong build.
    let other = directory.path().join("orin-999.0.0-linux-x86_64");
    fs::write(&other, b"binary").unwrap();
    let refused = sign_tool(&["sign", text(&secret), text(&other)]);
    assert!(!refused.status.success());
    assert!(
        stderr(&refused).contains("isn't an asset of Orin"),
        "{}",
        stderr(&refused)
    );
    assert!(
        !directory
            .path()
            .join("orin-999.0.0-linux-x86_64.sig")
            .exists()
    );

    let missing_key = sign_tool(&["sign", text(&directory.path().join("no.key")), text(&other)]);
    assert!(!missing_key.status.success());
    assert!(stderr(&missing_key).contains("couldn't be read"));

    let name = format!("orin-{}-linux-x86_64", current_version());
    let unsigned = directory.path().join(name);
    fs::write(&unsigned, b"binary").unwrap();
    let no_signature = sign_tool(&["verify", text(&unsigned)]);
    assert!(!no_signature.status.success());
    assert!(stderr(&no_signature).contains("couldn't be read"));

    for arguments in [
        &[][..],
        &["sign"][..],
        &["sign", "key"][..],
        &["frobnicate"][..],
    ] {
        let usage = sign_tool(arguments);
        assert!(!usage.status.success(), "{arguments:?}");
        assert!(stderr(&usage).starts_with("Usage:"), "{arguments:?}");
    }
}
