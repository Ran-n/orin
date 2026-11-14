// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:45:42.007815
//+ Revised: 	2026/11/14 22:00:10.813954
// -----------------------------------------------------------------------------

use std::fs;

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::{driver::AppDriver, pixels, snapshots};

pub fn run() -> Result<()> {
    for (width, height) in [(1280, 800), (1000, 680)] {
        let mut app = AppDriver::start_with_agent()?;
        app.resize(width, height)?;
        // The empty column's own launch button starts the agent, like the one in the header.
        let state = app.wait_for("agent launchers", |state| has(state, "start-claude"))?;
        ensure!(
            enabled(&state, "start-claude") == Some(true),
            "the installed agent's launch button is disabled"
        );
        ensure!(
            !has(&state, "agent-send") && !has(&state, "agent-cancel"),
            "there is no message box before a session starts"
        );
        app.click("start-claude")?;
        let state = app.wait_for("offline ACP fixture ready", |state| {
            session(state).is_some_and(|agent| agent["status"] == "Ready")
                && has(state, "agent-send")
        })?;
        ensure!(
            !has(&state, "start-claude"),
            "the launchers leave once a session is open"
        );
        // An idle agent offers Send (disabled until there is text) and nothing to stop.
        ensure!(
            enabled(&state, "agent-send") == Some(false) && !has(&state, "agent-cancel"),
            "an idle agent shows a disabled Send and no Stop"
        );
        let original = fs::read_to_string(app.fixture_file("src/main.rs"))?;
        prompt(&mut app, "Please update the main program.")?;
        wait_permission(&mut app)?;
        ensure!(
            fs::read_to_string(app.fixture_file("src/main.rs"))? == original,
            "agent wrote before explicit approval"
        );
        // While the agent waits, the one action beside the message box is Stop. Controls are
        // reported from the last painted frame, which can trail the session status.
        let state = app.wait_for("a busy agent shows Stop in place of Send", |state| {
            enabled(state, "agent-cancel") == Some(true) && !has(state, "agent-send")
        })?;
        capture(&mut app, &state, &format!("agent-permission-{width}"))?;
        approve(&mut app)?;
        let state = app.wait_for("completed ACP fixture turn", |state| {
            session(state).is_some_and(|agent| {
                agent["status"] == "Ready"
                    && agent["messages"]
                        .to_string()
                        .contains("Updated src/main.rs.")
            })
        })?;
        capture(&mut app, &state, &format!("agent-complete-{width}"))?;
        let changed = fs::read_to_string(app.fixture_file("src/main.rs"))?;
        ensure!(
            changed.contains("Created with Orin"),
            "approved agent did not write the fixture file"
        );
        review_changed_file(&mut app)?;
        cancel_turn(&mut app, &changed)?;
        report(&mut app, width)?;
        app.shutdown()?;
    }
    Ok(())
}

fn session(state: &Value) -> Option<&Value> {
    state["agent_sessions"].as_array()?.first()
}

fn prompt(app: &mut AppDriver, text: &str) -> Result<()> {
    app.click("agent-prompt")?;
    app.window.type_text(text)?;
    // Typed characters arrive as window messages, apart from driver requests: wait until the
    // app has taken them in and enabled Send.
    app.wait_for("typed prompt", |state| {
        state["controls"].as_array().is_some_and(|controls| {
            controls
                .iter()
                .any(|control| control["id"] == "agent-send" && control["enabled"] == true)
        })
    })?;
    app.click("agent-send")
}

fn wait_permission(app: &mut AppDriver) -> Result<Value> {
    app.wait_for("streamed ACP permission request", |state| {
        session(state).is_some_and(|agent| {
            agent["status"] == "WaitingPermission"
                && agent["pending_permissions"]
                    .as_array()
                    .is_some_and(|requests| !requests.is_empty())
                && agent["messages"]
                    .to_string()
                    .contains("I can update src/main.rs.")
        })
    })
}

fn approve(app: &mut AppDriver) -> Result<()> {
    let state = app.state()?;
    let control = state["controls"]
        .as_array()
        .context("missing permission controls")?
        .iter()
        .filter_map(|control| control["id"].as_str())
        .find(|id| id.contains("permission") && id.ends_with("allow-once"))
        .context("the actual permission approval button was not rendered")?
        .to_owned();
    app.click(&control)
}

fn review_changed_file(app: &mut AppDriver) -> Result<()> {
    let state = app.state()?;
    let control = state["controls"]
        .as_array()
        .context("missing changed-file controls")?
        .iter()
        .filter_map(|control| control["id"].as_str())
        .find(|id| {
            id.starts_with("changed-file-") && id.replace('\\', "/").ends_with("src/main.rs")
        })
        .context("the actual changed-file review link was not rendered")?
        .to_owned();
    app.click(&control)?;
    app.wait_for("reviewed agent change in editor", |state| {
        state["editor_text"]
            .as_str()
            .is_some_and(|text| text.contains("Created with Orin"))
    })?;
    Ok(())
}

fn cancel_turn(app: &mut AppDriver, previous: &str) -> Result<()> {
    prompt(app, "Make another change, but wait for my permission.")?;
    wait_permission(app)?;
    app.click("agent-cancel")?;
    let state = app.wait_for("cancelled ACP turn", |state| {
        session(state)
            .is_some_and(|agent| agent["status"] == "Ready" && agent["stop_reason"] == "cancelled")
    })?;
    ensure!(
        session(&state).context("missing ACP session")?["pending_permissions"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "cancellation left an actionable permission"
    );
    ensure!(
        fs::read_to_string(app.fixture_file("src/main.rs"))? == previous,
        "cancelled agent turn changed a file"
    );
    Ok(())
}

/// A turn with a plan, a tool call between two parts of the reply, and a Markdown reply.
fn report(app: &mut AppDriver, width: u32) -> Result<()> {
    prompt(app, "Report your progress.")?;
    let state = app.wait_for("progress report", |state| {
        session(state).is_some_and(|agent| {
            agent["status"] == "Ready"
                && agent["stop_reason"] == "end_turn"
                && agent["plan"].as_array().is_some_and(|plan| plan.len() == 3)
                && agent["messages"].to_string().contains("## Summary")
        }) && has(state, "agent-send")
    })?;
    let agent = session(&state).context("missing ACP session")?;
    ensure!(
        agent["error"].is_null(),
        "a malformed update ended the turn: {}",
        agent["error"]
    );
    ensure!(
        agent["plan"][0]["status"] == "completed" && agent["plan"][1]["status"] == "in_progress",
        "the latest plan replaces the first: {}",
        agent["plan"]
    );
    capture(app, &state, &format!("agent-report-{width}"))
}

fn enabled(state: &Value, id: &str) -> Option<bool> {
    state["controls"]
        .as_array()?
        .iter()
        .find(|control| control["id"] == id)?["enabled"]
        .as_bool()
}

fn capture(app: &mut AppDriver, state: &Value, name: &str) -> Result<()> {
    let capture = app.window.capture_settled()?;
    pixels::assert_real_pixels(&capture)?;
    pixels::assert_visible_controls(&capture, state)?;
    snapshots::assert_snapshot(name, &capture)
}

fn current(agent: &Value, option: &str) -> Option<String> {
    agent["config_options"]
        .as_array()?
        .iter()
        .find(|entry| entry["id"] == option)?["current"]
        .as_str()
        .map(str::to_owned)
}

/// Pick a model from the session's picker, see it remembered, and see a new session start with it.
pub fn model_picker() -> Result<()> {
    let mut app = AppDriver::start_with_agent()?;
    app.resize(1280, 800)?;
    app.click("agent-claude")?;
    let state = app.wait_for("session options", |state| {
        session(state).is_some_and(|agent| agent["status"] == "Ready")
            && has(state, "agent-option-chip-model")
    })?;
    for chip in ["agent-option-chip-effort", "agent-option-chip-mode"] {
        ensure!(has(&state, chip), "{chip} is absent");
    }
    ensure!(
        !has(&state, "agent-option-chip-fast"),
        "only model, effort, and mode are shown"
    );
    app.click("agent-option-chip-model")?;
    let state = app.wait_for("model menu", |state| {
        has(state, "agent-option-model-fixture-deep")
    })?;
    capture(&mut app, &state, "agent-model-menu-1280")?;

    // A model the agent refuses is reported and not remembered.
    app.click("agent-option-model-fixture-unavailable")?;
    app.wait_for("refused model", |state| {
        session(state).is_some_and(|agent| {
            agent["error"]
                .as_str()
                .is_some_and(|error| error.contains("not available"))
        })
    })?;
    ensure!(
        app.saved_settings()?["agent_options"]["claude"]["model"].is_null(),
        "a refused model must not be remembered"
    );

    app.click("agent-option-chip-model")?;
    app.wait_for("model menu again", |state| {
        has(state, "agent-option-model-fixture-deep")
    })?;
    app.click("agent-option-model-fixture-deep")?;
    let state = app.wait_for("model changed", |state| {
        session(state).is_some_and(|agent| {
            current(agent, "model").as_deref() == Some("fixture-deep") && agent["error"].is_null()
        }) && !has(state, "agent-option-model-fixture-deep")
    })?;
    capture(&mut app, &state, "agent-model-chosen-1280")?;
    app.wait_for_saved("remembered model", |saved| {
        saved["agent_options"]["claude"]["model"] == "fixture-deep"
    })?;

    // A new session of the same agent starts with the remembered model.
    app.click("agent-claude")?;
    app.wait_for("second session uses the remembered model", |state| {
        state["agent_sessions"]
            .as_array()
            .and_then(|sessions| sessions.get(1))
            .is_some_and(|agent| current(agent, "model").as_deref() == Some("fixture-deep"))
    })?;
    app.shutdown()
}

fn has(state: &Value, id: &str) -> bool {
    state["controls"]
        .as_array()
        .is_some_and(|controls| controls.iter().any(|control| control["id"] == id))
}
