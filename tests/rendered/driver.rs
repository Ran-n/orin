// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:22.631944
//+ Revised: 	2026/11/13 07:53:54.003448
// -----------------------------------------------------------------------------

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, ChildStdin},
    sync::mpsc::Receiver,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::window::AppWindow;
use super::{
    fixture::{self, LaunchOptions},
    protocol::response_reader,
};

pub struct AppDriver {
    child: Child,
    input: ChildStdin,
    responses: Receiver<Value>,
    sequence: u64,
    scratch: TempDir,
    pub window: AppWindow,
}

impl AppDriver {
    pub fn fixture_file(&self, relative: &str) -> PathBuf {
        self.scratch.path().join("fixture").join(relative)
    }

    pub fn start(open_project: bool) -> Result<Self> {
        Self::start_with(LaunchOptions {
            open_project,
            ..LaunchOptions::default()
        })
    }

    /// No project, with two remembered folders: the fixture project and one since deleted.
    pub fn start_with_recent_projects() -> Result<Self> {
        Self::start_isolated(LaunchOptions::default(), true, tempfile::tempdir()?)
    }

    #[cfg(feature = "agent-fixture")]
    pub fn start_with_agent() -> Result<Self> {
        Self::start_with(LaunchOptions {
            open_project: true,
            agent_program: Some(PathBuf::from(env!("CARGO_BIN_EXE_orin-acp-fixture"))),
            ..LaunchOptions::default()
        })
    }

    pub fn start_with(options: LaunchOptions) -> Result<Self> {
        Self::start_isolated(options, false, tempfile::tempdir()?)
    }

    /// Start in a scratch folder the caller prepared (for example with a copy of Orin in it).
    pub fn start_in(scratch: TempDir, options: LaunchOptions) -> Result<Self> {
        Self::start_isolated(options, false, scratch)
    }

    pub fn child_id(&self) -> u32 {
        self.child.id()
    }

    pub fn scratch(&self) -> &Path {
        self.scratch.path()
    }

    /// Wait for the app to exit on its own (after an update restart, for example).
    pub fn wait_exit(&mut self, within: Duration) -> Result<std::process::ExitStatus> {
        let deadline = Instant::now() + within;
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                bail!("native app did not exit: {}", self.logs());
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn start_isolated(
        options: LaunchOptions,
        recent_projects: bool,
        scratch: TempDir,
    ) -> Result<Self> {
        if recent_projects {
            let recent = [scratch.path().join("fixture"), scratch.path().join("gone")];
            fs::write(
                scratch.path().join("settings.json"),
                serde_json::to_string(&json!({"schema_version": 2, "recent_projects": recent}))?,
            )?;
        }
        let mut child = fixture::launch(scratch.path(), &options)?;
        let input = child.stdin.take().context("driver stdin was not piped")?;
        let responses =
            response_reader(child.stdout.take().context("driver stdout was not piped")?);
        let window = match AppWindow::wait(child.id()) {
            Ok(window) => window,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let mut driver = Self {
            child,
            input,
            responses,
            sequence: 0,
            scratch,
            window,
        };
        driver.wait_for("rendered workspace", |state| {
            state["controls"]
                .as_array()
                .is_some_and(|controls| !controls.is_empty())
        })?;
        Ok(driver)
    }

    pub fn request(&mut self, mut command: Value) -> Result<Value> {
        self.sequence += 1;
        command["id"] = json!(self.sequence);
        writeln!(self.input, "{command}")?;
        self.input.flush()?;
        let response = self
            .responses
            .recv_timeout(Duration::from_secs(15))
            .with_context(|| self.logs())?;
        ensure!(
            response["id"] == self.sequence,
            "driver response ID mismatch: {response}"
        );
        ensure!(
            response["ok"] == true,
            "driver rejected command: {response}"
        );
        Ok(response)
    }

    pub fn state(&mut self) -> Result<Value> {
        Ok(self.request(json!({"command": "state"}))?["state"].take())
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.request(json!({"command": "resize", "width": width, "height": height}))?;
        thread::sleep(Duration::from_millis(300));
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.request(json!({"command": "quit"}))?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.child.try_wait()? {
                ensure!(
                    status.success(),
                    "native app exited with {status}: {}",
                    self.logs()
                );
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("native app did not shut down cleanly: {}", self.logs());
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// The centre of an enabled control, in logical window coordinates.
    fn control_center(&mut self, id: &str) -> Result<(f64, f64)> {
        let state = self.state()?;
        let control = state["controls"]
            .as_array()
            .context("state.controls must be an array")?
            .iter()
            .find(|control| control["id"] == id)
            .with_context(|| format!("control {id:?} absent: {state}"))?;
        ensure!(control["enabled"] == true, "control {id:?} is disabled");
        let coordinate = |key: &str| {
            control[key]
                .as_f64()
                .with_context(|| format!("missing control {key}"))
        };
        Ok((
            coordinate("x")? + coordinate("width")? / 2.0,
            coordinate("y")? + coordinate("height")? / 2.0,
        ))
    }

    /// Wait until the saved settings file satisfies `predicate` (saves can trail the state).
    pub fn wait_for_saved(
        &self,
        description: &str,
        predicate: impl Fn(&Value) -> bool,
    ) -> Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            // The file may not exist yet; a read error is kept for the timeout message.
            let (saved, problem) = match self.saved_settings() {
                Ok(saved) => (saved, None),
                Err(error) => (Value::Null, Some(error)),
            };
            if predicate(&saved) {
                return Ok(saved);
            }
            if Instant::now() > deadline {
                match problem {
                    Some(error) => bail!("timed out waiting for saved {description}: {error:#}"),
                    None => bail!("timed out waiting for saved {description}: {saved}"),
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn click(&mut self, id: &str) -> Result<()> {
        let (x, y) = self.control_center(id)?;
        self.window.click(x, y)?;
        thread::sleep(Duration::from_millis(120));
        Ok(())
    }

    /// Drag a control (such as a column divider) horizontally by `dx` logical pixels.
    pub fn drag(&mut self, id: &str, dx: f64) -> Result<()> {
        let (x, y) = self.control_center(id)?;
        self.window.drag((x, y), (x + dx, y))?;
        thread::sleep(Duration::from_millis(200));
        Ok(())
    }

    /// The settings file the app wrote to its isolated scratch folder.
    pub fn saved_settings(&self) -> Result<Value> {
        let path = self.scratch.path().join("settings.json");
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn wait_for(
        &mut self,
        description: &str,
        predicate: impl Fn(&Value) -> bool,
    ) -> Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let state = self.state()?;
            if predicate(&state) {
                return Ok(state);
            }
            if Instant::now() > deadline {
                bail!(
                    "timed out waiting for {description}: {state}\n{}",
                    self.logs()
                );
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn logs(&self) -> String {
        fs::read_to_string(self.scratch.path().join("app.log"))
            .unwrap_or_else(|error| format!("app log unavailable: {error}"))
    }
}

impl Drop for AppDriver {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_some() {
            return;
        }
        let _ = writeln!(
            self.input,
            "{}",
            json!({"id": self.sequence + 1, "command": "quit"})
        );
        let _ = self.input.flush();
        self.window.close();
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
