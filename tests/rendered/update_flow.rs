// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:38.949009
//+ Revised: 	2026/11/13 13:04:33.686408
// -----------------------------------------------------------------------------

//! The updater in the real window: the check, the banner, Settings › Updates, and the
//! download-verify-restart path, against a local release server (never the network).

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

#[path = "../update/fixture.rs"]
mod release_server;

use super::{driver::AppDriver, fixture::LaunchOptions, pixels, snapshots};
use release_server::{FixtureServer, Reply};

const TARGET: &str = "windows-x86_64";

/// The next minor version after the running one, as the fixture release offers it.
pub fn next_version() -> String {
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    semver::Version::new(current.major, current.minor + 1, 0).to_string()
}

fn asset() -> String {
    format!("orin-{}-{TARGET}.exe", next_version())
}

/// A GitHub-shaped `releases/latest` answer whose downloads point back at `server`.
fn github_release(server: &FixtureServer) -> String {
    let asset = asset();
    json!({
        "tag_name": format!("v{}", next_version()),
        "html_url": server.url("/release"),
        "assets": [
            {"name": asset, "browser_download_url": server.url("/asset")},
            {"name": format!("{asset}.b3"), "browser_download_url": server.url("/asset.b3")},
        ],
    })
    .to_string()
}

fn gitlab_release(server: &FixtureServer) -> String {
    let asset = asset();
    json!({
        "tag_name": format!("v{}", next_version()),
        "_links": {"self": server.url("/release")},
        "assets": {"links": [
            {"name": asset, "direct_asset_url": server.url("/asset")},
            {"name": format!("{asset}.b3"), "direct_asset_url": server.url("/asset.b3")},
        ]},
    })
    .to_string()
}

fn b3_line(bytes: &[u8]) -> String {
    format!("{}  {}\n", blake3::hash(bytes).to_hex(), asset())
}

/// A server offering the next version, whose binary is `binary`.
fn release_server(binary: Vec<u8>) -> FixtureServer {
    let server = FixtureServer::start(vec![]);
    server.route("/github", Reply::ok(github_release(&server)));
    server.route("/gitlab", Reply::ok(gitlab_release(&server)));
    server.route("/asset.b3", Reply::ok(b3_line(&binary)));
    server.route("/asset", Reply::ok(binary));
    server
}

fn release_options(server: &FixtureServer, open_project: bool) -> LaunchOptions {
    LaunchOptions {
        open_project,
        env: vec![
            ("ORIN_RELEASE_TARGET".into(), TARGET.into()),
            (
                "ORIN_UPDATE_SOURCES".into(),
                format!("{}|{}", server.url("/github"), server.url("/gitlab")),
            ),
        ],
        ..LaunchOptions::default()
    }
}

fn open_updates(app: &mut AppDriver) -> Result<Value> {
    app.click("open-settings")?;
    app.wait_for("settings page", |state| state["page"] == "settings")?;
    app.click("settings-section-updates")?;
    app.wait_for("updates section", |state| {
        state["settings_page"] == "updates"
    })
}

fn control_enabled(state: &Value, id: &str) -> Option<bool> {
    state["controls"]
        .as_array()?
        .iter()
        .find(|control| control["id"] == id)
        .and_then(|control| control["enabled"].as_bool())
}

fn status(state: &Value) -> &str {
    state["update"]["status"].as_str().unwrap_or_default()
}

/// Check now, the banner, Skip this version, the GitLab fallback, the switch, and the page in
/// every theme.
pub fn banner_and_settings_section() -> Result<()> {
    let server = release_server(b"not used".to_vec());
    let mut app = AppDriver::start_with(release_options(&server, true))?;
    app.resize(1000, 680)?;
    let state = open_updates(&mut app)?;
    ensure!(status(&state) == "Not checked yet.", "{}", state["update"]);
    ensure!(
        server.total_requests() == 0,
        "the test driver never checks by itself"
    );
    app.click("update-check-now")?;
    let next = next_version();
    // The offer reaches the reported state a frame before its buttons do, so wait for both.
    let offer_buttons = ["update-install", "update-skip", "update-dismiss"];
    let state = app.wait_for("offered update and its buttons", |state| {
        state["update"]["offered"] == next.as_str()
            && state["update"]["banner"] == true
            && offer_buttons
                .iter()
                .all(|id| control_enabled(state, id) == Some(true))
    })?;
    ensure!(
        status(&state).starts_with(&format!("Orin {next} is available (checked ")),
        "{}",
        status(&state)
    );
    ensure!(
        server.requests("/gitlab") == 0,
        "GitLab is only the fallback"
    );
    let capture = app.window.capture_settled()?;
    pixels::assert_real_pixels(&capture)?;
    pixels::assert_visible_controls(&capture, &state)?;
    for theme in ["gruvbox_dark", "gruvbox_light", "dark", "light"] {
        app.click("settings-section-appearance")?;
        app.click(&format!("theme-{theme}"))?;
        app.wait_for("theme", |state| state["theme"] == theme)?;
        app.click("settings-section-updates")?;
        let state = app.wait_for("updates again", |state| state["settings_page"] == "updates")?;
        let capture = app.window.capture_settled()?;
        pixels::assert_visible_controls(&capture, &state)?;
        snapshots::assert_snapshot(&format!("settings-updates-{theme}-1000"), &capture)?;
    }
    // Skip this version hides the banner, is saved, and keeps later checks quiet.
    app.click("update-skip")?;
    app.wait_for("banner hidden", |state| state["update"]["banner"] == false)?;
    app.wait_for_saved("skipped version", |saved| {
        saved["updates"]["skipped_version"] == next.as_str()
    })?;
    app.click("update-check-now")?;
    let state = app.wait_for("quiet check", |state| {
        state["update"]["checking"] == false && status(state).contains("you chose to skip it")
    })?;
    ensure!(
        state["update"]["banner"] == false,
        "a skipped version stays quiet"
    );
    // GitHub refusing (a rate limit) falls back to GitLab.
    let before = server.requests("/gitlab");
    server.route("/github", Reply::status(403));
    app.click("update-check-now")?;
    app.wait_for("GitLab answer", |state| {
        state["update"]["checking"] == false && status(state).contains("you chose to skip it")
    })?;
    ensure!(server.requests("/gitlab") == before + 1, "GitLab answered");
    // Switching automatic checks off is saved and asks nobody.
    let requests = server.total_requests();
    app.click("update-auto")?;
    app.wait_for_saved("checks off", |saved| {
        saved["updates"]["check_automatically"] == false
    })?;
    thread::sleep(Duration::from_millis(500));
    ensure!(
        server.total_requests() == requests,
        "no request after switching off"
    );
    app.shutdown()
}

/// Without a release target, the page says the build is from source and its controls are off.
pub fn source_build_has_no_update_controls() -> Result<()> {
    let mut app = AppDriver::start(true)?;
    let state = open_updates(&mut app)?;
    ensure!(
        status(&state).contains("built from source"),
        "{}",
        status(&state)
    );
    for id in ["update-check-now", "update-auto"] {
        ensure!(
            control_enabled(&state, id) == Some(false),
            "{id} must be disabled"
        );
    }
    let capture = app.window.capture_settled()?;
    pixels::assert_visible_controls(&capture, &state)?;
    app.shutdown()
}

/// Quitting while a check hangs still exits promptly: a check only reads, so it is abandoned.
pub fn quit_during_a_hanging_check() -> Result<()> {
    let server = FixtureServer::start(vec![("/github", Reply::hang()), ("/gitlab", Reply::hang())]);
    let mut app = AppDriver::start_with(release_options(&server, true))?;
    open_updates(&mut app)?;
    app.click("update-check-now")?;
    app.wait_for("checking", |state| state["update"]["checking"] == true)?;
    app.shutdown()
}

/// A scratch folder holding a copy of Orin, so installing replaces the copy, not the build.
fn scratch_with_copy() -> Result<(tempfile::TempDir, PathBuf)> {
    // A name no other run uses, so `expect_restart` can't mistake another test's Orin for ours.
    static COPIES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let scratch = tempfile::tempdir()?;
    let copy = scratch.path().join(format!(
        "orin-under-test-{}-{}.exe",
        std::process::id(),
        COPIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::copy(env!("CARGO_BIN_EXE_orin"), &copy)?;
    Ok((scratch, copy))
}

/// The "new" Orin: the same program with a marker appended (Windows ignores trailing bytes),
/// so the replaced file can be told from the old one.
fn new_binary() -> Result<Vec<u8>> {
    let mut binary = fs::read(env!("CARGO_BIN_EXE_orin"))?;
    binary.extend_from_slice(b"ORIN-UPDATE-TEST-MARKER");
    Ok(binary)
}

/// Process IDs running `image` (a file name), from `tasklist`.
fn processes(image: &str) -> Result<Vec<u32>> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {image}"), "/FO", "CSV", "/NH"])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split("\",\"").nth(1)?.parse().ok())
        .collect())
}

/// The command line of process `id`.
fn command_line(id: u32) -> Result<String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!("(Get-CimInstance Win32_Process -Filter 'ProcessId={id}').CommandLine"),
        ])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Wait for the restarted Orin, check how it was started, and close it.
fn expect_restart(scratch: &Path, copy: &Path, old: u32) -> Result<()> {
    let image = copy
        .file_name()
        .context("copy has a name")?
        .to_string_lossy()
        .into_owned();
    let deadline = Instant::now() + Duration::from_secs(30);
    let id = loop {
        if let Some(id) = processes(&image)?.into_iter().find(|id| *id != old) {
            break id;
        }
        if Instant::now() > deadline {
            bail!("the updated Orin didn't start");
        }
        thread::sleep(Duration::from_millis(200));
    };
    let window = super::window::AppWindow::wait(id)?;
    let line = command_line(id)?;
    let project = scratch.join("fixture");
    let settings = scratch.join("settings.json");
    let result = (|| {
        ensure!(
            line.contains(&project.to_string_lossy().into_owned()),
            "same project: {line}"
        );
        ensure!(
            line.contains(&settings.to_string_lossy().into_owned()),
            "same settings: {line}"
        );
        Ok(())
    })();
    window.close();
    let deadline = Instant::now() + Duration::from_secs(15);
    while processes(&image)?.contains(&id) {
        if Instant::now() > deadline {
            let _ = Command::new("taskkill")
                .args(["/PID", &id.to_string(), "/F"])
                .status();
            bail!("the updated Orin didn't close");
        }
        thread::sleep(Duration::from_millis(200));
    }
    result
}

fn installed_marker(copy: &Path) -> Result<bool> {
    let bytes = fs::read(copy)?;
    Ok(bytes.ends_with(b"ORIN-UPDATE-TEST-MARKER"))
}

fn leftover_downloads(scratch: &Path) -> usize {
    fs::read_dir(scratch.join("updates")).map_or(0, |entries| {
        entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("orin-update-")
            })
            .count()
    })
}

/// Update and restart: download, verify, replace the copy, and reopen the same project.
pub fn install_restarts_into_same_project() -> Result<()> {
    let server = release_server(new_binary()?);
    let (scratch, copy) = scratch_with_copy()?;
    let mut options = release_options(&server, true);
    options.executable = Some(copy.clone());
    let mut app = AppDriver::start_in(scratch, options)?;
    let scratch = app.scratch().to_path_buf();
    open_updates(&mut app)?;
    app.click("update-check-now")?;
    app.wait_for("offered update", |state| state["update"]["banner"] == true)?;
    let old = app.child_id();
    app.click("update-install")?;
    let status = app.wait_exit(Duration::from_secs(60))?;
    ensure!(
        status.success(),
        "the old Orin closed cleanly: {}",
        app.logs()
    );
    ensure!(
        installed_marker(&copy)?,
        "the copy on disk is the new binary"
    );
    ensure!(
        leftover_downloads(&scratch) == 0,
        "the download was moved into place"
    );
    expect_restart(&scratch, &copy, old)
}

/// A checksum that doesn't match leaves the binary alone and says why.
pub fn wrong_checksum_is_refused() -> Result<()> {
    let server = release_server(new_binary()?);
    server.route("/asset.b3", Reply::ok(b3_line(b"something else")));
    let (scratch, copy) = scratch_with_copy()?;
    let mut options = release_options(&server, true);
    options.executable = Some(copy.clone());
    let mut app = AppDriver::start_in(scratch, options)?;
    open_updates(&mut app)?;
    app.click("update-check-now")?;
    app.wait_for("offered update", |state| state["update"]["banner"] == true)?;
    app.click("update-install")?;
    let next = next_version();
    let state = app.wait_for("checksum error", |state| {
        state["error"].as_str().is_some_and(|error| {
            error
                == format!(
                    "The download of Orin {next} didn't match its checksum, so it wasn't installed."
                )
        })
    })?;
    ensure!(
        state["update"]["installed"].is_null(),
        "nothing was installed"
    );
    ensure!(!installed_marker(&copy)?, "the binary is untouched");
    ensure!(
        leftover_downloads(app.scratch()) == 0,
        "the bad download was deleted"
    );
    let capture = app.window.capture_settled()?;
    pixels::assert_visible_controls(&capture, &state)?;
    app.shutdown()
}

/// Unsaved edits stop the restart; the update is installed and Discard & close relaunches.
pub fn unsaved_edits_hold_the_restart() -> Result<()> {
    let server = release_server(new_binary()?);
    let (scratch, copy) = scratch_with_copy()?;
    let mut options = release_options(&server, true);
    options.executable = Some(copy.clone());
    let mut app = AppDriver::start_in(scratch, options)?;
    let scratch = app.scratch().to_path_buf();
    app.click("file:README.md")?;
    app.wait_for("README open", |state| state["editor_text"].is_string())?;
    app.click("editor-input")?;
    app.window.type_text("unsaved")?;
    app.wait_for("dirty editor", |state| state["editor_dirty"] == true)?;
    open_updates(&mut app)?;
    app.click("update-check-now")?;
    app.wait_for("offered update", |state| state["update"]["banner"] == true)?;
    app.click("update-install")?;
    let next = next_version();
    let state = app.wait_for("installed, waiting for the edits", |state| {
        state["update"]["installed"] == next.as_str()
            && state["error"]
                .as_str()
                .is_some_and(|error| error.contains("unsaved edits"))
            // Controls are measured when drawn, so wait for the frame that shows the banner.
            && control_enabled(state, "update-restart").is_some()
    })?;
    ensure!(installed_marker(&copy)?, "the update is already in place");
    for id in ["update-restart", "discard-close"] {
        ensure!(
            control_enabled(&state, id) == Some(true),
            "{id} is offered: {:?}, error {}",
            control_enabled(&state, id),
            state["error"]
        );
    }
    let capture = app.window.capture_settled()?;
    pixels::assert_visible_controls(&capture, &state)?;
    let old = app.child_id();
    app.click("discard-close")?;
    let status = app.wait_exit(Duration::from_secs(30))?;
    ensure!(status.success(), "closed cleanly: {}", app.logs());
    expect_restart(&scratch, &copy, old)
}

/// Quitting in the middle of a download cancels it and leaves no file behind.
pub fn quit_mid_download_leaves_nothing() -> Result<()> {
    let binary = new_binary()?;
    let server = release_server(binary.clone());
    server.route("/asset", Reply::ok(binary).paced(Duration::from_millis(40)));
    let (scratch, copy) = scratch_with_copy()?;
    let mut options = release_options(&server, true);
    options.executable = Some(copy.clone());
    let mut app = AppDriver::start_in(scratch, options)?;
    open_updates(&mut app)?;
    app.click("update-check-now")?;
    app.wait_for("offered update", |state| state["update"]["banner"] == true)?;
    app.click("update-install")?;
    let state = app.wait_for("downloading", |state| {
        state["update"]["downloading"]
            .as_u64()
            .is_some_and(|read| read > 0)
    })?;
    ensure!(
        control_enabled(&state, "update-cancel") == Some(true),
        "Cancel is offered while downloading"
    );
    let scratch = app.scratch().to_path_buf();
    app.shutdown()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while leftover_downloads(&scratch) > 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    ensure!(
        leftover_downloads(&scratch) == 0,
        "the partial download was deleted"
    );
    ensure!(!installed_marker(&copy)?, "nothing was installed");
    Ok(())
}

/// Cancel stops the download, deletes its file, and offers the release again.
pub fn cancel_mid_download_leaves_nothing() -> Result<()> {
    let binary = new_binary()?;
    let server = release_server(binary.clone());
    server.route("/asset", Reply::ok(binary).paced(Duration::from_millis(40)));
    let (scratch, copy) = scratch_with_copy()?;
    let mut options = release_options(&server, true);
    options.executable = Some(copy.clone());
    let mut app = AppDriver::start_in(scratch, options)?;
    let scratch = app.scratch().to_path_buf();
    open_updates(&mut app)?;
    app.click("update-check-now")?;
    app.wait_for("offered update", |state| state["update"]["banner"] == true)?;
    app.click("update-install")?;
    app.wait_for("downloading", |state| {
        state["update"]["downloading"]
            .as_u64()
            .is_some_and(|read| read > 0)
    })?;
    app.click("update-cancel")?;
    let state = app.wait_for("download cancelled", |state| {
        state["update"]["downloading"].is_null()
            && control_enabled(state, "update-install") == Some(true)
    })?;
    ensure!(
        state["error"].is_null(),
        "a cancel is not an error: {}",
        state["error"]
    );
    let capture = app.window.capture_settled()?;
    pixels::assert_visible_controls(&capture, &state)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while leftover_downloads(&scratch) > 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    ensure!(
        leftover_downloads(&scratch) == 0,
        "the partial download was deleted"
    );
    ensure!(!installed_marker(&copy)?, "nothing was installed");
    app.shutdown()
}
