// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:52.069077
//+ Revised: 	2026/11/14 01:02:34.346310
// -----------------------------------------------------------------------------

use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result, bail, ensure};
use portable_pty::{ChildKiller, CommandBuilder, PtySize, native_pty_system};

use super::{
    TerminalScreen, TerminalSnapshot,
    process_tree::ProcessTree,
    screen::{SharedScreen, lock, validate_size},
    worker::{Command, Worker, read_output},
};

const QUEUE_CAPACITY: usize = 32;
const MAX_WRITE_BYTES: usize = 65_536;

/// Owns a PTY and its threads. Drop synchronously stops and reaps this process tree.
pub struct Session {
    commands: SyncSender<Command>,
    screen: SharedScreen,
    shutdown: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    tree: Arc<ProcessTree>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    controller: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
    process_id: Option<u32>,
}

impl Session {
    /// Process startup must run on a background thread, away from the UI.
    pub fn spawn_shell(cwd: &Path, rows: u16, cols: u16) -> Result<Self> {
        Self::spawn_shell_in_path(cwd, rows, cols, None)
    }

    pub fn spawn_shell_in_path(
        cwd: &Path,
        rows: u16,
        cols: u16,
        path: Option<&std::ffi::OsStr>,
    ) -> Result<Self> {
        tracing::info!("starting terminal session");
        let mut command = super::shell::shell_command()?;
        if let Some(path) = path {
            command.env("PATH", path);
        }
        Self::spawn_builder(command, cwd, rows, cols)
    }

    pub(super) fn spawn_builder(
        mut command: CommandBuilder,
        cwd: &Path,
        rows: u16,
        cols: u16,
    ) -> Result<Self> {
        validate_size(rows, cols)?;
        ensure!(
            cwd.is_dir(),
            "The session working directory does not exist."
        );
        command.cwd(crate::process::launch::native_path(cwd));
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let screen = Arc::new(Mutex::new(TerminalScreen::new(rows, cols)?));
        let pair = native_pty_system().openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let mut child = pair
            .slave
            .spawn_command(command)
            .context("Cannot launch terminal process.")?;
        drop(pair.slave);
        let tree = match ProcessTree::attach_pty(child.as_ref()) {
            Ok(tree) => Arc::new(tree),
            Err(error) => {
                if let Err(error) = child.kill() {
                    tracing::warn!(%error, "unsupervised terminal could not be stopped");
                }
                if let Err(error) = child.wait() {
                    tracing::error!(%error, "unsupervised terminal could not be reaped");
                }
                return Err(error);
            }
        };
        Self::start_threads(child, pair.master, writer, reader, tree, screen)
    }

    fn start_threads(
        child: Box<dyn portable_pty::Child + Send + Sync>,
        master: Box<dyn portable_pty::MasterPty + Send>,
        writer: Box<dyn std::io::Write + Send>,
        reader: Box<dyn std::io::Read + Send>,
        tree: Arc<ProcessTree>,
        screen: SharedScreen,
    ) -> Result<Self> {
        let (commands, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let shutdown = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let process_id = child.process_id();
        let killer = child.clone_killer();
        // Replies to the shell's terminal queries go through the worker, so the output reader
        // never waits on the input writer (which can block while the shell's output backs up).
        let (replies, reply_receiver) = mpsc::sync_channel(super::worker::REPLY_CAPACITY);
        let worker = Worker {
            child,
            master,
            writer,
            replies: reply_receiver,
            tree: tree.clone(),
            screen: screen.clone(),
            shutdown: shutdown.clone(),
            running: running.clone(),
            commands: receiver,
            reaped: false,
        };
        lock(&screen).mark_running();
        let reader_screen = screen.clone();
        let reader_shutdown = shutdown.clone();
        let reader = thread::Builder::new()
            .name("orin-pty-reader".into())
            .spawn(move || read_output(reader, reader_screen, reader_shutdown, replies))?;
        let controller = match thread::Builder::new()
            .name("orin-pty-control".into())
            .spawn(move || worker.run())
        {
            Ok(controller) => controller,
            Err(error) => {
                if reader.join().is_err() {
                    tracing::error!("terminal reader panicked during startup cleanup");
                }
                return Err(error).context("Cannot start terminal worker.");
            }
        };
        Ok(Self {
            commands,
            screen,
            shutdown,
            running,
            tree,
            killer,
            controller: Some(controller),
            reader: Some(reader),
            process_id,
        })
    }

    /// Enqueue input without blocking; callers can retry a full queue.
    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        ensure!(
            self.try_write(bytes)?,
            "Terminal input is busy; retry when it has caught up."
        );
        Ok(())
    }

    /// False means backpressure: retain these bytes and retry without losing input.
    pub fn try_write(&self, bytes: &[u8]) -> Result<bool> {
        ensure!(
            bytes.len() <= MAX_WRITE_BYTES,
            "Terminal input exceeds {MAX_WRITE_BYTES} bytes; send smaller chunks."
        );
        if bytes.is_empty() {
            return Ok(true);
        }
        self.try_enqueue(Command::Write(bytes.to_vec()))
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        validate_size(rows, cols)?;
        if !self.running.load(Ordering::Acquire) {
            return lock(&self.screen).resize(rows, cols);
        }
        self.enqueue(Command::Resize(rows, cols))
    }

    /// Positive lines show older output; negative lines return towards the live screen.
    pub fn scroll(&self, lines: i32) -> Result<()> {
        lock(&self.screen).scroll(lines);
        Ok(())
    }

    pub fn snapshot(&self) -> TerminalSnapshot {
        lock(&self.screen).snapshot()
    }

    pub fn process_id(&self) -> Option<u32> {
        self.process_id
    }

    fn enqueue(&self, command: Command) -> Result<()> {
        ensure!(
            self.try_enqueue(command)?,
            "Terminal input is busy; retry when it has caught up."
        );
        Ok(())
    }

    fn try_enqueue(&self, command: Command) -> Result<bool> {
        ensure!(
            self.running.load(Ordering::Acquire) && !self.shutdown.load(Ordering::Acquire),
            "This terminal session has ended."
        );
        match self.commands.try_send(command) {
            Ok(()) => Ok(true),
            Err(TrySendError::Full(_)) => Ok(false),
            Err(TrySendError::Disconnected(_)) => bail!("This terminal session has ended."),
        }
    }

    /// Stop/reap on a background thread. The final screen remains available.
    pub fn stop(&mut self) -> Result<()> {
        self.shutdown.store(true, Ordering::Release);
        let termination = if self.running.load(Ordering::Acquire) {
            let result = self.tree.terminate();
            if let Err(error) = self.killer.kill() {
                tracing::debug!(%error, "terminal child may already have exited");
            }
            result
        } else {
            Ok(())
        };
        let join = |worker: Option<JoinHandle<()>>| {
            worker.map(|worker| {
                crate::process::join_within(worker, crate::process::WORKER_JOIN_LIMIT)
            })
        };
        let control = join(self.controller.take());
        let read = join(self.reader.take());
        ensure!(
            control != Some(crate::process::Joined::Panicked),
            "Terminal control worker panicked."
        );
        ensure!(
            read != Some(crate::process::Joined::Panicked),
            "Terminal output worker panicked."
        );
        termination?;
        tracing::info!("terminal session stopped");
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            tracing::error!(%error, "terminal shutdown failed");
        }
    }
}
