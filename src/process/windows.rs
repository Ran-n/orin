// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:20.600485
//+ Revised: 	2026/11/12 09:16:35.665117
// -----------------------------------------------------------------------------

use std::{
    io, mem,
    os::windows::{io::AsRawHandle, process::CommandExt},
    process::{Child, Command},
    ptr,
};

use anyhow::{Context, Result, ensure};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    },
};

mod resume;

/// The final owner closing this unnamed job also terminates its descendants.
pub(crate) struct ProcessTree {
    job: usize,
}

pub(crate) fn configure(command: &mut Command) {
    use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};
    command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
}

impl ProcessTree {
    pub(crate) fn attach_pty(child: &dyn portable_pty::Child) -> Result<Self> {
        Self::attach_handle(
            child
                .as_raw_handle()
                .context("PTY process handle is unavailable.")?,
        )
    }

    pub(crate) fn attach_child(child: &Child) -> Result<Self> {
        let tree = Self::attach_handle(child.as_raw_handle())?;
        // No adapter code can execute or create descendants until its job is assigned.
        resume::process(child.id())?;
        Ok(tree)
    }

    fn attach_handle(process: HANDLE) -> Result<Self> {
        // SAFETY: null pointers create an unnamed, non-inheritable job with default security.
        let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        ensure!(
            !job.is_null(),
            "Cannot create process job: {}",
            io::Error::last_os_error()
        );
        let tree = Self { job: job as usize };
        tree.configure()?;
        // SAFETY: both handles are live for this call; this job exclusively belongs to Orin.
        ensure!(
            unsafe { AssignProcessToJobObject(job, process) } != 0,
            "Cannot supervise the process tree: {}",
            io::Error::last_os_error()
        );
        Ok(tree)
    }

    fn configure(&self) -> Result<()> {
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the pointer, class, and size describe the same limits struct.
        ensure!(
            unsafe {
                SetInformationJobObject(
                    self.handle(),
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    mem::size_of_val(&limits) as u32,
                )
            } != 0,
            "Cannot configure process cleanup: {}",
            io::Error::last_os_error()
        );
        Ok(())
    }

    pub(crate) fn terminate(&self) -> Result<()> {
        // SAFETY: this job is owned by Orin and never contains the host application.
        ensure!(
            unsafe { TerminateJobObject(self.handle(), 1) } != 0,
            "Cannot stop session processes: {}",
            io::Error::last_os_error()
        );
        Ok(())
    }

    fn handle(&self) -> HANDLE {
        self.job as HANDLE
    }
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        // SAFETY: close exactly once, after the final Arc owning the job is dropped.
        if unsafe { CloseHandle(self.handle()) } == 0 {
            tracing::error!(error = %io::Error::last_os_error(), "failed to close process job");
        }
    }
}
