// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 18:00:39.064029
//+ Revised: 	2026/11/13 07:53:51.974512
// -----------------------------------------------------------------------------

//! Native recursive change observation with bounded, coalesced notification state.

use super::{ProjectError, io_error};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, TryLockError,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_ERROR_CHARACTERS: usize = 2048;

#[derive(Default)]
struct WatchState {
    root: PathBuf,
    changed: AtomicBool,
    failure: Mutex<Option<WatchFailure>>,
}

struct WatchFailure {
    first: String,
    count: u64,
}

impl WatchState {
    fn record(&self, result: notify::Result<Event>) {
        match result {
            Ok(event) if !matches!(event.kind, EventKind::Access(_)) && self.relevant(&event) => {
                self.changed.store(true, Ordering::Release)
            }
            Ok(_) => {}
            Err(error) => self.record_failure(error),
        }
    }

    fn relevant(&self, event: &Event) -> bool {
        event.paths.is_empty()
            || event.paths.iter().any(|path| {
                let relative = path.strip_prefix(&self.root).unwrap_or(path);
                let mut components = relative.components();
                let mut first = true;
                while let Some(component) = components.next() {
                    let name = component.as_os_str();
                    // Dependency folders are generated wherever they appear; `target`, `build`
                    // and `dist` only at the project root, since `src/build/` is source.
                    let generated = ["node_modules", ".venv"]
                        .iter()
                        .any(|ignored| name == *ignored)
                        || (first
                            && ["target", "build", "dist"]
                                .iter()
                                .any(|ignored| name == *ignored));
                    if generated {
                        return false;
                    }
                    first = false;
                    if name == ".git" {
                        return !components.next().is_some_and(|part| {
                            part.as_os_str() == "objects" || part.as_os_str() == "logs"
                        });
                    }
                }
                true
            })
    }

    fn record_failure(&self, error: notify::Error) {
        let Ok(mut failure) = self.failure.lock() else {
            tracing::error!(target: "orin::project", "file watcher error state lock was poisoned");
            return;
        };
        if let Some(failure) = failure.as_mut() {
            failure.count = failure.count.saturating_add(1);
        } else {
            *failure = Some(WatchFailure {
                first: error
                    .to_string()
                    .chars()
                    .take(MAX_ERROR_CHARACTERS)
                    .collect(),
                count: 1,
            });
        }
    }

    fn take(&self, root: &Path) -> Result<bool, ProjectError> {
        let failure = match self.failure.try_lock() {
            Ok(mut pending) => pending.take(),
            Err(TryLockError::WouldBlock) => return Ok(false),
            Err(TryLockError::Poisoned(_)) => {
                return Err(io_error(
                    root,
                    std::io::Error::other("File watcher error state is unavailable"),
                ));
            }
        };
        if let Some(failure) = failure {
            return Err(io_error(
                root,
                std::io::Error::other(format!(
                    "File watcher failed ({} errors): {}",
                    failure.count, failure.first
                )),
            ));
        }
        Ok(self.changed.swap(false, Ordering::AcqRel))
    }
}

/// Owns a platform-native recursive watcher. Construction and destruction may block;
/// keep both on the workspace's background workers, including during project switches.
#[must_use]
pub struct ProjectWatch {
    _watcher: RecommendedWatcher,
    root: PathBuf,
    state: Arc<WatchState>,
}

impl ProjectWatch {
    pub fn new(root: &Path) -> Result<Self, ProjectError> {
        let root = root.canonicalize().map_err(|error| io_error(root, error))?;
        if !root.is_dir() {
            return Err(ProjectError::NotDirectory);
        }
        let state = Arc::new(WatchState {
            root: root.clone(),
            ..WatchState::default()
        });
        let events = state.clone();
        let mut watcher = notify::recommended_watcher(move |result| events.record(result))
            .map_err(|error| io_error(&root, std::io::Error::other(error.to_string())))?;
        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|error| io_error(&root, std::io::Error::other(error.to_string())))?;
        tracing::info!(target: "orin::project", "native project watch started");
        Ok(Self {
            _watcher: watcher,
            root,
            state,
        })
    }

    /// Drains a compact event batch without waiting for filesystem IO. Pending errors
    /// are reported independently, leaving a pending change available on the next call.
    pub fn take_changed(&self) -> Result<bool, ProjectError> {
        self.state.take(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::AccessKind;

    #[test]
    fn access_events_are_ignored_and_mutations_coalesce() {
        let state = WatchState::default();
        state.record(Ok(Event::new(EventKind::Access(AccessKind::Any))));
        assert!(!state.take(Path::new("root")).unwrap());
        for _ in 0..100 {
            state.record(Ok(Event::new(EventKind::Any)));
        }
        assert!(state.take(Path::new("root")).unwrap());
        assert!(!state.take(Path::new("root")).unwrap());
    }

    #[test]
    fn errors_remain_visible_independently_from_pending_mutations() {
        let state = WatchState::default();
        state.record(Ok(Event::new(EventKind::Any)));
        state.record(Err(notify::Error::generic("first fixture failure")));
        state.record(Err(notify::Error::generic("second fixture failure")));
        let error = state.take(Path::new("root")).unwrap_err().to_string();
        assert!(error.contains("2 errors") && error.contains("first fixture failure"));
        assert!(state.take(Path::new("root")).unwrap());
        assert!(!state.take(Path::new("root")).unwrap());
    }

    #[test]
    fn polling_never_blocks_while_a_callback_owns_error_state() {
        let state = WatchState::default();
        let _guard = state.failure.lock().unwrap();
        assert!(!state.take(Path::new("root")).unwrap());
    }

    #[test]
    fn generated_paths_are_ignored_but_git_index_head_and_refs_are_relevant() {
        let state = WatchState::default();
        for path in [
            "target/debug/app",
            "ui/node_modules/pkg/file",
            ".venv/lib/file",
            "build/output",
            "dist/app",
            ".git/objects/object",
            ".git/logs/HEAD",
        ] {
            state.record(Ok(Event::new(EventKind::Any).add_path(PathBuf::from(path))));
            assert!(!state.take(Path::new("root")).unwrap(), "{path}");
        }
        for path in [
            "src/main.rs",
            "src/build/main.rs",
            "docs/dist/index.html",
            ".git/index",
            ".git/HEAD",
            ".git/refs/heads/main",
        ] {
            state.record(Ok(Event::new(EventKind::Any).add_path(PathBuf::from(path))));
            assert!(state.take(Path::new("root")).unwrap(), "{path}");
        }
    }
}
