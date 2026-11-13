// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:20.764819
//+ Revised: 	2026/11/13 14:00:02.809945
// -----------------------------------------------------------------------------

use std::{
    io,
    os::unix::process::CommandExt,
    process::{Child, Command},
};

use anyhow::{Context, Result, ensure};

pub(crate) struct ProcessTree {
    session: libc::pid_t,
}

pub(crate) fn configure(command: &mut Command) {
    // SAFETY: setsid is async-signal-safe; no allocation or locks run between fork and exec.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

impl ProcessTree {
    pub(crate) fn attach_pty(child: &dyn portable_pty::Child) -> Result<Self> {
        Self::from_pid(
            child
                .process_id()
                .context("PTY process id is unavailable.")?,
        )
    }

    pub(crate) fn attach_child(child: &Child) -> Result<Self> {
        Self::from_pid(child.id())
    }

    fn from_pid(process: u32) -> Result<Self> {
        Ok(Self {
            session: process.try_into().context("Process id is out of range.")?,
        })
    }

    pub(crate) fn terminate(&self) -> Result<()> {
        // Listed before the group signal, so a process that left the group is still found.
        let members = self.live_session_processes();
        let group = signal(-self.session);
        let live = members?;
        match group {
            // macOS refuses (EPERM) to signal a group whose members have all exited but not yet
            // been reaped; with no live member there is nothing left to stop.
            Err(error) if error.raw_os_error() == Some(libc::EPERM) && live.is_empty() => {}
            other => other.context("Cannot stop session process.")?,
        }
        for process in live {
            signal(process).context("Cannot stop session process.")?;
        }
        Ok(())
    }

    /// Processes in this session that are still running; exited (zombie) ones are left out.
    fn live_session_processes(&self) -> Result<Vec<libc::pid_t>> {
        let output = Command::new("ps")
            .args(["-axo", "pid=,stat="])
            .output()
            .context("Cannot enumerate session processes.")?;
        ensure!(
            output.status.success(),
            "Cannot enumerate session processes."
        );
        Ok(live_processes(&String::from_utf8_lossy(&output.stdout))
            .into_iter()
            // SAFETY: getsid only queries each process and takes no pointers.
            .filter(|process| unsafe { libc::getsid(*process) } == self.session)
            .collect())
    }
}

/// The ids in `ps -o pid=,stat=` output whose state isn't `Z` (exited, waiting to be reaped).
fn live_processes(listing: &str) -> Vec<libc::pid_t> {
    listing
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let process = fields.next()?.parse::<libc::pid_t>().ok()?;
            let state = fields.next().unwrap_or_default();
            (!state.starts_with('Z')).then_some(process)
        })
        .collect()
}

/// SIGKILL a process, or a group when negative; one that is already gone is not an error.
fn signal(process: libc::pid_t) -> io::Result<()> {
    // SAFETY: the pid or group belongs to this owned process session.
    if unsafe { libc::kill(process, libc::SIGKILL) } == -1 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_live_processes_skip_zombies_and_unparsable_lines() {
        let listing = "    1 Ss\n   42 S+\n   43 Z+\n   44 Z\n   45 R\nnot-a-pid S\n   46\n";
        assert_eq!(live_processes(listing), [1, 42, 45, 46]);
    }

    #[test]
    fn unit_signal_treats_a_missing_process_as_stopped() {
        // pid_t::MAX is never a real process id.
        assert!(signal(libc::pid_t::MAX).is_ok());
    }

    #[test]
    fn integration_terminate_stops_a_running_session_and_tolerates_an_exited_one() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30 & wait"]);
        configure(&mut command);
        let mut child = command.spawn().unwrap();
        let tree = ProcessTree::attach_child(&child).unwrap();
        tree.terminate().unwrap();
        child.wait().unwrap();
        // Everything has exited now; stopping it again must not fail on any platform.
        tree.terminate().unwrap();
    }
}
