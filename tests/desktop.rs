// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:22.358859
//+ Revised: 	2026/11/14 12:09:20.629690
// -----------------------------------------------------------------------------

#![cfg(all(target_os = "windows", feature = "desktop"))]

mod rendered;

use anyhow::{Context, Result, ensure};
use rendered::{driver::AppDriver, pixels, snapshots};
use serde_json::Value;

const VIEWPORTS: [(u32, u32); 2] = [(1280, 800), (1000, 680)];

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn custom_agent_folder_is_used_and_can_be_removed() -> Result<()> {
    let scratch = tempfile::tempdir()?;
    let tools = scratch.path().join("agent tools Ω");
    std::fs::create_dir(&tools)?;
    std::fs::write(
        tools.join("claude.cmd"),
        "@echo off\r\necho {\"loggedIn\":true,\"authMethod\":\"fixture\"}\r\n",
    )?;
    std::fs::write(tools.join("npx.cmd"), "@exit /b 0\r\n")?;
    std::fs::write(
        scratch.path().join("settings.json"),
        serde_json::to_vec(&serde_json::json!({"tool_directories":[tools]}))?,
    )?;
    let mut app = AppDriver::start_in(
        scratch,
        rendered::fixture::LaunchOptions {
            open_project: true,
            ..Default::default()
        },
    )?;
    app.click("open-settings")?;
    app.click("settings-section-accounts")?;
    app.wait_for("configured Claude installation", |state| {
        state["accounts"].as_array().is_some_and(|accounts| {
            accounts.iter().any(|account| {
                account["id"] == "claude" && account["sign_in"]["state"] == "signed_in"
            })
        }) && has_control(state, "agent-remove-tool-folder-0")
    })?;
    app.click("agent-remove-tool-folder-0")?;
    app.wait_for("removed tool folder", |state| {
        state["accounts"].as_array().is_some_and(|accounts| {
            accounts
                .iter()
                .any(|account| account["id"] == "claude" && account["sign_in"]["state"] == "no_cli")
        })
    })?;
    app.wait_for_saved("removed tool folder", |saved| {
        saved.get("tool_directories").is_none()
    })?;
    app.shutdown()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn first_open_shows_welcome_with_recent_folders_and_opens_one() -> Result<()> {
    let mut app = AppDriver::start_with_recent_projects()?;
    for (width, height) in VIEWPORTS {
        app.resize(width, height)?;
        let state = app.wait_for("welcome page", |state| {
            state["page"] == "welcome" && has_control(state, "recent-1")
        })?;
        ensure!(
            state["project_root"].is_null(),
            "test must not load the user's project"
        );
        for id in [
            "welcome-page",
            "welcome-open-folder",
            "welcome-settings",
            "welcome-step-theme",
            "welcome-step-agents",
            "welcome-step-layout",
            "recent-0",
            "recent-remove-0",
            "open-welcome",
            "command-center",
            "open-accounts",
        ] {
            ensure!(has_control(&state, id), "welcome control {id} is absent");
        }
        ensure!(
            !has_control(&state, "welcome-back"),
            "no project to go back to"
        );
        let capture = app.window.capture_settled()?;
        pixels::assert_real_pixels(&capture)?;
        pixels::assert_visible_controls(&capture, &state)?;
        snapshots::assert_snapshot(&format!("welcome-{width}"), &capture)?;
    }
    // Remove the deleted folder, then open the fixture from the list.
    app.click("recent-remove-1")?;
    app.wait_for("forgotten folder", |state| {
        state["recent_projects"]
            .as_array()
            .is_some_and(|recent| recent.len() == 1)
    })?;
    app.click("recent-0")?;
    let state = app.wait_for("project opened from Recent", |state| {
        !state["project_root"].is_null() && state["page"].is_null()
    })?;
    ensure!(
        state["recent_projects"][0]
            .as_str()
            .is_some_and(|path| path.ends_with("fixture")),
        "opened folder is first in Recent: {}",
        state["recent_projects"]
    );
    // The shell starts with the project.
    app.wait_for("terminal autostart", |state| {
        state["terminal"]["running"] == true
    })?;
    // An automatically started shell must not prevent project navigation.
    let state = app.wait_for("open remains enabled while the terminal runs", |state| {
        control_enabled(state, "open-project") == Some(true)
    })?;
    ensure!(
        state["error"].is_null(),
        "nothing was refused: {}",
        state["error"]
    );
    // Home closes the project, including its automatically started shell.
    app.click("open-welcome")?;
    let state = app.wait_for("welcome again", |state| {
        state["page"] == "welcome"
            && state["project_root"].is_null()
            && control_enabled(state, "recent-0") == Some(true)
    })?;
    ensure!(
        control_enabled(&state, "welcome-open-folder") == Some(true)
            && state["terminal"]["running"] == false,
        "Home must stop the shell and leave Recent usable"
    );
    app.wait_for_saved("closed folder", |saved| saved["project"].is_null())?;
    app.click("recent-0")?;
    app.wait_for("back to the project", |state| state["page"].is_null())?;
    app.wait_for("reopened shell", |state| {
        state["terminal"]["running"] == true
    })?;
    app.click("close-project")?;
    app.wait_for("explicit Close Folder", |state| {
        state["project_root"].is_null() && control_enabled(state, "recent-0") == Some(true)
    })?;
    app.click("recent-0")?;
    app.wait_for("reopened from Recent again", |state| {
        !state["project_root"].is_null() && state["page"].is_null()
    })?;
    // Saves can trail the state, so wait for the file to show the remembered folder.
    app.wait_for_saved("recent folders", |saved| {
        saved["recent_projects"]
            .as_array()
            .is_some_and(|recent| recent.len() == 1)
            && saved["page"].is_null()
    })?;
    app.shutdown()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn project_workspace_has_visible_panes_and_disabled_unavailable_agents() -> Result<()> {
    let mut app = AppDriver::start(true)?;
    for (width, height) in VIEWPORTS {
        app.resize(width, height)?;
        let state = app.wait_for("autostarted shell", |state| {
            state["terminal"]["running"] == true
                && state["terminal"]["output"]
                    .as_str()
                    .is_some_and(|output| output.contains('>'))
        })?;
        let controls = state["controls"]
            .as_array()
            .context("missing control array")?;
        assert_columns_side_by_side(&state, width as f64)?;
        // Like VS Code: Git decorations in Files and the branch in the status bar.
        ensure!(
            state["git"]["branch"] == "main",
            "branch is shown: {}",
            state["git"]
        );
        ensure!(
            state["git"]["files"]["README.md"] == "A"
                && state["git"]["files"]["src/main.rs"] == "A",
            "staged new files are marked A: {}",
            state["git"]
        );
        for id in [
            "git-README.md",
            "git-src",
            "status-branch",
            "activity-files",
            "activity-code",
            "activity-agents",
            "activity-terminal",
            "open-accounts",
            "open-settings",
        ] {
            ensure!(
                has_control(&state, id),
                "VS Code-style control {id} is absent"
            );
        }
        for id in [
            "open-project",
            "collapse-files",
            "collapse-agents",
            "collapse-code",
            "collapse-terminal",
        ] {
            ensure!(
                controls.iter().any(|control| control["id"] == id),
                "workspace control {id} is absent"
            );
        }
        for id in [
            "agent-claude",
            "agent-codex",
            "agent-gemini",
            // The empty Agents column repeats the launchers as labelled buttons.
            "start-claude",
            "start-codex",
            "start-gemini",
        ] {
            let control = controls
                .iter()
                .find(|control| control["id"] == id)
                .with_context(|| format!("missing {id}"))?;
            ensure!(
                control["enabled"] == false,
                "unavailable agent {id} is actionable"
            );
        }
        // With agents that can't start, the column offers the way to set them up.
        ensure!(
            control_enabled(&state, "setup-agents") == Some(true),
            "Set up agents is absent or disabled"
        );
        ensure!(
            state["git"]["ignored"]
                .as_array()
                .is_some_and(|ignored| ignored.iter().any(|path| path == "target")),
            "ignored build output is reported: {}",
            state["git"]
        );
        ensure!(
            !has_control(&state, "git-target"),
            "an ignored folder gets no Git badge"
        );
        let capture = app.window.capture_settled()?;
        pixels::assert_real_pixels(&capture)?;
        pixels::assert_visible_controls(&capture, &state)?;
        pixels::assert_disabled_agents(&capture, &state)?;
        // File-type and folder-role icons in the default GRUVBOX_DARK palette.
        for (id, rgb) in [
            ("file:README.md", [0x83, 0xa5, 0x98]),  // Readme: syntax.link
            ("file:Cargo.toml", [0xfa, 0xbd, 0x2f]), // Config: syntax.type_name
            ("file:.gitignore", [0xb8, 0xbb, 0x26]), // Git: syntax.string
            ("folder:src", [0xb8, 0xbb, 0x26]),      // Source: accent
            ("folder:tests", [0xfa, 0xbd, 0x2f]),    // Tests: syntax.type_name
        ] {
            pixels::assert_icon_color(&capture, &state, id, rgb)?;
        }
        // Ignored `target/` has a grey name: GRUVBOX_DARK git.ignored.
        pixels::assert_label_color(&capture, &state, "folder:target", [0x92, 0x83, 0x74])?;
        snapshots::assert_snapshot(&format!("workspace-{width}"), &capture)?;
    }
    // An opened well-known folder keeps its glyph and colour.
    if !has_control(&app.state()?, "file:src/main.rs") {
        app.click("folder:src")?;
    }
    let state = app.wait_for("src expanded", |state| {
        has_control(state, "file:src/main.rs")
    })?;
    let capture = app.window.capture_settled()?;
    pixels::assert_icon_color(&capture, &state, "folder:src", [0xb8, 0xbb, 0x26])?;
    pixels::assert_icon_color(&capture, &state, "file:src/main.rs", [0xfb, 0x49, 0x34])?; // Code: syntax.keyword
    app.click("folder:src")?;
    app.wait_for("src collapsed", |state| {
        !has_control(state, "file:src/main.rs")
    })?;
    // The activity bar hides and shows a column, and is remembered like the column's « button.
    app.click("activity-terminal")?;
    app.wait_for("terminal hidden from the activity bar", |state| {
        state["columns"]["terminal"]["collapsed"] == true
    })?;
    app.click("activity-terminal")?;
    app.wait_for("terminal shown again", |state| {
        state["columns"]["terminal"]["collapsed"] == false
    })?;
    // Settings is a page; a column's activity icon returns to the columns.
    app.click("open-settings")?;
    app.wait_for("settings page", |state| state["page"] == "settings")?;
    app.click("activity-code")?;
    app.wait_for("back to the columns", |state| {
        state["page"].is_null() && state["columns"]["code"]["collapsed"] == false
    })?;
    app.shutdown()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn account_menu_lists_agent_sign_ins_and_opens_accounts() -> Result<()> {
    let mut app = AppDriver::start(true)?;
    app.resize(1280, 800)?;
    app.click("open-accounts")?;
    let state = app.wait_for("account menu", |state| {
        has_control(state, "account-menu-manage")
    })?;
    for id in [
        "account-menu-agent-claude",
        "account-menu-agent-codex",
        "account-menu-agent-gemini",
    ] {
        ensure!(has_control(&state, id), "account menu entry {id} is absent");
    }
    ensure!(
        control_enabled(&state, "account-menu-sync") == Some(false),
        "settings sync is a placeholder and must look disabled"
    );
    let capture = app.window.capture_settled()?;
    pixels::assert_real_pixels(&capture)?;
    pixels::assert_visible_controls(&capture, &state)?;
    snapshots::assert_snapshot("account-menu-1280", &capture)?;
    app.click("account-menu-manage")?;
    let state = app.wait_for("accounts settings", |state| {
        state["settings_page"] == "accounts" && !has_control(state, "account-menu-manage")
    })?;
    ensure!(state["page"] == "settings", "the Settings page is open");
    app.shutdown()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn explorer_native_click_selects_a_file_and_cycles_diff_views() -> Result<()> {
    let mut app = AppDriver::start(true)?;
    for (width, height) in VIEWPORTS {
        app.resize(width, height)?;
        let state = app.state()?;
        if !has_control(&state, "file:src/main.rs") {
            app.click("folder:src")?;
        }
        app.click("file:src/main.rs")?;
        let state = app.wait_for("selected source file", |state| {
            state["selected_file"]
                .as_str()
                .is_some_and(|file| file.ends_with("src/main.rs"))
        })?;
        ensure!(
            state["viewer_mode"] == "file",
            "file selection must show file contents"
        );
        for expected in ["unstaged", "staged", "file"] {
            app.click(&format!("view-{expected}"))?;
            app.wait_for("diff mode change", |state| state["viewer_mode"] == expected)?;
            if expected == "unstaged" {
                app.wait_for("real unstaged Git diff", |state| {
                    state["viewer_text"].as_str().is_some_and(|text| {
                        text.contains("-    println!(\"before\")")
                            && text.contains("+    println!(\"after\")")
                    })
                })?;
            }
            if expected == "staged" {
                app.wait_for("real staged Git diff", |state| {
                    state["viewer_text"].as_str().is_some_and(|text| {
                        text.contains("+fn main()") && text.contains("+    println!(\"before\")")
                    })
                })?;
            }
        }
        app.wait_for("loaded editor contents", |state| {
            state["editor_text"]
                .as_str()
                .is_some_and(|text| text.contains("after"))
        })?;
        let state = app.wait_for("highlighted source with its language", |state| {
            has_control(state, "status-language") && has_control(state, "close-file")
        })?;
        let capture = app.window.capture_settled()?;
        pixels::assert_real_pixels(&capture)?;
        pixels::assert_visible_controls(&capture, &state)?;
        snapshots::assert_snapshot(&format!("source-{width}"), &capture)?;
    }
    // The tab's close button closes the file, like closing a VS Code editor tab.
    app.click("close-file")?;
    let state = app.wait_for("file closed", |state| state["selected_file"].is_null())?;
    ensure!(
        !has_control(&state, "status-language"),
        "no language without an open file"
    );
    app.shutdown()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn terminal_native_input_runs_a_command_and_stop_ends_the_shell() -> Result<()> {
    let mut app = AppDriver::start(true)?;
    for (width, height) in VIEWPORTS {
        app.resize(width, height)?;
        if has_control(&app.state()?, "expand-terminal") {
            app.click("expand-terminal")?;
        }
        // The shell starts by itself with the project; after a stop it is started again by hand.
        if app.state()?["terminal"]["running"] != true {
            app.click("terminal-start")?;
        }
        app.wait_for("shell start", |state| state["terminal"]["running"] == true)?;
        app.click("terminal-input")?;
        app.window
            .type_text("prompt $G & cls & echo ORIN_TERMINAL_NATIVE_OK")?;
        app.window.enter()?;
        app.wait_for("command output", |state| {
            state["terminal"]["output"].as_str().is_some_and(|output| {
                output
                    .lines()
                    .any(|line| line.trim() == "ORIN_TERMINAL_NATIVE_OK")
            })
        })?;
        let capture = app.window.capture_settled()?;
        pixels::assert_real_pixels(&capture)?;
        pixels::assert_visible_controls(&capture, &app.state()?)?;
        snapshots::assert_snapshot(&format!("terminal-{width}"), &capture)?;
        app.click("terminal-stop")?;
        app.wait_for("shell stop", |state| state["terminal"]["running"] == false)?;
    }
    app.shutdown()
}

fn has_control(state: &Value, id: &str) -> bool {
    state["controls"]
        .as_array()
        .is_some_and(|controls| controls.iter().any(|control| control["id"] == id))
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn editor_native_input_saves_undoes_and_preserves_external_conflicts() -> Result<()> {
    rendered::editor_flow::run()
}

#[cfg(feature = "agent-fixture")]
#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn structured_agent_native_prompt_permission_review_and_cancel() -> Result<()> {
    rendered::agent_flow::run()
}

#[cfg(feature = "agent-fixture")]
#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn agent_model_picker_changes_and_remembers_the_model() -> Result<()> {
    rendered::agent_flow::model_picker()
}

/// Every expanded column is on screen at once: same top edge, left to right in the saved order,
/// no overlap, at least its minimum width, and together they span the window.
fn assert_columns_side_by_side(state: &Value, window_width: f64) -> Result<()> {
    let controls = state["controls"].as_array().context("missing controls")?;
    let column = |key: &str| {
        controls
            .iter()
            .find(|control| control["id"] == format!("column-{key}"))
            .with_context(|| format!("column {key} is not visible"))
    };
    let minimum = |key: &str| match key {
        "files" => 140.0,
        "agents" => 300.0,
        "code" => 220.0,
        _ => 200.0,
    };
    let order: Vec<&str> = state["column_order"]
        .as_array()
        .context("missing column order")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    ensure!(order.len() == 4, "column order is incomplete: {order:?}");
    // The columns start right of the activity bar, as VS Code's views do.
    let activity = controls
        .iter()
        .find(|control| control["id"] == "activity-files")
        .context("activity bar is not visible")?;
    let activity_right =
        activity["x"].as_f64().unwrap_or(-1.0) + activity["width"].as_f64().unwrap_or(0.0);
    ensure!(
        activity["x"].as_f64() == Some(0.0) && activity_right >= 40.0,
        "activity bar is not on the left edge: {activity}"
    );
    let mut previous_right = activity_right;
    let mut top = None;
    for key in order {
        let minimum = minimum(key);
        let bounds = column(key)?;
        let (x, y, width) = (
            bounds["x"].as_f64().unwrap_or(-1.0),
            bounds["y"].as_f64().unwrap_or(-1.0),
            bounds["width"].as_f64().unwrap_or(0.0),
        );
        ensure!(
            width >= minimum - 1.0,
            "{key} is {width}px, below its {minimum}px minimum"
        );
        ensure!(
            x >= previous_right - 1.0,
            "{key} overlaps the column to its left"
        );
        let top = *top.get_or_insert(y);
        ensure!(
            (y - top).abs() < 1.0,
            "{key} is not beside the other columns"
        );
        previous_right = x + width;
    }
    ensure!(
        (previous_right - window_width).abs() <= 2.0,
        "columns end at {previous_right}px in a {window_width}px window"
    );
    Ok(())
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn columns_resize_collapse_and_restore_between_runs() -> Result<()> {
    let mut app = AppDriver::start(true)?;
    app.resize(1280, 800)?;
    let before = app.state()?["columns"]["files"]["width"]
        .as_f64()
        .context("files width")?;
    app.drag("divider-files-code", -40.0)?;
    let state = app.wait_for("files column widened", |state| {
        state["columns"]["files"]["width"]
            .as_f64()
            .is_some_and(|width| (width - (before - 40.0)).abs() < 3.0)
    })?;
    assert_columns_side_by_side(&state, 1280.0)?;
    app.click("collapse-terminal")?;
    let state = app.wait_for("terminal collapsed", |state| {
        state["columns"]["terminal"]["collapsed"] == true
    })?;
    ensure!(
        has_control(&state, "rail-terminal"),
        "collapsed terminal shows no rail"
    );
    ensure!(
        !has_control(&state, "column-terminal"),
        "collapsed terminal still renders"
    );
    let capture = app.window.capture_settled()?;
    pixels::assert_real_pixels(&capture)?;
    pixels::assert_visible_controls(&capture, &state)?;
    snapshots::assert_snapshot("columns-collapsed-1280", &capture)?;
    app.click("expand-terminal")?;
    app.wait_for("terminal expanded", |state| {
        state["columns"]["terminal"]["collapsed"] == false
    })?;
    app.click("collapse-terminal")?;
    app.wait_for("terminal collapsed again", |state| {
        state["columns"]["terminal"]["collapsed"] == true
    })?;
    app.shutdown()?;
    let saved = app.saved_settings()?;
    ensure!(
        saved["schema_version"] == 2,
        "settings were not saved as schema 2: {saved}"
    );
    ensure!(
        saved["columns"]["files"]["width"]
            .as_f64()
            .is_some_and(|width| (width - (before - 40.0)).abs() < 3.0),
        "column width was not remembered: {saved}"
    );
    ensure!(
        saved["columns"]["terminal"]["collapsed"] == true,
        "collapsed column was not remembered: {saved}"
    );
    Ok(())
}

fn column_order(state: &Value) -> Vec<String> {
    state["column_order"]
        .as_array()
        .map(|order| {
            order
                .iter()
                .filter_map(|key| key.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn control_enabled(state: &Value, id: &str) -> Option<bool> {
    state["controls"]
        .as_array()?
        .iter()
        .find(|control| control["id"] == id)?["enabled"]
        .as_bool()
}

fn control_height(state: &Value, id: &str) -> Result<f64> {
    state["controls"]
        .as_array()
        .and_then(|controls| controls.iter().find(|control| control["id"] == id))
        .and_then(|control| control["height"].as_f64())
        .with_context(|| format!("{id} height"))
}

fn first_control_with_prefix(state: &Value, prefix: &str) -> Option<String> {
    state["controls"]
        .as_array()?
        .iter()
        .filter_map(|control| control["id"].as_str())
        .find(|id| id.starts_with(prefix))
        .map(str::to_owned)
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn columns_move_and_settings_page_personalize_and_are_remembered() -> Result<()> {
    let mut app = AppDriver::start(true)?;
    app.resize(1280, 800)?;
    let state = app.state()?;
    ensure!(
        state["theme"] == "gruvbox_dark",
        "Gruvbox Dark is not the default theme: {}",
        state["theme"]
    );
    // Drag Terminal by its title to the far left.
    app.drag("move-terminal", -1150.0)?;
    let state = app.wait_for("terminal moved first", |state| {
        column_order(state).first().map(String::as_str) == Some("terminal")
    })?;
    assert_columns_side_by_side(&state, 1280.0)?;
    let capture = app.window.capture_settled()?;
    pixels::assert_real_pixels(&capture)?;
    pixels::assert_visible_controls(&capture, &state)?;
    snapshots::assert_snapshot("columns-moved-1280", &capture)?;

    // A click on a title without moving changes nothing.
    app.click("move-files")?;
    let state = app.state()?;
    ensure!(
        column_order(&state) == ["terminal", "files", "code", "agents"],
        "a click moved a column: {:?}",
        column_order(&state)
    );

    let before = control_height(&state, "open-project")?;
    app.click("open-settings")?;
    let state = app.wait_for("settings page", |state| {
        state["settings_page"] == "appearance"
    })?;
    ensure!(
        !has_control(&state, "column-files"),
        "the columns stay under the settings page"
    );
    for (width, height) in VIEWPORTS {
        app.resize(width, height)?;
        let state = app.state()?;
        let capture = app.window.capture_settled()?;
        pixels::assert_real_pixels(&capture)?;
        pixels::assert_visible_controls(&capture, &state)?;
        snapshots::assert_snapshot(&format!("settings-appearance-{width}"), &capture)?;
    }
    app.resize(1280, 800)?;

    app.click("theme-light")?;
    let state = app.wait_for("light theme", |state| state["theme"] == "light")?;
    let capture = app.window.capture_settled()?;
    let corner = capture.get_pixel(capture.width() / 2, capture.height() - 4);
    ensure!(
        corner.0.iter().all(|channel| *channel > 200),
        "light theme status bar is not light: {corner:?}"
    );
    pixels::assert_visible_controls(&capture, &state)?;
    snapshots::assert_snapshot("settings-appearance-light-1280", &capture)?;

    app.click("text-larger")?;
    app.click("text-larger")?;
    app.wait_for("larger text", |state| state["text_size"] == 15.0)?;

    app.click("font-code-toggle")?;
    let state = app.wait_for("font list", |state| {
        state["font_count"].as_u64().is_some_and(|count| count > 0)
    })?;
    let option = first_control_with_prefix(&state, "font-code-option:")
        .context("no installed font is offered")?;
    let font = option.trim_start_matches("font-code-option:").to_owned();
    app.click(&option)?;
    let state = app.wait_for("code font chosen", |state| {
        state["code_font"] == font.as_str()
    })?;
    ensure!(
        first_control_with_prefix(&state, "font-code-option:").is_none(),
        "the font list stays open after choosing"
    );

    app.click("settings-section-layout")?;
    let state = app.wait_for("layout section", |state| state["settings_page"] == "layout")?;
    ensure!(
        control_enabled(&state, "reset-layout") == Some(true),
        "reset layout is not offered after a move"
    );
    app.click("reset-layout")?;
    let state = app.wait_for("default layout", |state| {
        column_order(state) == ["files", "code", "agents", "terminal"]
    })?;
    ensure!(
        control_enabled(&state, "reset-layout") == Some(false),
        "reset layout stays actionable after resetting"
    );
    ensure!(
        state["theme"] == "light" && state["text_size"] == 15.0,
        "resetting the layout changed the appearance"
    );

    app.click("settings-close")?;
    let state = app.wait_for("columns again", |state| state["settings_page"].is_null())?;
    let after = control_height(&state, "open-project")?;
    ensure!(
        after > before + 2.0,
        "controls did not grow with the text: {before} -> {after}"
    );
    assert_columns_side_by_side(&state, 1280.0)?;
    let capture = app.window.capture_settled()?;
    pixels::assert_visible_controls(&capture, &state)?;
    snapshots::assert_snapshot("text-15-light-1280", &capture)?;

    // Move Files to the far right so the saved order is not the default.
    // Far enough to pass every other column, short of the window edge (the activity bar is on the left).
    app.drag("move-files", 1100.0)?;
    app.wait_for("files moved last", |state| {
        column_order(state).last().map(String::as_str) == Some("files")
    })?;
    app.shutdown()?;
    let saved = app.saved_settings()?;
    ensure!(
        saved["columns"]["order"] == serde_json::json!(["code", "agents", "terminal", "files"]),
        "column order was not remembered: {saved}"
    );
    ensure!(
        saved["appearance"]["theme"] == "light"
            && saved["appearance"]["text_size"] == 15.0
            && saved["appearance"]["code_font"] == font.as_str(),
        "appearance was not remembered: {saved}"
    );
    Ok(())
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn accounts_show_agent_sign_ins_and_start_one_in_the_terminal() -> Result<()> {
    let mut app = AppDriver::start(true)?;
    app.resize(1280, 800)?;
    // The person button opens the Accounts menu; its last entry opens Settings › Accounts.
    app.click("open-accounts")?;
    app.wait_for("account menu", |state| {
        has_control(state, "account-menu-manage")
    })?;
    app.click("account-menu-manage")?;
    // Controls are measured when drawn, so wait for the frame that shows the checked accounts.
    let state = app.wait_for("accounts checked and drawn", |state| {
        state["settings_page"] == "accounts"
            && state["accounts"].is_array()
            && control_enabled(state, "account-sign-in-codex").is_some()
            && !state["accounts_checking"].as_bool().unwrap_or(true)
            && state["controls"].as_array().is_some_and(|controls| {
                controls
                    .iter()
                    .any(|control| control["id"] == "account-refresh" && control["enabled"] == true)
            })
    })?;
    let sign_in = |state: &Value, id: &str| {
        state["accounts"]
            .as_array()
            .and_then(|accounts| accounts.iter().find(|account| account["id"] == id))
            .map(|account| account["sign_in"]["state"].clone())
    };
    ensure!(
        sign_in(&state, "claude") == Some("no_cli".into())
            && sign_in(&state, "gemini") == Some("no_cli".into())
            && sign_in(&state, "codex") == Some("signed_out".into()),
        "unexpected account states: {}",
        state["accounts"]
    );
    ensure!(
        control_enabled(&state, "account-sign-in-claude") == Some(false)
            && control_enabled(&state, "account-sign-in-codex") == Some(true),
        "sign-in is offered for a missing CLI or withheld for an installed one: claude {:?}, codex {:?}",
        control_enabled(&state, "account-sign-in-claude"),
        control_enabled(&state, "account-sign-in-codex")
    );
    // A missing CLI gets its installation guide on its own card; an installed one doesn't.
    ensure!(
        control_enabled(&state, "agent-install-claude") == Some(true)
            && control_enabled(&state, "agent-install-gemini") == Some(true)
            && !has_control(&state, "agent-install-codex"),
        "the installation guide is offered exactly for the agents whose CLI is missing"
    );
    ensure!(
        control_enabled(&state, "agent-add-tool-folder") == Some(true),
        "Add tool folder is absent or disabled"
    );
    for (width, height) in VIEWPORTS {
        app.resize(width, height)?;
        let state = app.state()?;
        for id in ["claude", "codex", "gemini"] {
            let card = control_height(&state, &format!("account-card-{id}"))?;
            ensure!(
                card < 90.0,
                "the {id} account card is {card}px tall at {width}px; cards hold two lines"
            );
        }
        let capture = app.window.capture_settled()?;
        pixels::assert_real_pixels(&capture)?;
        pixels::assert_visible_controls(&capture, &state)?;
        snapshots::assert_snapshot(&format!("settings-accounts-{width}"), &capture)?;
    }
    app.resize(1280, 800)?;
    app.click("account-sign-in-codex")?;
    let state = app.wait_for("codex login in the terminal", |state| {
        state["settings_page"].is_null()
            && state["terminal"]["output"]
                .as_str()
                .is_some_and(|output| output.contains("FAKE CODEX LOGIN"))
    })?;
    ensure!(
        state["columns"]["terminal"]["collapsed"] == false,
        "the terminal is hidden while the sign-in runs"
    );
    ensure!(
        !state["terminal"]["output"]
            .as_str()
            .unwrap_or_default()
            .contains("oauth"),
        "a real agent login started instead of the checked stand-in"
    );
    app.shutdown()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn markdown_preview_toggles_renders_and_is_remembered() -> Result<()> {
    let mut app = AppDriver::start(true)?;
    app.resize(1280, 800)?;
    app.click("folder:src")?;
    app.click("file:src/main.rs")?;
    let state = app.wait_for("main.rs open", |state| {
        state["selected_file"] == "src/main.rs" && state["editor_text"].is_string()
    })?;
    ensure!(
        !has_control(&state, "markdown-preview-toggle"),
        "the preview button is offered for a Rust file"
    );
    app.click("file:README.md")?;
    let state = app.wait_for("README open as source", |state| {
        state["selected_file"] == "README.md" && state["editor_text"].is_string()
    })?;
    ensure!(
        has_control(&state, "markdown-preview-toggle") && state["previewing"] == false,
        "a Markdown file opens as source with a preview button: {}",
        state["previewing"]
    );
    app.click("markdown-preview-toggle")?;
    let state = app.wait_for("rendered README", |state| state["previewing"] == true)?;
    let text = state["preview_text"].as_str().context("preview text")?;
    ensure!(
        text.contains("Fixture") && text.contains("Code | Read and edit") && !text.contains("# "),
        "preview shows Markdown syntax instead of rendering it: {text}"
    );
    ensure!(
        control_enabled(&state, "editor-undo") != Some(true),
        "undo is offered while the source is hidden"
    );
    for (width, height) in VIEWPORTS {
        app.resize(width, height)?;
        let state = app.state()?;
        ensure!(
            has_control(&state, "markdown-preview"),
            "the preview is not on screen at {width}px"
        );
        let capture = app.window.capture_settled()?;
        pixels::assert_real_pixels(&capture)?;
        pixels::assert_visible_controls(&capture, &state)?;
        snapshots::assert_snapshot(&format!("markdown-preview-{width}"), &capture)?;
    }
    app.resize(1280, 800)?;
    app.click("open-settings")?;
    app.click("theme-gruvbox_light")?;
    app.wait_for("light theme", |state| state["theme"] == "gruvbox_light")?;
    app.click("settings-close")?;
    let state = app.wait_for("preview again", |state| {
        state["settings_page"].is_null() && state["previewing"] == true
    })?;
    let capture = app.window.capture_settled()?;
    pixels::assert_visible_controls(&capture, &state)?;
    snapshots::assert_snapshot("markdown-preview-light-1280", &capture)?;

    app.click("markdown-preview-toggle")?;
    app.wait_for("source again", |state| state["previewing"] == false)?;
    app.click("markdown-preview-toggle")?;
    app.wait_for("preview again", |state| state["previewing"] == true)?;
    app.shutdown()?;
    let saved = app.saved_settings()?;
    ensure!(
        saved["markdown_preview"] == true,
        "the preview choice was not remembered: {saved}"
    );
    Ok(())
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn update_banner_and_settings_section() -> Result<()> {
    rendered::update_flow::banner_and_settings_section()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn update_settings_for_a_source_build() -> Result<()> {
    rendered::update_flow::source_build_has_no_update_controls()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn update_check_in_flight_does_not_hold_up_quitting() -> Result<()> {
    rendered::update_flow::quit_during_a_hanging_check()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn update_install_restarts_into_same_project() -> Result<()> {
    rendered::update_flow::install_restarts_into_same_project()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn update_signed_by_someone_else_is_refused() -> Result<()> {
    rendered::update_flow::wrong_signature_is_refused()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn update_with_a_wrong_checksum_is_refused() -> Result<()> {
    rendered::update_flow::wrong_checksum_is_refused()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn update_waits_for_unsaved_edits_then_restarts() -> Result<()> {
    rendered::update_flow::unsaved_edits_hold_the_restart()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn update_quit_mid_download_leaves_no_file() -> Result<()> {
    rendered::update_flow::quit_mid_download_leaves_nothing()
}

#[test]
#[ignore = "requires an interactive Windows desktop and native GPU rendering"]
fn update_cancel_mid_download_leaves_no_file() -> Result<()> {
    rendered::update_flow::cancel_mid_download_leaves_nothing()
}
