// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:42:12.796873
//+ Revised: 	2026/11/12 15:10:20.864725
// -----------------------------------------------------------------------------

//! Structured agent session creation and conversation selection.
use super::{
    agent_view::{AgentAction, AgentView},
    events::Event,
    workspace::{AgentTab, Workspace},
};
use crate::{
    agents::{AgentSession, Provider},
    settings::Column,
};
use gpui::{Context, Window, prelude::*};
use std::sync::{Arc, Mutex};

pub const MAX_AGENT_SESSIONS: usize = 8;
pub fn provider_id(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
        Provider::Gemini => "gemini",
    }
}
impl Workspace {
    pub fn can_launch(&self, provider: Provider) -> bool {
        !self.closing
            && !self.loading
            && !self.launching
            && self.project.is_some()
            && self.agents.len() < MAX_AGENT_SESSIONS
            && self
                .providers
                .iter()
                .any(|(kind, status)| *kind == provider && status.available)
    }
    pub fn launch_agent(&mut self, provider: Provider, cx: &mut Context<Self>) {
        if !self.can_launch(provider) {
            return;
        }
        let path = self
            .project
            .as_ref()
            .expect("project checked")
            .root()
            .to_owned();
        let generation = self.project_generation;
        let sender = self.sender.clone();
        let fixture = self
            .test_agent
            .clone()
            .filter(|_| provider == Provider::Claude);
        if self.submit(move || {
            let result = match fixture {
                Some(executable) => AgentSession::spawn_command(
                    provider,
                    std::process::Command::new(executable),
                    &path,
                ),
                None => AgentSession::spawn(provider, &path),
            };
            if sender
                .send(Event::Agent {
                    generation,
                    provider,
                    result,
                })
                .is_err()
            {
                tracing::debug!("Workspace closed during agent start");
            }
        }) {
            self.launching = true;
            cx.notify();
        }
    }
    pub fn agent_ready(
        &mut self,
        generation: u64,
        provider: Provider,
        result: anyhow::Result<AgentSession>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.launching = false;
        match result {
            Ok(session) if generation == self.project_generation && !self.closing => {
                let session = Arc::new(Mutex::new(session));
                let handle = session.clone();
                let controls = self.controls.clone();
                let view = cx.new(|cx| AgentView::new(handle, controls, cx));
                cx.observe(&view, |_, _, cx| cx.notify()).detach();
                cx.subscribe(&view, |view, _, action, cx| match action {
                    AgentAction::OpenFile(path) => {
                        if let Some(project) = view.project.clone() {
                            let path = path.clone();
                            let generation = view.project_generation;
                            let sender = view.sender.clone();
                            view.submit(move || {
                                let result = project.reported_file(&path);
                                if sender
                                    .send(Event::ReportedFile { generation, result })
                                    .is_err()
                                {
                                    tracing::debug!("Workspace closed during change review");
                                }
                            });
                            cx.notify();
                        }
                    }
                })
                .detach();
                self.agents.push(AgentTab {
                    provider,
                    view,
                    session,
                    options_applied: false,
                    pending_options: Vec::new(),
                });
                self.settings.active_session = self.agents.len() - 1;
                self.expand_column(Column::Agents);
                self.settings.selected_provider = provider_id(provider).into();
                self.changed(cx);
            }
            Ok(mut session) => {
                self.submit(move || {
                    if let Err(error) = session.stop() {
                        tracing::error!(%error,"Stale agent cleanup failed");
                    }
                });
            }
            Err(error) => {
                tracing::warn!(%error,"Agent start failed");
                self.error = Some(format!("{} could not start: {error:#}", provider.label()));
            }
        }
    }
    pub fn close_agent(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.agents.len() {
            return;
        }
        let tab = self.agents.remove(index);
        let session = tab.session.clone();
        let sender = self.sender.clone();
        self.submit(move || {
            let result = session
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .stop()
                .map_err(|error| {
                    tracing::error!(%error,"Agent stop failed");
                    "The agent could not stop cleanly. Close Orin before starting more work.".into()
                });
            if sender.send(Event::Stopped(result)).is_err() {
                tracing::debug!("Workspace closed during agent stop");
            }
        });
        self.settings.active_session = self
            .settings
            .active_session
            .min(self.agents.len().saturating_sub(1));
        self.changed(cx);
    }
}
