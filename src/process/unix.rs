// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:20.764819
//+ Revised: 	2026/11/12 09:16:35.326383
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
        let descendants = self.session_processes();
        signal(-self.session)?;
        for process in descendants? {
            signal(process)?;
        }
        Ok(())
    }

    fn session_processes(&self) -> Result<Vec<libc::pid_t>> {
        let output = Command::new("ps")
            .args(["-axo", "pid="])
            .output()
            .context("Cannot enumerate session processes.")?;
        ensure!(
            output.status.success(),
            "Cannot enumerate session processes."
        );
        Ok(String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .filter_map(|value| value.parse::<libc::pid_t>().ok())
            // SAFETY: getsid only queries each process and takes no pointers.
            .filter(|process| unsafe { libc::getsid(*process) } == self.session)
            .collect())
    }
}

fn signal(process: libc::pid_t) -> Result<()> {
    // SAFETY: the pid or group belongs to this owned process session.
    if unsafe { libc::kill(process, libc::SIGKILL) } == -1 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error).context("Cannot stop session process.");
        }
    }
    Ok(())
}
