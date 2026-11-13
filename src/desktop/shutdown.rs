// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:28:15.286785
//+ Revised: 	2026/11/13 07:53:50.839371
// -----------------------------------------------------------------------------

//! One cleanup path for window close, shortcuts, and the native test driver.

use super::workspace::Workspace;
use gpui::{AppContext, Context, Window};

impl Workspace {
    pub fn begin_shutdown(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if !self.discard_on_quit && self.editor_busy(cx) {
            self.error = Some(
                "This file has unsaved edits. Save it before closing, or choose Discard & close."
                    .into(),
            );
            cx.notify();
            return;
        }
        self.closing = true;
        // A download in progress stops between chunks; its temporary file is deleted. Its thread
        // is joined below, so Orin never exits while it is replacing itself.
        self.update
            .cancel
            .store(true, std::sync::atomic::Ordering::Release);
        let download = self.update.download_thread.take();
        let agents = self
            .agents
            .iter()
            .map(|tab| tab.session.clone())
            .collect::<Vec<_>>();
        let shell = self.shell.as_ref().map(|tab| tab.session.clone());
        let workers = self.workers.take();
        let watch = self.watch.take();
        let receiver = std::mem::replace(&mut self.receiver, std::sync::mpsc::channel().1);
        let store = self.store.clone();
        let settings = self.settings.clone();
        let writable = self.writable;
        let relaunch = self.update.relaunch_after_quit.clone().map(|executable| {
            crate::update::install::Relaunch {
                executable,
                project: self
                    .project
                    .as_ref()
                    .map(|project| project.root().to_path_buf()),
                settings: self.settings_path.clone(),
                test_driver: self.update.test_driver,
            }
        });
        let cleanup = cx.background_spawn(async move {
            if let Some(download) = download
                && crate::process::join_within(download, crate::process::WORKER_JOIN_LIMIT)
                    == crate::process::Joined::Panicked
            {
                tracing::error!(target: "orin::update", "Update download thread panicked");
            }
            if let Some(mut workers) = workers {
                workers.shutdown();
            }
            drop(watch);
            for session in agents {
                if let Err(error) = session
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .stop()
                {
                    tracing::error!(%error, "Session shutdown failed");
                }
            }
            if let Some(session) = shell
                && let Err(error) = session
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .stop()
            {
                tracing::error!(%error,"Shell shutdown failed");
            }
            drop(receiver);
            if writable && let Err(error) = store.save(&settings) {
                tracing::error!(%error, "Final workspace save failed");
            }
            // After the save, so the new version reads the latest state.
            if let Some(relaunch) = relaunch {
                match crate::update::install::relaunch(&relaunch) {
                    Ok(()) => tracing::info!(target: "orin::update", "New version started"),
                    // The window is gone; the new version runs the next time Orin is opened.
                    Err(error) => {
                        tracing::error!(target: "orin::update", %error, "New version could not be started; it runs at the next start");
                    }
                }
            }
        });
        cx.spawn(async move |_, cx| {
            cleanup.await;
            if let Err(error) = cx.update(|cx| cx.quit()) {
                tracing::debug!(target: "orin::desktop", %error, "Application already exited during shutdown");
            }
        })
        .detach();
        cx.notify();
    }
}
