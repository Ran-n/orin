// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/14 12:09:01.976775
//+ Revised: 	2026/11/14 21:46:04.550044
// -----------------------------------------------------------------------------

//! Sign release assets, so Orin's updater can tell the maintainer's files from anyone else's.
//!
//! - `orin-sign keygen <secret-file>` makes a new secret key and prints its public key.
//! - `orin-sign public <secret-file>` prints the public key of an existing secret key.
//! - `orin-sign sign <secret-file> <asset>...` writes `<asset>.sig` next to each asset.
//! - `orin-sign verify <asset>...` checks each asset's `.sig` against the keys Orin is built with.
//!
//! `<secret-file>` may be `-` for `public` and `sign`: the key is then read from standard
//! input, so a password manager can pipe it in and it never rests in a file.
//! An asset is signed as this checkout's version, so run it from the tag being released.

use orin::update::{
    signature::{RELEASE_KEYS, generate, public_key, sign, statement, verify_any},
    target::{current_version, signature_name},
};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::ExitCode,
};

const USAGE: &str = "Usage:
  orin-sign keygen <secret-file>
  orin-sign public <secret-file>
  orin-sign sign <secret-file> <asset>...
  orin-sign verify <asset>...
<secret-file> may be - to read the key from standard input.";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let result = match arguments.as_slice() {
        ["keygen", secret] => keygen(Path::new(secret)),
        ["public", secret] => read_secret(Path::new(secret)).and_then(|secret| public(&secret)),
        ["sign", secret, assets @ ..] if !assets.is_empty() => read_secret(Path::new(secret))
            .and_then(|secret| {
                assets
                    .iter()
                    .try_for_each(|asset| sign_asset(&secret, Path::new(asset)))
            }),
        ["verify", assets @ ..] if !assets.is_empty() => assets
            .iter()
            .try_for_each(|asset| verify_asset(Path::new(asset))),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("{reason}");
            ExitCode::FAILURE
        }
    }
}

/// Write a new secret key to `path`, never over an existing file: losing the key that shipped
/// builds trust would leave them unable to update.
fn keygen(path: &Path) -> Result<(), String> {
    let secret = generate().map_err(|error| error.to_string())?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("{} couldn't be created: {error}", path.display()))?;
    writeln!(file, "{secret}")
        .map_err(|error| format!("{} couldn't be written: {error}", path.display()))?;
    public(&secret)
}

fn public(secret: &str) -> Result<(), String> {
    println!("{}", public_key(secret).map_err(|error| error.to_string())?);
    Ok(())
}

/// The secret key in `path`, or from standard input when `path` is `-`.
fn read_secret(path: &Path) -> Result<String, String> {
    let secret = if path == Path::new("-") {
        let mut piped = String::new();
        std::io::stdin()
            .read_to_string(&mut piped)
            .map(|_| piped)
            .map_err(|error| format!("the key couldn't be read from standard input: {error}"))
    } else {
        fs::read_to_string(path)
            .map_err(|error| format!("{} couldn't be read: {error}", path.display()))
    }?;
    Ok(secret.trim().to_owned())
}

/// The statement a signature of `asset` covers: this version, its file name, its BLAKE3 hash.
fn asset_statement(asset: &Path) -> Result<String, String> {
    let version = current_version();
    let name = asset
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no usable file name", asset.display()))?;
    if !name.starts_with(&format!("orin-{version}-")) {
        return Err(format!(
            "{name} isn't an asset of Orin {version}, the version of this checkout"
        ));
    }
    let mut hasher = blake3::Hasher::new();
    let mut file = fs::File::open(asset)
        .map_err(|error| format!("{} couldn't be read: {error}", asset.display()))?;
    std::io::copy(&mut file, &mut hasher)
        .map_err(|error| format!("{} couldn't be read: {error}", asset.display()))?;
    Ok(statement(
        &version,
        name,
        hasher.finalize().to_hex().as_str(),
    ))
}

fn signature_path(asset: &Path) -> std::path::PathBuf {
    asset.with_file_name(signature_name(
        &asset.file_name().unwrap_or_default().to_string_lossy(),
    ))
}

fn sign_asset(secret: &str, asset: &Path) -> Result<(), String> {
    let signature = sign(secret, &asset_statement(asset)?).map_err(|error| error.to_string())?;
    let path = signature_path(asset);
    fs::write(&path, signature)
        .map_err(|error| format!("{} couldn't be written: {error}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}

fn verify_asset(asset: &Path) -> Result<(), String> {
    let path = signature_path(asset);
    let signature = fs::read_to_string(&path)
        .map_err(|error| format!("{} couldn't be read: {error}", path.display()))?;
    verify_any(&RELEASE_KEYS, &asset_statement(asset)?, &signature)
        .map_err(|error| format!("{}: {error}", asset.display()))?;
    println!(
        "{}: signed with one of Orin's release keys",
        asset.display()
    );
    Ok(())
}
