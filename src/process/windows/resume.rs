// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:57:42.400444
//+ Revised: 	2026/11/12 15:28:14.746307
// -----------------------------------------------------------------------------

use std::{io, mem};

use anyhow::{Result, bail, ensure};
use windows_sys::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
    },
};

pub(super) fn process(process_id: u32) -> Result<()> {
    // SAFETY: this read-only snapshot enumerates system thread metadata.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    ensure!(
        snapshot != INVALID_HANDLE_VALUE,
        "Cannot inspect suspended adapter: {}",
        io::Error::last_os_error()
    );
    let mut entry = THREADENTRY32 {
        dwSize: mem::size_of::<THREADENTRY32>() as u32,
        ..THREADENTRY32::default()
    };
    // SAFETY: entry size and snapshot are valid for the ToolHelp API.
    let mut available = unsafe { Thread32First(snapshot, &mut entry) } != 0;
    let mut thread = None;
    while available {
        if entry.th32OwnerProcessID == process_id {
            thread = Some(entry.th32ThreadID);
            break;
        }
        // SAFETY: continue enumerating the same owned snapshot and correctly sized entry.
        available = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
    }
    // SAFETY: close the snapshot once; no further enumerator references remain.
    if unsafe { CloseHandle(snapshot) } == 0 {
        tracing::error!(error = %io::Error::last_os_error(), "failed to close adapter thread snapshot");
    }
    let thread =
        thread.ok_or_else(|| anyhow::anyhow!("Suspended adapter has no initial thread."))?;
    resume(thread)
}

fn resume(thread_id: u32) -> Result<()> {
    // SAFETY: query the initial thread belonging to our suspended child only.
    let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
    ensure!(
        !thread.is_null(),
        "Cannot open adapter's initial thread: {}",
        io::Error::last_os_error()
    );
    // SAFETY: the live thread handle has the required access; one suspension belongs to us.
    let previous = unsafe { ResumeThread(thread) };
    let error = io::Error::last_os_error();
    // SAFETY: the only owned thread handle is closed exactly once.
    if unsafe { CloseHandle(thread) } == 0 {
        tracing::error!(error = %io::Error::last_os_error(), "failed to close adapter initial thread handle");
    }
    match previous {
        1 => Ok(()),
        u32::MAX => bail!("Cannot start supervised adapter: {error}"),
        _ => bail!("Adapter's initial thread had an unexpected suspension count."),
    }
}
