// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:17:17.757458
//+ Revised: 	2026/11/12 09:16:19.976066
// -----------------------------------------------------------------------------

//! A bounded worker pool for filesystem and process operations.

use std::sync::{
    Arc, Mutex,
    mpsc::{self, SyncSender},
};
use std::thread::{self, JoinHandle};

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct Workers {
    sender: Option<SyncSender<Job>>,
    handles: Vec<JoinHandle<()>>,
}

impl Workers {
    pub fn new(size: usize, capacity: usize) -> std::io::Result<Self> {
        assert!(size > 0 && capacity > 0);
        let (sender, receiver) = mpsc::sync_channel::<Job>(capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        let mut pool = Self {
            sender: Some(sender),
            handles: Vec::new(),
        };
        for index in 0..size {
            let receiver = receiver.clone();
            pool.handles.push(thread::Builder::new().name(format!("orin-io-{index}")).spawn(move || loop {
                let job = receiver.lock().unwrap_or_else(|poison| poison.into_inner()).recv();
                let Ok(job) = job else { break };
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

    pub fn shutdown(&mut self) {
        self.sender.take();
        for handle in self.handles.drain(..) {
            if handle.join().is_err() {
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
