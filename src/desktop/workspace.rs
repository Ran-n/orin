// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:27:01.254950
//+ Revised: 	2026/11/13 17:30:08.374824
// -----------------------------------------------------------------------------

//! Presentation state and event delivery for one native workspace window.

use super::{
    agent_view::AgentView, columns::ColumnDrag, columns::ColumnMove, controls::Controls,
    driver::Driver, editor::TextEditor, events::Event, settings_page::FontSlot,
    terminal_view::TerminalView,
};
use crate::{
    agents::{
        AgentSession, Provider, ProviderStatus,
        accounts::{Account, Environment},
    },
    background::Workers,
    project::{Entry, Project},
    settings::{SettingsStore, WorkspaceSettings},
    startup::Options,
    terminal::Session,
};
use gpui::{App, Context, Entity, FocusHandle, Window};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

pub struct ShellTab {
    pub view: Entity<TerminalView>,
    pub session: Arc<Mutex<Session>>,
}

pub struct AgentTab {
    pub provider: Provider,
    pub view: Entity<AgentView>,
    pub session: Arc<Mutex<AgentSession>>,
    /// The agent's remembered choices (model, effort, mode) were sent once it was ready.
    pub options_applied: bool,
    /// Choices the user made that the agent hasn't confirmed yet; remembered once it does.
    pub pending_options: Vec<PendingOption>,
}

/// A choice sent to an agent, waiting for it to take or refuse it.
pub struct PendingOption {
    pub option: String,
    pub value: String,
    /// The session's error when the choice was made; a different one means it was refused.
    pub error_before: Option<String>,
}

/// Everything resolved before the window opens, handed to the workspace in one piece.
pub struct Startup {
    pub options: Options,
    pub store: SettingsStore,
    pub settings: WorkspaceSettings,
    pub providers: Vec<(Provider, ProviderStatus)>,
    /// A problem found while loading settings, shown once the window is up.
    pub error: Option<String>,
    /// False when the existing settings file could not be read and must not be overwritten.
    pub writable: bool,
}

pub struct Workspace {
    pub settings: WorkspaceSettings,
    pub store: SettingsStore,
    pub writable: bool,
    /// The settings file couldn't be read at start, so Retry saving reads it again first
    /// instead of writing over what the user may have repaired.
    pub settings_unread: bool,
    /// The Markdown preview's last parse, reused while the text is unchanged.
    pub markdown_cache: super::markdown::ParseCache,
    pub error: Option<String>,
    pub project: Option<Project>,
    pub entries: BTreeMap<PathBuf, Vec<Entry>>,
    pub content: String,
    pub project_generation: u64,
    pub file_generation: u64,
    pub document_generation: u64,
    pub loading: bool,
    pub launching: bool,
    pub agents: Vec<AgentTab>,
    pub shell: Option<ShellTab>,
    pub editor: Option<Entity<TextEditor>>,
    pub discard_on_quit: bool,
    pub test_agent: Option<PathBuf>,
    pub watch: Option<crate::project::ProjectWatch>,
    pub watch_pending: Option<Instant>,
    /// Branch and changed files for the Files column's decorations and the status bar.
    pub git: Option<crate::project::GitStatus>,
    /// Git status couldn't be read (shown in the status bar).
    pub git_failed: bool,
    pub providers: Vec<(Provider, ProviderStatus)>,
    pub workers: Option<Workers>,
    pub sender: mpsc::Sender<Event>,
    pub receiver: mpsc::Receiver<Event>,
    pub controls: Controls,
    pub driver: Option<Driver>,
    pub dirty: Option<Instant>,
    pub save_inflight: bool,
    pub closing: bool,
    pub focus: FocusHandle,
    /// The column divider being dragged, if any.
    pub drag: Option<ColumnDrag>,
    /// The column being moved by its title, if any.
    pub moving: Option<ColumnMove>,
    /// Focus for the Settings page, so Escape closes it.
    pub settings_focus: FocusHandle,
    /// The font picker open on the Settings page, if any.
    pub font_picker: Option<FontSlot>,
    /// Installed font families, listed the first time a picker opens.
    pub font_names: Option<Vec<String>>,
    /// Agent sign-in status; `None` until checked.
    pub accounts: Option<Vec<Account>>,
    pub accounts_checking: bool,
    pub account_environment: Environment,
    /// A command typed into the shell as soon as it starts (an agent sign-in).
    pub shell_input: Option<String>,
    /// Move keyboard focus to the shell when it starts; not for the automatic start.
    pub shell_focus: bool,
    /// The open drop-down menu, if any.
    pub menu: Option<super::menus::OpenMenu>,
    pub menu_focus: FocusHandle,
    /// Device pixels per logical pixel, refreshed every frame for crisp image sizes.
    pub scale_factor: f32,
    /// Update checks, downloads, and the banner.
    pub update: super::update_actions::UpdateState,
    /// The `--settings` file this run uses, so a restart after an update reopens it.
    pub settings_path: Option<PathBuf>,
}

impl Workspace {
    pub fn new(startup: Startup, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Startup {
            options,
            store,
            settings,
            providers,
            error,
            writable,
        } = startup;
        super::theme::apply(&settings.appearance);
        let (sender, receiver) = mpsc::channel();
        let project = options.project.clone().or_else(|| settings.project.clone());
        let mut account_environment = Environment::current();
        account_environment.path =
            crate::process::launch::tool_path(&settings.tool_directories, !options.test_driver);
        let mut workspace = Self {
            settings,
            store,
            writable,
            settings_unread: !writable,
            markdown_cache: super::markdown::ParseCache::default(),
            error,
            project: None,
            entries: BTreeMap::new(),
            content: String::new(),
            project_generation: 0,
            file_generation: 0,
            document_generation: 0,
            loading: false,
            launching: false,
            agents: vec![],
            shell: None,
            editor: None,
            discard_on_quit: false,
            test_agent: None,
            watch: None,
            watch_pending: None,
            git: None,
            git_failed: false,
            providers,
            workers: None,
            sender,
            receiver,
            controls: Default::default(),
            driver: None,
            dirty: None,
            save_inflight: false,
            closing: false,
            focus: cx.focus_handle(),
            drag: None,
            moving: None,
            settings_focus: cx.focus_handle(),
            font_picker: None,
            font_names: None,
            accounts: None,
            accounts_checking: false,
            account_environment,
            shell_input: None,
            shell_focus: true,
            menu: None,
            menu_focus: cx.focus_handle(),
            scale_factor: window.scale_factor(),
            update: super::update_actions::UpdateState::new(options.test_driver),
            settings_path: options.settings.clone(),
        };
        workspace.initialize(options, window, cx);
        match project {
            Some(path) => workspace.open_project(path, cx),
            // Like VS Code, a start without a folder shows the Welcome page.
            None => {
                if workspace.settings.page.is_none() {
                    workspace.settings.page = Some(crate::settings::Page::Welcome);
                }
            }
        }
        workspace
    }

    fn initialize(&mut self, options: Options, window: &mut Window, cx: &mut Context<Self>) {
        match Workers::new(2, 32) {
            Ok(workers) => self.workers = Some(workers),
            Err(error) => {
                tracing::error!(%error, "IO workers failed");
                self.error = Some(
                    "Orin could not start its background workers. Close it and try again.".into(),
                );
            }
        }
        if options.test_driver {
            #[cfg(feature = "agent-fixture")]
            if let Some(path) = std::env::var_os("ORIN_TEST_AGENT")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
            {
                self.test_agent = Some(path.clone());
                for (provider, status) in &mut self.providers {
                    status.available = *provider == Provider::Claude;
                    status.executable = (*provider == Provider::Claude).then(|| path.clone());
                }
            }
            match Driver::new() {
                Ok(driver) => self.driver = Some(driver),
                Err(error) => {
                    tracing::error!(%error, "UI driver failed");
                    self.error = Some("The UI test driver could not start.".into());
                }
            }
        }
        let delay = super::update_actions::next_check_delay(
            self.settings.updates.last_check,
            super::update_actions::now(),
        );
        self.schedule_update_check(delay, cx);
        let view = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            if let Err(error) = view.update(cx, |view, cx| view.begin_shutdown(window, cx)) {
                tracing::warn!(%error, "Workspace already closed");
                cx.quit();
            }
            false
        });
        cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                if view
                    .update_in(cx, |view, window, cx| view.tick(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn submit(&mut self, job: impl FnOnce() + Send + 'static) -> bool {
        // No workers means either shutdown took them or they never started.
        let missing = if self.closing {
            "The workspace is closing."
        } else {
            "Orin's background workers aren't running. Close Orin and start it again."
        };
        let result = self
            .workers
            .as_ref()
            .ok_or(missing)
            .and_then(|workers| workers.submit(job));
        if let Err(message) = result {
            tracing::warn!(target: "orin::desktop", message, "Background job refused");
            self.error = Some(message.into());
            return false;
        }
        true
    }

    /// Retry saving. After a failed write the session's settings are written again; after a
    /// file that couldn't be read at start, the file is read first, and a repaired one keeps
    /// its preferences, with only this session's window and project state laid over it.
    pub fn retry_saving(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        if !self.settings_unread {
            self.writable = true;
            self.changed(cx);
            return;
        }
        let store = self.store.clone();
        let sender = self.sender.clone();
        self.submit(move || {
            let result = store.load().map_err(|error| {
                tracing::warn!(target: "orin::settings", %error, "Settings file still unreadable");
                format!(
                    "The settings file still can't be read ({error}). Repair or delete {}, then retry.",
                    store.path().display()
                )
            });
            if sender.send(Event::SettingsReread(result)).is_err() {
                tracing::debug!("Workspace closed before the settings were read again");
            }
        });
        cx.notify();
    }

    pub fn settings_reread(
        &mut self,
        result: Result<crate::settings::WorkspaceSettings, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(stored) => {
                tracing::info!(target: "orin::settings", "Repaired settings file read; saving resumes");
                self.settings = session_over_stored(stored, &self.settings);
                self.settings_unread = false;
                self.writable = true;
                self.changed(cx);
            }
            Err(message) => {
                self.error = Some(message);
                cx.notify();
            }
        }
    }

    /// The open file has unsaved edits or a save in flight, so it must not be replaced.
    pub fn editor_busy(&self, cx: &App) -> bool {
        self.editor.as_ref().is_some_and(|editor| {
            let editor = editor.read(cx);
            editor.dirty() || editor.saving
        })
    }

    pub fn changed(&mut self, cx: &mut Context<Self>) {
        self.dirty = Some(Instant::now());
        cx.notify();
    }

    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Ok(event) = self.receiver.try_recv() {
            self.apply_event(event, window, cx);
        }
        self.poll_watch(cx);
        self.poll_sessions(cx);
        self.poll_driver(window, cx);
        self.observe_bounds(window, cx);
        self.save_if_ready();
    }

    fn observe_bounds(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        let bounds = window.bounds();
        let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let position = Some((bounds.origin.x.into(), bounds.origin.y.into()));
        if (
            self.settings.width,
            self.settings.height,
            self.settings.position,
        ) != (width, height, position)
        {
            self.settings.width = width;
            self.settings.height = height;
            self.settings.position = position;
            self.changed(cx);
        }
    }

    fn save_if_ready(&mut self) {
        if self.closing
            || !self.writable
            || self.save_inflight
            || self
                .dirty
                .is_none_or(|time| time.elapsed() < Duration::from_millis(350))
        {
            return;
        }
        let settings = self.settings.clone();
        let store = self.store.clone();
        let sender = self.sender.clone();
        if self.submit(move || {
            let result = store.save(&settings).map_err(|error| {
                tracing::error!(%error, "Settings save failed");
                "Workspace settings could not be saved. Check that the settings folder is writable."
                    .into()
            });
            if sender.send(Event::Saved(result)).is_err() {
                tracing::debug!("Workspace closed before settings acknowledgement");
            }
        }) {
            self.save_inflight = true;
            self.dirty = None;
        }
    }
}

/// A repaired settings file's preferences, with what this session has open laid over them, so
/// the window, project, and file don't jump when saving resumes.
pub fn session_over_stored(
    mut stored: crate::settings::WorkspaceSettings,
    session: &crate::settings::WorkspaceSettings,
) -> crate::settings::WorkspaceSettings {
    stored.project.clone_from(&session.project);
    stored.selected_file.clone_from(&session.selected_file);
    stored.viewer_mode = session.viewer_mode;
    stored.editor_cursor = session.editor_cursor;
    stored
        .expanded_folders
        .clone_from(&session.expanded_folders);
    stored.page = session.page;
    stored.active_session = session.active_session;
    stored.width = session.width;
    stored.height = session.height;
    stored.position = session.position;
    if let Some(project) = &session.project {
        stored.remember_project(&crate::process::launch::native_path(project));
    }
    stored
}

#[cfg(test)]
mod retry_tests {
    use super::session_over_stored;
    use crate::settings::WorkspaceSettings;
    use std::path::PathBuf;

    #[test]
    fn unit_a_repaired_file_keeps_its_preferences_under_the_open_session() {
        let mut stored = WorkspaceSettings::default();
        stored.appearance.text_size = 18.;
        stored.updates.check_automatically = false;
        stored.recent_projects = vec![PathBuf::from("/old")];
        stored.width = 1200.;
        let session = WorkspaceSettings {
            width: 1600.,
            selected_file: Some(PathBuf::from("README.md")),
            ..WorkspaceSettings::default()
        };
        let merged = session_over_stored(stored, &session);
        assert_eq!(
            merged.appearance.text_size, 18.,
            "the repaired preference stays"
        );
        assert!(!merged.updates.check_automatically);
        assert_eq!(merged.recent_projects, vec![PathBuf::from("/old")]);
        assert_eq!(merged.width, 1600., "the open window keeps its size");
        assert_eq!(merged.selected_file, Some(PathBuf::from("README.md")));
    }
}
