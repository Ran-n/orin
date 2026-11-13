// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:37.141130
//+ Revised: 	2026/11/13 07:53:50.776324
// -----------------------------------------------------------------------------

//! Update checks and installs, wired to the window: timers, worker threads, banner state.
//!
//! Network work runs on dedicated `orin-update*` threads, never on the UI thread or the shared
//! worker pool, because a slow request must not hold one of the pool's few slots. A check still
//! running at quit is abandoned (it only reads); a download is cancelled, its file deleted, and
//! quitting waits a few seconds for its thread, so Orin never exits halfway through replacing
//! itself.

use super::{events::Event, workspace::Workspace};
use crate::update::{
    check::{UpdateStatus, check},
    download::download,
    install::{download_dir, install},
    release::UpdateError,
    source::{Timeouts, sources},
    target::{current_version, release_target},
};
use gpui::{Context, Window};
use semver::Version;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// The first automatic check waits a minute, so it doesn't compete with opening the project.
const FIRST_CHECK: Duration = Duration::from_secs(60);
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// After a failed automatic check: try again later without nagging.
const RETRY_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// A download in progress, for the banner's percentage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Downloading {
    pub version: Version,
    /// The release page, for the error message; a check finishing mid-download may replace
    /// the offer it came from.
    pub page: String,
    pub read: u64,
    pub total: Option<u64>,
}

/// Everything the window knows about updates.
#[derive(Default)]
pub struct UpdateState {
    /// The last check's result, if Orin checked since it started.
    pub status: Option<UpdateStatus>,
    /// When that result was found, in seconds since the Unix epoch.
    pub checked_at: Option<u64>,
    pub checking: bool,
    /// The banner was closed with ✕; it stays hidden until the next start.
    pub dismissed: bool,
    pub downloading: Option<Downloading>,
    /// The version now on disk and the file to start it from.
    pub installed: Option<(Version, PathBuf)>,
    /// Set on quit or Cancel so the download stops between chunks; a new flag per download.
    pub cancel: Arc<AtomicBool>,
    /// The download thread, joined (briefly) at quit.
    pub download_thread: Option<JoinHandle<()>>,
    /// Start the new version once shutdown has saved the settings.
    pub relaunch_after_quit: Option<PathBuf>,
    /// Bumped whenever the schedule changes, so stale timers do nothing.
    timer_generation: u64,
    /// Under the test driver automatic checks are always off, so tests never touch the network.
    pub test_driver: bool,
}

impl UpdateState {
    pub fn new(test_driver: bool) -> Self {
        Self {
            test_driver,
            ..Self::default()
        }
    }

    /// The banner shows an offered release, a download, or an installed update.
    pub fn banner_visible(&self) -> bool {
        self.installed.is_some()
            || self.downloading.is_some()
            || (!self.dismissed
                && self
                    .status
                    .as_ref()
                    .and_then(UpdateStatus::offered)
                    .is_some())
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// `14:02` in the local time zone.
pub fn clock(at: u64) -> String {
    i64::try_from(at)
        .ok()
        .and_then(|seconds| jiff::Timestamp::from_second(seconds).ok())
        .map(|time| {
            time.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "--:--".into())
}

/// How long until the next automatic check, given the last successful one. A last check in the
/// future (a clock set back, or an edited settings file) waits at most one interval.
pub fn next_check_delay(last_check: Option<u64>, now: u64) -> Duration {
    let due = last_check.map_or(Duration::ZERO, |last| {
        Duration::from_secs(
            last.saturating_add(CHECK_INTERVAL.as_secs())
                .saturating_sub(now)
                .min(CHECK_INTERVAL.as_secs()),
        )
    });
    due.max(FIRST_CHECK)
}

impl Workspace {
    fn automatic_checks(&self) -> bool {
        self.settings.updates.check_automatically
            && !self.update.test_driver
            && release_target().is_some()
    }

    /// Arm the next automatic check `delay` from now, replacing any earlier timer.
    pub fn schedule_update_check(&mut self, delay: Duration, cx: &mut Context<Self>) {
        self.update.timer_generation += 1;
        if !self.automatic_checks() {
            return;
        }
        let generation = self.update.timer_generation;
        tracing::debug!(target: "orin::update", seconds = delay.as_secs(), "next update check scheduled");
        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(delay).await;
            let result = view.update(cx, |view, cx| {
                if view.update.timer_generation == generation
                    && view.automatic_checks()
                    && !view.closing
                {
                    view.check_for_updates(false, cx);
                }
            });
            if result.is_err() {
                tracing::debug!(target: "orin::update", "window closed before the update check");
            }
        })
        .detach();
    }

    /// Ask the release sources on the `orin-update` thread; `manual` when the user pressed
    /// Check now, which always checks and reports failures in the error banner.
    pub fn check_for_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.update.checking || self.closing {
            return;
        }
        let Some(target) = release_target() else {
            self.update.status = Some(UpdateStatus::SourceBuild);
            cx.notify();
            return;
        };
        self.update.checking = true;
        let sender = self.sender.clone();
        let skipped = self.settings.updates.skipped();
        let spawned = std::thread::Builder::new()
            .name("orin-update".into())
            .spawn(move || {
                let (sources, _) = sources(Timeouts::default());
                let status = check(&sources, &current_version(), Some(target), skipped.as_ref());
                if sender
                    .send(Event::UpdateChecked {
                        manual,
                        status,
                        at: now(),
                    })
                    .is_err()
                {
                    tracing::debug!(target: "orin::update", "window closed before the update check finished");
                }
            });
        if let Err(error) = spawned {
            tracing::error!(target: "orin::update", %error, "Update check thread could not start");
            self.update.checking = false;
            if manual {
                self.error = Some("Orin couldn't start checking for updates. Try again.".into());
            }
            self.schedule_update_check(RETRY_INTERVAL, cx);
        }
        cx.notify();
    }

    pub fn update_checked(
        &mut self,
        manual: bool,
        status: UpdateStatus,
        at: u64,
        cx: &mut Context<Self>,
    ) {
        self.update.checking = false;
        let failed = matches!(status, UpdateStatus::Failed(_));
        let known_offer = self
            .update
            .status
            .as_ref()
            .and_then(UpdateStatus::offered)
            .cloned();
        if failed {
            tracing::warn!(target: "orin::update", status = %status.message(&clock(at)), "Update check failed");
            if manual {
                self.error = Some(status.message(&clock(at)));
            }
        } else {
            self.settings.updates.last_check = Some(at);
            tracing::info!(target: "orin::update", status = %status.message(&clock(at)), "Update check finished");
        }
        // A failed check says nothing new about releases, so a known offer stays.
        if !(failed && known_offer.is_some()) {
            // The close button hides the banner until a different version is offered.
            if status
                .offered()
                .is_some_and(|offered| Some(offered) != known_offer.as_ref())
            {
                self.update.dismissed = false;
            }
            self.update.status = Some(status);
            self.update.checked_at = Some(at);
        }
        self.schedule_update_check(
            if failed {
                RETRY_INTERVAL
            } else {
                CHECK_INTERVAL
            },
            cx,
        );
        self.changed(cx);
    }

    pub fn toggle_update_checks(&mut self, cx: &mut Context<Self>) {
        let updates = &mut self.settings.updates;
        updates.check_automatically = !updates.check_automatically;
        let delay = next_check_delay(updates.last_check, now());
        // Turning checks off bumps the generation, so a pending timer does nothing.
        self.schedule_update_check(delay, cx);
        self.changed(cx);
    }

    /// Hide the offered version until a newer one appears.
    pub fn skip_update(&mut self, cx: &mut Context<Self>) {
        if let Some(version) = self
            .update
            .status
            .as_ref()
            .and_then(UpdateStatus::offered)
            .cloned()
        {
            self.settings.updates.skipped_version = Some(version.to_string());
            self.update.status = Some(UpdateStatus::Skipped(version));
            self.changed(cx);
        }
    }

    /// Offer the skipped version again, and check at once so the banner can show it.
    pub fn unskip_update(&mut self, cx: &mut Context<Self>) {
        if self.settings.updates.skipped_version.take().is_some() {
            if matches!(self.update.status, Some(UpdateStatus::Skipped(_))) {
                self.update.status = None;
            }
            self.changed(cx);
            self.check_for_updates(true, cx);
        }
    }

    /// Stop the download in progress; its temporary file is deleted.
    pub fn cancel_update(&mut self, cx: &mut Context<Self>) {
        if self.update.downloading.is_some() {
            tracing::info!(target: "orin::update", "Update download cancel requested");
            self.update.cancel.store(true, Ordering::Release);
            cx.notify();
        }
    }

    /// Hide the banner until the next start.
    pub fn dismiss_update(&mut self, cx: &mut Context<Self>) {
        self.update.dismissed = true;
        cx.notify();
    }

    /// Download, verify and install the offered release on the `orin-update-download` thread.
    pub fn install_update(&mut self, cx: &mut Context<Self>) {
        let Some(UpdateStatus::Available {
            release,
            asset,
            checksum,
        }) = self.update.status.clone()
        else {
            return;
        };
        if self.update.downloading.is_some() || self.update.installed.is_some() || self.closing {
            return;
        }
        let Some(dir) = download_dir() else {
            self.error =
                Some("Orin couldn't find its cache folder to download the update into.".into());
            cx.notify();
            return;
        };
        let version = release.version.clone();
        self.update.downloading = Some(Downloading {
            version: version.clone(),
            page: release.page.clone(),
            read: 0,
            total: None,
        });
        // A fresh flag, so an earlier Cancel doesn't stop this download.
        self.update.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.update.cancel.clone();
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name("orin-update-download".into())
            .spawn(move || {
                let (_, http) = sources(Timeouts::default());
                let mut last_percent = None;
                let progress = |read: u64, total: Option<u64>| {
                    // One event per whole percent (or per MiB without a size) keeps the channel quiet.
                    let step = total.map_or(read >> 20, |total| read * 100 / total.max(1));
                    if last_percent != Some(step) {
                        last_percent = Some(step);
                        tracing::debug!(target: "orin::update", step, "update download progress");
                        if sender.send(Event::UpdateProgress { read, total }).is_err() {
                            tracing::debug!(target: "orin::update", "window closed during the update download");
                        }
                    }
                };
                let result = download(&http, &asset, &checksum, &dir, &cancel, progress)
                    .and_then(|file| {
                        // Quit or Cancel during the hash check must not replace the binary.
                        if cancel.load(Ordering::Acquire) {
                            tracing::info!(target: "orin::update", "Update cancelled before install");
                            Err(UpdateError::Cancelled)
                        } else {
                            install(file)
                        }
                    })
                    .map(|executable| (version, executable));
                match &result {
                    Err(UpdateError::Cancelled) | Ok(_) => {}
                    Err(error) => tracing::error!(target: "orin::update", %error, "Update failed"),
                }
                if sender.send(Event::UpdateInstalled(result)).is_err() {
                    tracing::debug!(target: "orin::update", "window closed before the update finished");
                }
            });
        match spawned {
            Ok(thread) => self.update.download_thread = Some(thread),
            Err(error) => {
                tracing::error!(target: "orin::update", %error, "Update download thread could not start");
                self.update.downloading = None;
                self.error = Some("Orin couldn't start downloading the update. Try again.".into());
            }
        }
        cx.notify();
    }

    pub fn update_progress(&mut self, read: u64, total: Option<u64>, cx: &mut Context<Self>) {
        if let Some(downloading) = &mut self.update.downloading {
            downloading.read = read;
            downloading.total = total;
            cx.notify();
        }
    }

    pub fn update_installed(
        &mut self,
        result: Result<(Version, PathBuf), UpdateError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(downloading) = self.update.downloading.take() else {
            return;
        };
        match result {
            Ok((version, executable)) => {
                self.update.installed = Some((version, executable));
                // Restarting now would cut off an agent mid-turn; the banner offers Restart now.
                if self.agents_busy(cx) {
                    tracing::info!(target: "orin::update", "Update installed; restart waits for busy agents");
                } else {
                    self.restart_into_update(window, cx);
                }
            }
            Err(UpdateError::Cancelled) => {}
            Err(UpdateError::Checksum) => {
                self.error = Some(format!(
                    "The download of Orin {} didn't match its checksum, so it wasn't installed.",
                    downloading.version
                ));
            }
            Err(error) => {
                let page = &downloading.page;
                self.error = Some(match error {
                    UpdateError::Io(reason)
                        if reason.starts_with("Orin couldn't replace itself") =>
                    {
                        format!("{reason}. Download it from the release page instead: {page}")
                    }
                    other => format!(
                        "Orin {} couldn't be downloaded: {other}. Try again, or download it from {page}",
                        downloading.version
                    ),
                });
            }
        }
        cx.notify();
    }

    /// Quit and start the installed version. If quitting stops for unsaved edits, the restart
    /// stays requested, so Discard & close (or closing after saving) starts the new version.
    pub fn restart_into_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, executable)) = &self.update.installed else {
            return;
        };
        self.update.relaunch_after_quit = Some(executable.clone());
        self.begin_shutdown(window, cx);
    }

    /// Whether any agent is in the middle of a turn.
    pub fn agents_busy(&self, cx: &Context<Self>) -> bool {
        self.agents
            .iter()
            .any(|tab| tab.view.read(cx).snapshot.is_busy())
    }

    /// The banner's progress text, like `Downloading Orin 0.3.0… 42 %`.
    pub fn download_text(downloading: &Downloading) -> String {
        match downloading.total {
            Some(total) if total > 0 => format!(
                "Downloading Orin {}… {} %",
                downloading.version,
                downloading.read * 100 / total
            ),
            _ => format!(
                "Downloading Orin {}… {:.1} MiB",
                downloading.version,
                downloading.read as f64 / 1_048_576.0
            ),
        }
    }

    /// Settings › Updates status line.
    pub fn update_status_text(&self) -> String {
        if release_target().is_none() {
            return UpdateStatus::SourceBuild.message("");
        }
        if self.update.checking {
            return "Checking…".into();
        }
        match (&self.update.status, self.update.checked_at) {
            (Some(status), Some(at)) => status.message(&clock(at)),
            (Some(status), None) => status.message("--:--"),
            (None, _) => match self.settings.updates.last_check {
                Some(at) => format!(
                    "Not checked since Orin started (last check at {}).",
                    clock(at)
                ),
                None => "Not checked yet.".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_first_check_waits_a_minute_and_then_a_day() {
        let now = 1_000_000;
        assert_eq!(next_check_delay(None, now), FIRST_CHECK);
        assert_eq!(
            next_check_delay(Some(now - 3_600), now),
            Duration::from_secs(23 * 3_600)
        );
        assert_eq!(next_check_delay(Some(now - 2 * 86_400), now), FIRST_CHECK);
        assert_eq!(next_check_delay(Some(now - 86_400 + 10), now), FIRST_CHECK);
    }

    #[test]
    fn regression_a_last_check_in_the_future_waits_one_interval_and_never_overflows() {
        let now = 1_000_000;
        assert_eq!(
            next_check_delay(Some(now + 7 * 86_400), now),
            CHECK_INTERVAL
        );
        assert_eq!(next_check_delay(Some(u64::MAX), now), CHECK_INTERVAL);
        assert_eq!(next_check_delay(Some(0), u64::MAX), FIRST_CHECK);
    }

    #[test]
    fn unit_download_text_shows_percent_or_size() {
        let mut downloading = Downloading {
            version: Version::new(0, 3, 0),
            page: String::new(),
            read: 42,
            total: Some(100),
        };
        assert_eq!(
            Workspace::download_text(&downloading),
            "Downloading Orin 0.3.0… 42 %"
        );
        downloading.total = None;
        downloading.read = 3 * 1_048_576;
        assert_eq!(
            Workspace::download_text(&downloading),
            "Downloading Orin 0.3.0… 3.0 MiB"
        );
    }

    #[test]
    fn unit_clock_is_hours_and_minutes() {
        let text = clock(now());
        assert_eq!(text.len(), 5, "{text}");
        assert_eq!(&text[2..3], ":");
        assert_eq!(clock(u64::MAX), "--:--");
    }

    #[test]
    fn unit_banner_follows_offer_download_and_install() {
        let mut state = UpdateState::new(true);
        assert!(!state.banner_visible());
        state.status = Some(UpdateStatus::UpToDate);
        assert!(!state.banner_visible());
        state.downloading = Some(Downloading {
            version: Version::new(0, 3, 0),
            page: String::new(),
            read: 0,
            total: None,
        });
        assert!(state.banner_visible());
        state.downloading = None;
        state.installed = Some((Version::new(0, 3, 0), PathBuf::from("orin")));
        state.dismissed = true;
        assert!(state.banner_visible(), "an installed update stays visible");
    }
}
