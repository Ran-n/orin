// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:15.817650
//+ Revised: 	2026/11/12 09:16:50.160664
// -----------------------------------------------------------------------------

use std::fs;

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::{driver::AppDriver, pixels, snapshots};

pub fn run() -> Result<()> {
    for (width, height) in [(1280, 800), (1000, 680)] {
        let mut app = AppDriver::start(true)?;
        app.resize(width, height)?;
        open_source(&mut app)?;
        let edited = edit_and_undo(&mut app)?;
        app.click("editor-save")?;
        app.wait_for("saved editor", |state| state["editor_dirty"] == false)?;
        ensure!(
            fs::read_to_string(app.fixture_file("src/main.rs"))? == edited,
            "Save wrote different content"
        );
        external_conflict(&mut app, width)?;
        app.shutdown()?;
    }
    Ok(())
}

fn open_source(app: &mut AppDriver) -> Result<()> {
    let state = app.state()?;
    let visible = state["controls"]
        .as_array()
        .context("missing controls")?
        .iter()
        .any(|control| control["id"] == "file:src/main.rs");
    if !visible {
        app.click("folder:src")?;
    }
    app.click("file:src/main.rs")?;
    app.wait_for("source editor", |state| {
        state["editor_text"]
            .as_str()
            .is_some_and(|text| text.contains("after"))
    })?;
    Ok(())
}

fn edit_and_undo(app: &mut AppDriver) -> Result<String> {
    app.click("editor-input")?;
    app.window.type_text(" // intención Ω")?;
    let state = app.wait_for("Unicode editor input", |state| {
        state["editor_text"]
            .as_str()
            .is_some_and(|text| text.contains("intención Ω"))
    })?;
    ensure!(
        state["editor_dirty"] == true,
        "native edit did not mark document dirty"
    );
    let edited = editor_text(&state)?.to_owned();
    app.click("editor-undo")?;
    app.wait_for("editor undo", |state| state["editor_text"] != edited)?;
    app.click("editor-redo")?;
    app.wait_for("editor redo", |state| state["editor_text"] == edited)?;
    Ok(edited)
}

fn external_conflict(app: &mut AppDriver, width: u32) -> Result<()> {
    const EXTERNAL: &str = "// Changed outside Orin\nfn main() {}\n";
    fs::write(app.fixture_file("src/main.rs"), EXTERNAL)?;
    app.click("editor-input")?;
    app.window.type_text(" local")?;
    let state = app.wait_for("second local edit", |state| state["editor_dirty"] == true)?;
    let local = editor_text(&state)?.to_owned();
    app.click("editor-save")?;
    let state = app.wait_for("visible external change conflict", |state| {
        state["editor_conflict"] == true
    })?;
    ensure!(
        editor_text(&state)? == local,
        "conflict discarded unsaved edits"
    );
    ensure!(
        fs::read_to_string(app.fixture_file("src/main.rs"))? == EXTERNAL,
        "conflict overwrote external changes"
    );
    let capture = app.window.capture_settled()?;
    pixels::assert_real_pixels(&capture)?;
    pixels::assert_visible_controls(&capture, &state)?;
    snapshots::assert_snapshot(&format!("editor-conflict-{width}"), &capture)?;
    app.click("editor-reload")?;
    app.wait_for("explicit conflict reload", |state| {
        state["editor_text"] == EXTERNAL
            && state["editor_dirty"] == false
            && state["editor_conflict"] == false
    })?;
    Ok(())
}

fn editor_text(state: &Value) -> Result<&str> {
    state["editor_text"]
        .as_str()
        .context("state.editor_text must be a string")
}
