// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:45:43.835809
//+ Revised: 	2026/11/12 10:31:43.427111
// -----------------------------------------------------------------------------

//! A deterministic offline ACP subprocess. Never invokes a model or vendor CLI.

use serde_json::{Value, json};
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, BufRead, Write},
    path::PathBuf,
    process::Command,
};

#[path = "fixture/behavior.rs"]
mod behavior;

struct Fixture {
    mode: String,
    cwd: PathBuf,
    prompt: Option<Value>,
    authenticated: bool,
    unsupported: u8,
    /// Session options, like an adapter's model, effort, and mode pickers.
    model: String,
    effort: String,
    mode_choice: String,
}

fn main() -> anyhow::Result<()> {
    if env::args().any(|argument| argument == "--child") {
        fs::write(
            env::var_os("ORIN_ACP_FIXTURE_CHILD_PID")
                .ok_or_else(|| anyhow::anyhow!("No child pid path"))?,
            std::process::id().to_string(),
        )?;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    let mut fixture = Fixture {
        mode: env::var("ORIN_ACP_FIXTURE_MODE").unwrap_or_else(|_| "permission".into()),
        cwd: env::current_dir()?,
        prompt: None,
        authenticated: false,
        unsupported: 0,
        model: "fixture-fast".into(),
        effort: "low".into(),
        mode_choice: "ask".into(),
    };
    for line in io::stdin().lock().lines() {
        let line = line?;
        if let Some(path) = env::var_os("ORIN_ACP_FIXTURE_LOG") {
            writeln!(
                OpenOptions::new().create(true).append(true).open(path)?,
                "{line}"
            )?;
        }
        fixture.frame(serde_json::from_str(&line)?)?;
    }
    Ok(())
}

impl Fixture {
    fn frame(&mut self, frame: Value) -> anyhow::Result<()> {
        match frame["method"].as_str() {
            Some("initialize") => self.initialize(frame["id"].clone())?,
            Some("session/new") => self.new_session(frame["id"].clone())?,
            Some("authenticate") => {
                self.authenticated = true;
                reply(frame["id"].clone(), json!({}))?;
            }
            Some("session/prompt") => self.prompt(frame)?,
            Some("session/set_config_option") => self.set_config_option(frame)?,
            Some("session/set_model") => {
                self.model = frame["params"]["modelId"]
                    .as_str()
                    .unwrap_or_default()
                    .into();
                reply(frame["id"].clone(), json!({}))?;
            }
            Some("session/cancel") if self.mode == "slow-cancel" => {
                // Longer than Orin's old 5 s cancel deadline, well inside its grace period.
                std::thread::sleep(std::time::Duration::from_secs(7));
                self.complete("cancelled")?
            }
            Some("session/cancel") if self.mode != "ignore-cancel" => self.complete("cancelled")?,
            None if frame["id"] == "permission-1" => self.permission_response(frame)?,
            None if frame["id"] == "fs-request" || frame["id"] == "terminal-request" => {
                anyhow::ensure!(
                    frame["error"]["code"] == -32601,
                    "Client granted an unadvertised capability"
                );
                self.unsupported += 1;
                if self.unsupported == 2 {
                    self.complete("end_turn")?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn initialize(&mut self, id: Value) -> anyhow::Result<()> {
        if self.mode == "malformed" {
            io::stdout().write_all(b"not json\n")?;
            io::stdout().flush()?;
            return Ok(());
        }
        if self.mode == "child" {
            Command::new(env::current_exe()?)
                .arg("--child")
                .stdin(std::process::Stdio::null())
                .spawn()?;
        }
        reply(
            id,
            json!({"protocolVersion":if self.mode == "wrong-version" { 2 } else { 1 },
            "agentCapabilities":{},"agentInfo":{"name":"orin-fixture","version":"1"},
            "authMethods":[{"id":"fixture-login","name":"Fixture sign-in","description":"Offline fake authentication"}]}),
        )
    }

    fn new_session(&self, id: Value) -> anyhow::Result<()> {
        if self.mode == "auth" && !self.authenticated {
            return emit(
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":"Authentication required"}}),
            );
        }
        if self.mode == "legacy-models" {
            return reply(
                id,
                json!({"sessionId":"fixture-session","models":{"currentModelId":self.model,"availableModels":[
                    {"modelId":"fixture-fast","name":"Fixture Fast"},
                    {"modelId":"fixture-deep","name":"Fixture Deep"}]}}),
            );
        }
        reply(
            id,
            json!({"sessionId":"fixture-session","configOptions":self.config_options()}),
        )
    }

    /// Grouped model choices, an effort level, a mode, and an option of no shown category.
    fn config_options(&self) -> Value {
        json!([
            {"id":"model","name":"Model","category":"model","type":"select","currentValue":self.model,"options":[
                {"group":"fixture","name":"Fixture","options":[
                    {"value":"fixture-fast","name":"Fixture Fast"},
                    {"value":"fixture-deep","name":"Fixture Deep","description":"Slower, more careful"},
                    {"value":"fixture-unavailable","name":"Unavailable model"}]}]},
            {"id":"effort","name":"Effort","category":"thought_level","type":"select","currentValue":self.effort,"options":[
                {"value":"low","name":"Low"},{"value":"high","name":"High"}]},
            {"id":"mode","name":"Mode","category":"mode","type":"select","currentValue":self.mode_choice,"options":[
                {"value":"ask","name":"Ask"},{"value":"code","name":"Code"},{"value":"plan","name":"Plan"}]},
            {"id":"fast","name":"Fast output","type":"boolean","currentValue":false}
        ])
    }

    fn set_config_option(&mut self, frame: Value) -> anyhow::Result<()> {
        let value = frame["params"]["value"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        if value == "fixture-unavailable" {
            return emit(
                json!({"jsonrpc":"2.0","id":frame["id"],"error":{"code":-32602,"message":"That model is not available on this plan"}}),
            );
        }
        match frame["params"]["configId"].as_str() {
            Some("model") => self.model = value,
            Some("effort") => self.effort = value,
            Some("mode") => self.mode_choice = value,
            _ => {}
        }
        reply(
            frame["id"].clone(),
            json!({"configOptions":self.config_options()}),
        )
    }

    fn tool(&self) -> Value {
        let path = self.cwd.join("src/main.rs").to_string_lossy().into_owned();
        json!({"sessionUpdate":"tool_call","toolCallId":"edit-1","title":"Update src/main.rs","kind":"edit","status":"pending",
            "locations":[{"path":path}],"content":[{"type":"diff","path":path,"oldText":"","newText":new_text()}]})
    }

    fn complete(&mut self, reason: &str) -> anyhow::Result<()> {
        if let Some(id) = self.prompt.take() {
            reply(id, json!({"stopReason":reason}))?;
        }
        Ok(())
    }
}

fn new_text() -> &'static str {
    "fn main() {\n    println!(\"Created with Orin\");\n}\n"
}
fn reply(id: Value, result: Value) -> anyhow::Result<()> {
    emit(json!({"jsonrpc":"2.0","id":id,"result":result}))
}
fn update(update: Value) -> anyhow::Result<()> {
    emit(
        json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"fixture-session","update":update}}),
    )
}
fn emit(frame: Value) -> anyhow::Result<()> {
    serde_json::to_writer(io::stdout().lock(), &frame)?;
    io::stdout().write_all(b"\n")?;
    io::stdout().flush()?;
    Ok(())
}
