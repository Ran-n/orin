// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:35:20.421592
//+ Revised: 	2026/11/12 15:07:12.382982
// -----------------------------------------------------------------------------

//! Process ownership shared by the shell PTY and structured agent transports.

pub(crate) mod capture;
pub(crate) mod launch;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(crate) use unix::{ProcessTree, configure};
#[cfg(windows)]
pub(crate) use windows::{ProcessTree, configure};

/// How long `stop()` waits for a session's reader and control threads. A descendant that left
/// the process tree can keep a pipe open forever; quitting must not wait on it.
pub(crate) const WORKER_JOIN_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);

/// How a bounded join ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Joined {
    Finished,
    Panicked,
    /// Still running after the limit; the thread is left to end with the process.
    Abandoned,
}

/// Join `worker`, giving up after `limit` so a blocked read can't hang shutdown.
pub(crate) fn join_within(
    worker: std::thread::JoinHandle<()>,
    limit: std::time::Duration,
) -> Joined {
    let deadline = std::time::Instant::now() + limit;
    while !worker.is_finished() {
        if std::time::Instant::now() >= deadline {
            let name = worker.thread().name().unwrap_or("unnamed").to_owned();
            tracing::warn!(thread = %name, "Worker still blocked after stop; leaving it to exit with Orin");
            return Joined::Abandoned;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if worker.join().is_err() {
        Joined::Panicked
    } else {
        Joined::Finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn unit_bounded_join_reports_finished_panicked_and_abandoned() {
        let finished = std::thread::spawn(|| {});
        assert_eq!(
            join_within(finished, Duration::from_secs(5)),
            Joined::Finished
        );
        let panicked = std::thread::spawn(|| panic!("worker failure under test"));
        assert_eq!(
            join_within(panicked, Duration::from_secs(5)),
            Joined::Panicked
        );
        let (_keep, wait) = std::sync::mpsc::channel::<()>();
        let blocked = std::thread::spawn(move || {
            let _ = wait.recv_timeout(Duration::from_secs(30));
        });
        let started = std::time::Instant::now();
        assert_eq!(
            join_within(blocked, Duration::from_millis(100)),
            Joined::Abandoned
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
