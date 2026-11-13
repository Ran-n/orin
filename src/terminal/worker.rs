// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:52.229635
//+ Revised: 	2026/11/12 09:16:42.329466
// -----------------------------------------------------------------------------

use std::{
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError, Sender},
    },
    time::Duration,
};

use anyhow::Result;
use portable_pty::{Child, MasterPty, PtySize};

use super::{
    process_tree::ProcessTree,
    screen::{SharedScreen, lock},
};

pub(super) enum Command {
    Write(Vec<u8>),
    Resize(u16, u16),
}

pub(super) struct Worker {
    pub child: Box<dyn Child + Send + Sync>,
    pub writer: Box<dyn Write + Send>,
    /// Replies to the shell's terminal queries, from the output reader.
    pub replies: Receiver<Vec<u8>>,
    pub master: Box<dyn MasterPty + Send>,
    pub tree: Arc<ProcessTree>,
    pub screen: SharedScreen,
    pub shutdown: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
    pub commands: Receiver<Command>,
    pub reaped: bool,
}

impl Worker {
    pub(super) fn run(mut self) {
        let result = self.control_loop();
        if let Err(error) = result {
            tracing::error!(%error, "terminal worker failed");
            lock(&self.screen).set_error(format!("Terminal failed: {error}"));
        }
        self.cleanup();
    }

    fn control_loop(&mut self) -> Result<()> {
        while !self.shutdown.load(Ordering::Acquire) {
            if let Some(status) = self.child.try_wait()? {
                self.reaped = true;
                lock(&self.screen).mark_exited(status.exit_code());
                tracing::info!(exit_code = status.exit_code(), "terminal process exited");
                break;
            }
            while let Ok(reply) = self.replies.try_recv() {
                self.write(&reply)?;
            }
            match self.commands.recv_timeout(Duration::from_millis(15)) {
                Ok(command) => self.handle(command)?,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        Ok(())
    }

    fn handle(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Write(bytes) => self.write(&bytes)?,
            Command::Resize(rows, cols) => {
                self.master.resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })?;
                lock(&self.screen).resize(rows, cols)?;
            }
        }
        Ok(())
    }

    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }

    fn cleanup(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Err(error) = self.tree.terminate() {
            tracing::error!(%error, "failed to terminate terminal descendants");
            lock(&self.screen).set_error(format!("Could not stop all session processes: {error}"));
        }
        if !self.reaped {
            if let Err(error) = self.child.kill() {
                tracing::debug!(%error, "terminal child may already have exited");
            }
            match self.child.wait() {
                Ok(status) => {
                    lock(&self.screen).mark_exited(status.exit_code());
                    self.reaped = true;
                }
                Err(error) => {
                    lock(&self.screen)
                        .set_error(format!("Could not reap terminal process: {error}"));
                }
            }
        }
        self.running.store(false, Ordering::Release);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if !self.reaped {
            self.cleanup();
        }
    }
}

pub(super) fn read_output(
    mut reader: Box<dyn Read + Send>,
    screen: SharedScreen,
    shutdown: Arc<AtomicBool>,
    replies: Sender<Vec<u8>>,
) {
    let mut buffer = [0; 8_192];
    let mut queries = super::queries::QueryResponder::default();
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(length) => {
                let reply = process_chunk(&screen, &mut queries, &buffer[..length]);
                if !reply.is_empty()
                    && replies.send(reply).is_err()
                    && !shutdown.load(Ordering::Acquire)
                {
                    lock(&screen).set_error("Cannot answer the shell's terminal query.".into());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if expected_eof(&error) || shutdown.load(Ordering::Acquire) => break,
            Err(error) => {
                tracing::error!(%error, "terminal output read failed");
                lock(&screen).set_error(format!("Cannot read terminal output: {error}"));
                break;
            }
        }
    }
}

/// Parse one read into the screen and collect the replies to the queries in it. Each query is
/// answered with the cursor where the output had put it at that query, not at the read's end.
fn process_chunk(
    screen: &SharedScreen,
    queries: &mut super::queries::QueryResponder,
    chunk: &[u8],
) -> Vec<u8> {
    let mut state = lock(screen);
    let mut replies = Vec::new();
    let mut parsed = 0;
    for (index, byte) in chunk.iter().enumerate() {
        if let Some(query) = queries.feed(*byte) {
            state.feed(&chunk[parsed..=index]);
            parsed = index + 1;
            replies.extend(query.reply(state.cursor()));
        }
    }
    state.process(&chunk[parsed..]);
    replies
}

/// Windows reports a closed pipe as `ERROR_BROKEN_PIPE` or `ERROR_NO_DATA`.
#[cfg(windows)]
const PIPE_CLOSED: [i32; 2] = [109, 232];

fn expected_eof(error: &io::Error) -> bool {
    if matches!(
        error.kind(),
        io::ErrorKind::BrokenPipe | io::ErrorKind::UnexpectedEof
    ) {
        return true;
    }
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(libc::EIO)
    }
    #[cfg(windows)]
    {
        error
            .raw_os_error()
            .is_some_and(|code| PIPE_CLOSED.contains(&code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::screen::TerminalScreen;
    use std::sync::Mutex;

    #[test]
    fn regression_each_cursor_query_gets_the_cursor_at_that_point() {
        // A cursor query followed by more output in the same read used to get the cursor
        // from the end of the read.
        let screen: SharedScreen = Arc::new(Mutex::new(TerminalScreen::new(24, 80).unwrap()));
        let mut queries = super::super::queries::QueryResponder::default();
        let reply = process_chunk(&screen, &mut queries, b"abc\x1b[6nlonger tail\x1b[6n");
        assert_eq!(reply, b"\x1b[1;4R\x1b[1;15R");
        assert!(lock(&screen).snapshot().text.contains("abclonger tail"));
    }
}
