// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 15:07:11.563383
//+ Revised: 	2026/11/12 15:08:39.038356
// -----------------------------------------------------------------------------

//! Bounded command output and deadlines, with owned descendants stopped on every path.

use super::{ProcessTree, configure};
use anyhow::{Context, Result, ensure};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Running {
    child: Child,
    tree: Option<ProcessTree>,
    reaped: bool,
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(tree) = &self.tree
            && let Err(error) = tree.terminate()
        {
            tracing::error!(target: "orin::process", %error, "command descendants could not be stopped");
        }
        if !self.reaped {
            if let Err(error) = self.child.kill() {
                tracing::debug!(target: "orin::process", %error, "command may already have exited");
            }
            if let Err(error) = self.child.wait() {
                tracing::error!(target: "orin::process", %error, "command could not be reaped");
            }
        }
    }
}

/// Capture to anonymous temporary files: inherited output handles never keep a reader blocked.
/// `limit` applies to each stream; checked while running and again before allocating output.
pub(crate) fn output(command: &mut Command, timeout: Duration, limit: u64) -> Result<Output> {
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    command
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?);
    configure(command);
    let mut running = Running {
        child: command.spawn()?,
        tree: None,
        reaped: false,
    };
    running.tree = Some(ProcessTree::attach_child(&running.child)?);
    let started = Instant::now();
    let status = loop {
        check_size(&stdout, &stderr, limit)?;
        if let Some(status) = running.child.try_wait()? {
            running.reaped = true;
            break status;
        }
        ensure!(
            started.elapsed() < timeout,
            "The command took too long to finish."
        );
        thread::sleep(Duration::from_millis(20));
    };
    // Stop descendants before inspecting final output, including when the parent exited normally.
    running
        .tree
        .as_ref()
        .context("Command supervision is unavailable.")?
        .terminate()?;
    running.tree = None;
    check_size(&stdout, &stderr, limit)?;
    Ok(Output {
        status,
        stdout: read(&mut stdout, limit)?,
        stderr: read(&mut stderr, limit)?,
    })
}

fn check_size(stdout: &File, stderr: &File, limit: u64) -> Result<()> {
    ensure!(
        stdout.metadata()?.len() <= limit && stderr.metadata()?.len() <= limit,
        "The command produced too much output."
    );
    Ok(())
}

fn read(file: &mut File, limit: u64) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "The command produced too much output."
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(script: &str) -> Command {
        #[cfg(windows)]
        {
            let mut command = Command::new(super::super::launch::shell_program());
            command.args(["/D", "/C", script]);
            command
        }
        #[cfg(unix)]
        {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            command
        }
    }

    #[test]
    fn integration_capture_retains_output_and_exit_status() {
        #[cfg(windows)]
        let script = "echo captured & echo diagnostic 1>&2 & exit 7";
        #[cfg(unix)]
        let script = "echo captured; echo diagnostic 1>&2; exit 7";
        let output = output(&mut shell(script), Duration::from_secs(5), 1024).unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert!(String::from_utf8_lossy(&output.stdout).contains("captured"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("diagnostic"));
    }

    #[test]
    fn integration_capture_rejects_excess_output_and_spawn_failure() {
        assert!(
            output(&mut shell("echo excessive"), Duration::from_secs(5), 2)
                .unwrap_err()
                .to_string()
                .contains("too much output")
        );
        let directory = tempfile::tempdir().unwrap();
        assert!(
            output(
                &mut Command::new(directory.path().join("missing")),
                Duration::from_secs(5),
                1024
            )
            .is_err()
        );
    }

    #[test]
    fn integration_capture_deadline_terminates_the_owned_command() {
        #[cfg(windows)]
        let script = "ping.exe -n 10 127.0.0.1 >nul";
        #[cfg(unix)]
        let script = "sleep 10";
        let started = Instant::now();
        let error = output(&mut shell(script), Duration::from_millis(100), 1024).unwrap_err();
        assert!(error.to_string().contains("too long"));
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
