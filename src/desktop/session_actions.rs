// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:42:12.623216
//+ Revised: 	2026/11/12 10:31:40.895275
// -----------------------------------------------------------------------------

//! Shell sessions belong to the Terminal pane, independently of structured agents.
use super::{
    events::Event,
    terminal_view::TerminalView,
    workspace::{ShellTab, Workspace},
};
use crate::{settings::Column, terminal::Session};
use gpui::{Context, Window, prelude::*};
use std::sync::{Arc, Mutex};

impl Workspace {
    pub fn launch_shell(&mut self, cx: &mut Context<Self>) {
        if self.closing
            || self.launching
            || self.project.is_none()
            || self
                .shell
                .as_ref()
                .is_some_and(|tab| tab.view.read(cx).running())
        {
            return;
        }
        let path = self
            .project
            .as_ref()
            .expect("project checked")
            .root()
            .to_owned();
        let sender = self.sender.clone();
        let generation = self.project_generation;
        if self.submit(move || {
            let result = Session::spawn_shell(&path, 24, 80);
            if sender.send(Event::Shell { generation, result }).is_err() {
                tracing::debug!("Workspace closed during shell start");
            }
        }) {
            self.launching = true;
            cx.notify();
        }
    }
    pub fn shell_ready(
        &mut self,
        generation: u64,
        result: anyhow::Result<Session>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.launching = false;
        match result {
            Ok(session) if generation == self.project_generation && !self.closing => {
                let session = Arc::new(Mutex::new(session));
                let handle = session.clone();
                let controls = self.controls.clone();
                let view =
                    cx.new(|cx| TerminalView::new(handle, controls, "terminal-input".into(), cx));
                cx.observe(&view, |_, _, cx| cx.notify()).detach();
                if std::mem::replace(&mut self.shell_focus, true) {
                    window.focus(&view.read(cx).focus);
                }
                if let Some(line) = self.shell_input.take() {
                    view.update(cx, |view, cx| view.send(line.as_bytes(), cx));
                }
                self.shell = Some(ShellTab { view, session });
                self.expand_column(Column::Terminal);
                self.changed(cx);
            }
            Ok(mut session) => {
                self.submit(move || {
                    if let Err(error) = session.stop() {
                        tracing::error!(%error,"Stale shell cleanup failed");
                    }
                });
            }
            Err(error) => {
                tracing::warn!(%error,"Shell start failed");
                self.error = Some(
                    "The terminal could not start. Check that your system shell is available."
                        .into(),
                );
            }
        }
    }
    pub fn stop_shell(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = &self.shell else {
            return;
        };
        let session = tab.session.clone();
        let sender = self.sender.clone();
        self.submit(move || {
            let result = session
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .stop()
                .map_err(|error| {
                    tracing::error!(%error,"Shell stop failed");
                    "The shell could not stop cleanly. Close Orin before starting more work.".into()
                });
            if sender.send(Event::Stopped(result)).is_err() {
                tracing::debug!("Workspace closed during shell stop");
            }
        });
        cx.notify();
    }
    pub fn poll_sessions(&mut self, cx: &mut Context<Self>) {
        for tab in &self.agents {
            tab.view.update(cx, |view, cx| view.poll(cx));
        }
        self.apply_agent_preferences(cx);
        if let Some(tab) = &self.shell {
            tab.view.update(cx, |view, cx| view.poll(cx));
        }
    }
    pub fn toggle_terminal(&mut self, cx: &mut Context<Self>) {
        self.toggle_column(Column::Terminal, cx);
    }

    /// Start the default shell without taking focus, as a project opens.
    pub fn autostart_shell(&mut self, cx: &mut Context<Self>) {
        if self.shell.is_some() || self.launching || self.closing {
            return;
        }
        tracing::info!(target: "orin::terminal", "starting the default shell for the project");
        self.shell_focus = false;
        self.launch_shell(cx);
        if !self.launching {
            self.shell_focus = true;
        }
    }

    /// Send each newly ready session the agent's remembered choices it still offers.
    fn apply_agent_preferences(&mut self, cx: &mut Context<Self>) {
        self.remember_confirmed_options(cx);
        for tab in &mut self.agents {
            if tab.options_applied {
                continue;
            }
            let snapshot = &tab.view.read(cx).snapshot;
            if snapshot.status != crate::agents::AgentStatus::Ready
                || snapshot.config_options.is_empty()
            {
                continue;
            }
            tab.options_applied = true;
            let Some(saved) = self
                .settings
                .agent_options
                .get(super::agent_actions::provider_id(tab.provider))
            else {
                continue;
            };
            let wanted: Vec<(String, String)> = snapshot
                .config_options
                .iter()
                .filter_map(|option| {
                    let value = saved.get(&option.id)?;
                    (option.current != *value && option.offers(value))
                        .then(|| (option.id.clone(), value.clone()))
                })
                .collect();
            let session = tab
                .session
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            for (option, value) in wanted {
                if let Err(error) = session.set_config_option(&option, &value) {
                    tracing::warn!(target: "orin::agents", %error, option, "remembered agent option was not applied");
                }
            }
        }
    }

    /// Remember choices the agent has taken; forget ones it refused, so a model a plan doesn't
    /// include isn't tried again in every new session.
    fn remember_confirmed_options(&mut self, cx: &mut Context<Self>) {
        let mut confirmed = Vec::new();
        for tab in &mut self.agents {
            if tab.pending_options.is_empty() {
                continue;
            }
            let snapshot = &tab.view.read(cx).snapshot;
            let provider = super::agent_actions::provider_id(tab.provider);
            tab.pending_options.retain(|pending| {
                let taken = snapshot
                    .config_options
                    .iter()
                    .any(|offered| offered.id == pending.option && offered.current == pending.value);
                if taken {
                    confirmed.push((provider, pending.option.clone(), pending.value.clone()));
                    return false;
                }
                if snapshot.error.is_some() && snapshot.error != pending.error_before {
                    tracing::info!(target: "orin::agents", option = %pending.option, "agent refused the choice; it is not remembered");
                    return false;
                }
                true
            });
        }
        if confirmed.is_empty() {
            return;
        }
        for (provider, option, value) in confirmed {
            self.settings
                .agent_options
                .entry(provider.to_owned())
                .or_default()
                .insert(option, value);
        }
        self.changed(cx);
    }

    /// Set an option of the session at `index`; it is remembered for that agent once taken.
    pub fn set_agent_option(
        &mut self,
        index: usize,
        option: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.agents.get_mut(index) else {
            return;
        };
        let result = tab
            .session
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .set_config_option(option, value);
        match result {
            Ok(()) => {
                let error_before = tab.view.read(cx).snapshot.error.clone();
                tab.pending_options
                    .retain(|pending| pending.option != option);
                tab.pending_options.push(super::workspace::PendingOption {
                    option: option.to_owned(),
                    value: value.to_owned(),
                    error_before,
                });
                cx.notify();
            }
            Err(error) => {
                tracing::warn!(target: "orin::agents", %error, option, "agent option could not be changed");
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }
}
