// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:23.056525
//+ Revised: 	2026/11/12 09:16:19.271016
// -----------------------------------------------------------------------------

use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{ChildStderr, ChildStdin, ChildStdout},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};

use anyhow::{Result, ensure};
use serde_json::Value;

use super::{
    engine::Event,
    protocol::{self, MAX_FRAME_BYTES},
};

pub(super) fn send(sender: &SyncSender<Event>, shutdown: &AtomicBool, mut event: Event) {
    while !shutdown.load(Ordering::Acquire) {
        match sender.try_send(event) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => return,
            Err(TrySendError::Full(pending)) => {
                event = pending;
                thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

pub(super) fn read_stdout(
    stdout: ChildStdout,
    sender: SyncSender<Event>,
    shutdown: Arc<AtomicBool>,
) {
    let mut reader = BufReader::new(stdout);
    while !shutdown.load(Ordering::Acquire) {
        let result = read_frame(&mut reader);
        match result {
            Ok(Some(frame)) => send(&sender, &shutdown, Event::Frame(frame)),
            Ok(None) => {
                send(&sender, &shutdown, Event::Eof);
                return;
            }
            Err(error) => {
                send(
                    &sender,
                    &shutdown,
                    Event::TransportError(format!("Invalid ACP output: {error}")),
                );
                return;
            }
        }
    }
}

pub(super) fn read_frame(reader: &mut impl BufRead) -> Result<Option<Value>> {
    let mut bytes = Vec::new();
    let length = reader
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if length == 0 {
        return Ok(None);
    }
    ensure!(
        length <= MAX_FRAME_BYTES,
        "ACP message exceeds the 4 MiB limit."
    );
    ensure!(
        bytes.last() == Some(&b'\n'),
        "Agent closed stdout in the middle of an ACP message."
    );
    protocol::decode(&bytes).map(Some)
}

pub(super) fn write_stdin(
    mut stdin: ChildStdin,
    receiver: Receiver<Value>,
    sender: SyncSender<Event>,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::Acquire) {
        let frame = match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(frame) => frame,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        };
        if let Err(error) = write_frame(&mut stdin, &frame) {
            send(
                &sender,
                &shutdown,
                Event::TransportError(format!("Cannot write to the ACP adapter: {error}")),
            );
            return;
        }
    }
}

fn write_frame(writer: &mut impl Write, frame: &Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, frame)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

pub(super) fn drain_stderr(mut stderr: ChildStderr, shutdown: Arc<AtomicBool>) {
    let mut buffer = [0; 4_096];
    // Adapter diagnostics may contain credentials: only their size is ever logged, so a
    // failed start still shows that the adapter complained.
    let mut written = 0_u64;
    while !shutdown.load(Ordering::Acquire) {
        match stderr.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => written += count as u64,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                tracing::debug!(%error, "ACP diagnostics stream closed");
                break;
            }
        }
    }
    if written > 0 {
        tracing::info!(target: "orin::agents", bytes = written, "ACP adapter wrote diagnostics (not logged)");
    }
}
