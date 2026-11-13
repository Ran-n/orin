// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:36:12.170995
//+ Revised: 	2026/11/12 22:43:39.395872
// -----------------------------------------------------------------------------

//! Isolated UI-test protocol; input is cancellable and never controls the real cursor.

use super::{agent_actions::provider_id, workspace::Workspace};
use crate::settings::{Column, Page, ViewerMode};
use gpui::{Context, Window, px, size};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Deserialize)]
pub struct Request {
    pub id: u64,
    pub command: String,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

pub struct Driver {
    pub receiver: Receiver<Request>,
    stop: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
}

impl Driver {
    pub fn new() -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(32);
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let reader = thread::Builder::new()
            .name("orin-ui-driver".into())
            .spawn(move || {
                let mut pending = Vec::new();
                let mut input = io::stdin();
                let mut bytes = [0_u8; 4096];
                while !cancelled.load(Ordering::Acquire) {
                    match available_input() {
                        Ok(0) => {
                            thread::sleep(Duration::from_millis(15));
                            continue;
                        }
                        Err(_) => break,
                        Ok(_) => {}
                    }
                    let count = match input.read(&mut bytes) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => count,
                    };
                    pending.extend_from_slice(&bytes[..count]);
                    if pending.len() > 65_536 {
                        reply(json!({"id":0,"ok":false,"error":"Driver request is too large."}));
                        break;
                    }
                    while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
                        let line = pending.drain(..=end).collect::<Vec<_>>();
                        match serde_json::from_slice::<Request>(&line) {
                            Ok(request) => {
                                let quit = request.command == "quit";
                                let id = request.id;
                                if sender.try_send(request).is_err() {
                                    reply(
                                        json!({"id":id,"ok":false,"error":"The driver is busy."}),
                                    );
                                }
                                if quit {
                                    return;
                                }
                            }
                            Err(_) => {
                                reply(json!({"id":0,"ok":false,"error":"Invalid driver request."}))
                            }
                        }
                    }
                }
            })?;
        Ok(Self {
            receiver,
            stop,
            reader: Some(reader),
        })
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if self
            .reader
            .take()
            .is_some_and(|reader| reader.join().is_err())
        {
            tracing::error!("UI driver reader failed");
        }
    }
}

#[cfg(windows)]
fn available_input() -> io::Result<u32> {
    use windows_sys::Win32::System::{
        Console::{GetStdHandle, STD_INPUT_HANDLE},
        Pipes::PeekNamedPipe,
    };
    let mut available = 0;
    // SAFETY: stdout belongs to this driver; the probe never consumes bytes or changes input state.
    let success = unsafe {
        PeekNamedPipe(
            GetStdHandle(STD_INPUT_HANDLE),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(available)
}

#[cfg(unix)]
fn available_input() -> io::Result<u32> {
    let mut descriptor = libc::pollfd {
        fd: 0,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll receives one live stack descriptor with a nonblocking timeout.
    let result = unsafe { libc::poll(&mut descriptor, 1, 0) };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(u32::from(result > 0))
    }
}

fn reply(value: Value) {
    let mut output = io::stdout().lock();
    if writeln!(output, "@@{value}")
        .and_then(|_| output.flush())
        .is_err()
    {
        tracing::warn!("UI driver output closed");
    }
}

impl Workspace {
    pub fn poll_driver(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let requests = self
            .driver
            .as_ref()
            .map(|driver| driver.receiver.try_iter().take(16).collect::<Vec<_>>())
            .unwrap_or_default();
        for request in requests {
            match request.command.as_str() {
                "state" => {
                    reply(json!({"id":request.id,"ok":true,"state":self.driver_state(window,cx)}))
                }
                "resize" => {
                    if let (Some(width), Some(height)) = (request.width, request.height)
                        && width.is_finite()
                        && height.is_finite()
                        && (1000.0..=7680.).contains(&width)
                        && (680.0..=4320.).contains(&height)
                    {
                        window.resize(size(px(width), px(height)));
                        reply(json!({"id":request.id,"ok":true}));
                        continue;
                    }
                    reply(json!({"id":request.id,"ok":false,"error":"Invalid window size."}));
                }
                "quit" => {
                    reply(json!({"id":request.id,"ok":true}));
                    self.discard_on_quit = true;
                    self.begin_shutdown(window, cx);
                }
                _ => reply(json!({"id":request.id,"ok":false,"error":"Unknown driver command."})),
            }
        }
    }

    fn driver_state(&self, window: &Window, cx: &gpui::App) -> Value {
        let terminal = self.shell.as_ref().map(|tab| &tab.view.read(cx).snapshot);
        let editing = self.settings.viewer_mode == ViewerMode::File;
        let columns: serde_json::Map<String, Value> = Column::ALL
            .into_iter()
            .map(|column| {
                let state = self.settings.columns.get(column);
                (
                    column.key().to_owned(),
                    json!({"width": state.width, "collapsed": state.collapsed}),
                )
            })
            .collect();
        json!({"project_root":self.project.as_ref().map(|project| project.root().to_string_lossy()),
            "selected_file":self.settings.selected_file.as_ref().map(|path| path.to_string_lossy().replace('\\',"/")),
            "viewer_mode":self.settings.viewer_mode.key(),
            "markdown_preview":self.settings.markdown_preview,
            "previewing":self.previewing_markdown(),
            "preview_text":self.previewing_markdown().then(|| self.editor.as_ref().map(|editor| super::markdown::plain_text(&super::markdown::parse(editor.read(cx).buffer().text())))).flatten(),
            "columns":columns,
            "column_order":self.settings.columns.order.map(Column::key),
            "theme":self.settings.appearance.theme.key(),
            "text_size":self.settings.appearance.text_size,
            "interface_font":self.settings.appearance.interface_font,
            "code_font":self.settings.appearance.code_font,
            "font_count":self.font_names.as_ref().map(Vec::len),
            "settings_page":self.settings.settings_section().map(|section| section.key()),
            "page":self.settings.page.map(|page| match page { Page::Welcome => "welcome", Page::Settings(_) => "settings" }),
            "recent_projects":self.settings.recent_projects,
            "git":self.git.as_ref().map(|git| json!({"branch":git.branch,"files":git.files.iter().map(|(path,status)|(path.to_string_lossy().replace('\\',"/"),status.letter())).collect::<std::collections::BTreeMap<_,_>>(),"ignored":git.ignored.iter().map(|path|path.to_string_lossy().replace('\\',"/")).collect::<Vec<_>>()})),
            "git_failed":self.git_failed,
            "update":{"status":self.update_status_text(),"checking":self.update.checking,"banner":self.update.banner_visible(),
                "offered":self.update.status.as_ref().and_then(crate::update::check::UpdateStatus::offered).map(ToString::to_string),
                "downloading":self.update.downloading.as_ref().map(|downloading|downloading.read),
                "installed":self.update.installed.as_ref().map(|(version,_)|version.to_string()),
                "check_automatically":self.settings.updates.check_automatically,
                "skipped_version":self.settings.updates.skipped_version},
            "accounts":self.accounts.as_ref().map(|accounts| accounts.iter().map(|account| json!({"id":provider_id(account.provider),"installed":account.cli.is_some(),"sign_in":account.sign_in})).collect::<Vec<_>>()),
            "accounts_checking":self.accounts_checking,
            "window_width":f32::from(window.viewport_size().width),
            "terminal":{"running":terminal.is_some_and(|value|value.running),"output":terminal.map(|value|value.text.as_str()).unwrap_or("")},
            "agents":self.providers.iter().map(|(provider,status)|json!({"id":provider_id(*provider),"available":status.available})).collect::<Vec<_>>(),
            "agent_sessions":self.agents.iter().map(|tab|&tab.view.read(cx).snapshot).collect::<Vec<_>>(),
            "active_agent":self.settings.active_session,
            "viewer_text":if editing {self.editor.as_ref().map(|editor|editor.read(cx).buffer().text()).unwrap_or(&self.content)}else{&self.content},
            "editor_text":self.editor.as_ref().map(|editor|editor.read(cx).buffer().text()),
            "editor_dirty":self.editor.as_ref().is_some_and(|editor|editor.read(cx).dirty()),
            "editor_conflict":self.editor.as_ref().is_some_and(|editor|editor.read(cx).conflict),
            "controls":self.controls.borrow().values().collect::<Vec<_>>(),"error":self.error})
    }
}
