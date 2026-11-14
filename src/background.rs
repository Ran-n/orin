// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:17:17.757458
//+ Revised: 	2026/11/14 01:02:27.096515
// -----------------------------------------------------------------------------

//! A bounded worker pool for filesystem and process operations.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};
use std::thread::{self, JoinHandle};

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct Workers {
    sender: Option<SyncSender<Job>>,
    handles: Vec<JoinHandle<()>>,
    /// Set when the pool shuts down: jobs still queued are dropped instead of run.
    closing: Arc<AtomicBool>,
}

impl Workers {
    pub fn new(size: usize, capacity: usize) -> std::io::Result<Self> {
        assert!(size > 0 && capacity > 0);
        let (sender, receiver) = mpsc::sync_channel::<Job>(capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        let mut pool = Self {
            sender: Some(sender),
            handles: Vec::new(),
            closing: Arc::new(AtomicBool::new(false)),
        };
        for index in 0..size {
            let receiver = receiver.clone();
            let closing = pool.closing.clone();
            pool.handles.push(thread::Builder::new().name(format!("orin-io-{index}")).spawn(move || loop {
                let job = receiver.lock().unwrap_or_else(|poison| poison.into_inner()).recv();
                let Ok(job) = job else { break };
                // Nothing is left to show a queued job's result once the workspace is closing.
                if closing.load(Ordering::Acquire) {
                    continue;
                }
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).is_err() {
                    tracing::error!(target: "orin::background", "Background operation panicked");
                }
            })?);
        }
        Ok(pool)
    }

    pub fn submit(&self, job: impl FnOnce() + Send + 'static) -> Result<(), &'static str> {
        self.sender
            .as_ref()
            .ok_or("The workspace is closing.")?
            .try_send(Box::new(job))
            .map_err(|_| "The workspace is busy. Try again in a moment.")
    }

    /// Stop taking jobs, drop the queued ones, and wait for the running ones, but only up to
    /// the shared worker limit: a job blocked on a dead network share must not keep Orin from
    /// quitting. A worker left behind ends with the process.
    pub fn shutdown(&mut self) {
        self.closing.store(true, Ordering::Release);
        self.sender.take();
        let deadline = std::time::Instant::now() + crate::process::WORKER_JOIN_LIMIT;
        for handle in self.handles.drain(..) {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if crate::process::join_within(handle, left) == crate::process::Joined::Panicked {
                tracing::error!(target: "orin::background", "Could not join a background worker");
            }
        }
    }
}

impl Drop for Workers {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    #[test]
    fn regression_shutdown_drops_queued_jobs_and_does_not_wait_on_a_blocked_one() {
        // One worker held by a job that outlives the join limit, with more jobs queued behind
        // it: shutdown used to run them all and wait without a deadline.
        let mut workers = Workers::new(1, 4).unwrap();
        let (started, running) = mpsc::channel();
        let (release, held) = mpsc::channel::<()>();
        workers
            .submit(move || {
                started.send(()).unwrap();
                let _ = held.recv_timeout(Duration::from_secs(60));
            })
            .unwrap();
        running.recv_timeout(Duration::from_secs(10)).unwrap();
        let ran = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let ran = ran.clone();
            workers
                .submit(move || {
                    ran.fetch_add(1, Ordering::SeqCst);
                })
                .unwrap();
        }
        let begun = Instant::now();
        workers.shutdown();
        assert!(
            begun.elapsed() < crate::process::WORKER_JOIN_LIMIT + Duration::from_secs(5),
            "shutdown waited on the blocked job"
        );
        assert!(
            workers.submit(|| {}).is_err(),
            "a closed pool takes no jobs"
        );
        // Let the held job finish: the worker then meets the queued jobs and drops them.
        drop(release);
        thread::sleep(Duration::from_millis(200));
        assert_eq!(
            ran.load(Ordering::SeqCst),
            0,
            "queued jobs ran after shutdown"
        );
    }
}
