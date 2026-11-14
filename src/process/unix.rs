// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:20.764819
//+ Revised: 	2026/11/14 12:12:23.626712
// -----------------------------------------------------------------------------

use std::{
    io,
    os::unix::process::CommandExt,
    process::{Child, Command},
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result, ensure};

pub(crate) struct ProcessTree {
    session: libc::pid_t,
    /// Set once the owner has reaped the session leader. Until then the leader's id is held
    /// (by the leader, running or exited); afterwards the system may give it to a stranger.
    leader_reaped: AtomicBool,
}

/// How often the survivors of a reaped leader are listed and stopped, so one that was being
/// forked during a pass is caught by the next.
const SURVIVOR_PASSES: usize = 3;

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
            leader_reaped: AtomicBool::new(false),
        })
    }

    /// The owner has waited for the session leader, so its process id is free to be reused.
    pub(crate) fn leader_reaped(&self) {
        self.leader_reaped.store(true, Ordering::Release);
    }

    pub(crate) fn terminate(&self) -> Result<()> {
        if self.leader_reaped.load(Ordering::Acquire) {
            return self.terminate_survivors();
        }
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

    /// Stop what outlived a reaped leader, without ever signalling by the leader's id: a group
    /// signal to a number the system has handed to someone else would kill a stranger.
    fn terminate_survivors(&self) -> Result<()> {
        for _ in 0..SURVIVOR_PASSES {
            let live = self.live_session_processes()?;
            let Some(members) = survivors(self.session, &live) else {
                tracing::warn!(
                    target: "orin::process",
                    session = self.session,
                    "The session's id was reused by another process; nothing was signalled"
                );
                return Ok(());
            };
            if members.is_empty() {
                return Ok(());
            }
            for &process in members {
                signal(process).context("Cannot stop session process.")?;
            }
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

/// The processes to stop once the leader of `session` has been reaped, out of the `live`
/// processes whose session has that number. `None` when one of them has the leader's own id:
/// the reaped leader can't be running, so the id was reused and that session isn't ours.
/// While any of our processes is alive the system keeps the id reserved, so otherwise every
/// live process there is one of ours.
fn survivors(session: libc::pid_t, live: &[libc::pid_t]) -> Option<&[libc::pid_t]> {
    (!live.contains(&session)).then_some(live)
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
    fn unit_a_reused_leader_id_is_never_signalled() {
        // Our own descendants, left behind by a reaped leader 40: all are stopped.
        assert_eq!(survivors(40, &[41, 42]), Some(&[41, 42][..]));
        assert_eq!(survivors(40, &[]), Some(&[] as &[libc::pid_t]));
        // A live process 40 can't be the reaped leader: the id, and its session, are a stranger's.
        assert_eq!(survivors(40, &[40, 41]), None);
    }

    #[test]
    fn integration_terminate_after_reaping_stops_survivors_without_a_group_signal() {
        // The leader exits at once and leaves a child behind in its session.
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30 &"]);
        configure(&mut command);
        let mut child = command.spawn().unwrap();
        let tree = ProcessTree::attach_child(&child).unwrap();
        child.wait().unwrap();
        tree.leader_reaped();
        assert_eq!(tree.live_session_processes().unwrap().len(), 1);
        tree.terminate().unwrap();
        // The survivor is killed and adopted by init; give its exit a moment to be seen.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !tree.live_session_processes().unwrap().is_empty() {
            assert!(
                std::time::Instant::now() < deadline,
                "the survivor is still running"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        // With nothing left, stopping again signals nobody and succeeds.
        tree.terminate().unwrap();
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
