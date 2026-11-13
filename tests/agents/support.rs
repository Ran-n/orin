// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:48:13.527293
//+ Revised: 	2026/11/12 15:14:53.148865
// -----------------------------------------------------------------------------

use orin::agents::{AgentSession, AgentSnapshot, AgentStatus, Provider};
use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

pub fn spawn(mode: &str, directory: &std::path::Path) -> AgentSession {
    let mut command = Command::new(env!("CARGO_BIN_EXE_orin-acp-fixture"));
    command.env("ORIN_ACP_FIXTURE_MODE", mode);
    command.env("ORIN_ACP_FIXTURE_LOG", directory.join("wire.jsonl"));
    AgentSession::spawn_command(Provider::Claude, command, directory).unwrap()
}

pub fn wait(session: &AgentSession, condition: impl Fn(&AgentSnapshot) -> bool) -> AgentSnapshot {
    wait_for(session, Duration::from_secs(8), condition)
}

pub fn wait_for(
    session: &AgentSession,
    timeout: Duration,
    condition: impl Fn(&AgentSnapshot) -> bool,
) -> AgentSnapshot {
    let deadline = Instant::now() + timeout;
    loop {
        let snapshot = session.snapshot();
        if condition(&snapshot) {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "ACP state timed out: {snapshot:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

pub fn ready(session: &AgentSession) -> AgentSnapshot {
    wait(session, |state| state.status == AgentStatus::Ready)
}

#[cfg(windows)]
pub fn alive(process: u32) -> bool {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    // SAFETY: query-only handle for the test-owned process; it is closed before returning.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process);
        if handle.is_null() {
            return false;
        }
        let mut status = 0;
        let result = GetExitCodeProcess(handle, &mut status) != 0 && status == 259;
        CloseHandle(handle);
        result
    }
}

#[cfg(unix)]
pub fn alive(process: u32) -> bool {
    Command::new("ps")
        .args(["-p", &process.to_string(), "-o", "stat="])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && !String::from_utf8_lossy(&output.stdout)
                    .trim_start()
                    .starts_with('Z')
        })
}
