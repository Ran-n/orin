// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:29:21.390733
//+ Revised: 	2026/11/12 09:16:29.957553
// -----------------------------------------------------------------------------

//! ANSI terminal painting backed by a supervised PTY session.

use super::{
    controls::{self, Controls},
    theme,
};
use crate::terminal::{Session, TerminalSnapshot};
use gpui::{
    Bounds, Context, ElementInputHandler, Entity, FocusHandle, MouseButton, TextRun, Window,
    canvas, fill, point, prelude::*, px, rgb, rgba, size,
};
use std::sync::{Arc, Mutex};

pub struct TerminalView {
    pub session: Arc<Mutex<Session>>,
    pub snapshot: TerminalSnapshot,
    pub focus: FocusHandle,
    pub bounds: Option<Bounds<gpui::Pixels>>,
    pub composition: String,
    pub error: Option<String>,
    pub controls: Controls,
    pub id: String,
    dimensions: (u16, u16),
}

impl TerminalView {
    pub fn new(
        session: Arc<Mutex<Session>>,
        controls: Controls,
        id: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let snapshot = session
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .snapshot();
        Self {
            session,
            snapshot,
            focus: cx.focus_handle(),
            bounds: None,
            composition: String::new(),
            error: None,
            controls,
            id,
            dimensions: (24, 80),
        }
    }

    pub fn running(&self) -> bool {
        self.snapshot.running
    }

    pub fn poll(&mut self, cx: &mut Context<Self>) {
        let Ok(session) = self.session.try_lock() else {
            return;
        };
        let snapshot = session.snapshot();
        drop(session);
        if snapshot != self.snapshot {
            self.snapshot = snapshot;
            cx.notify();
        }
    }

    pub fn send(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        let result = self
            .session
            .try_lock()
            .map_err(|_| anyhow::anyhow!("Session is stopping"))
            .and_then(|session| session.write(bytes));
        if let Err(error) = result {
            tracing::warn!(%error, "Terminal input failed");
            self.error = Some(
                "Input could not be sent. The session may have ended or still be catching up."
                    .into(),
            );
            cx.notify();
        }
    }

    pub fn terminal_canvas(entity: Entity<Self>) -> impl IntoElement {
        canvas(
            move |bounds, window, cx| {
                entity.update(cx, |view, cx| {
                    view.bounds = Some(bounds);
                    let columns = (f32::from(bounds.size.width)
                        / theme::metric(theme::TERMINAL_CELL))
                    .floor()
                    .clamp(2., 500.) as u16;
                    let rows = (f32::from(bounds.size.height) / theme::metric(theme::TERMINAL_LINE))
                        .floor()
                        .clamp(1., 200.) as u16;
                    if view.dimensions != (rows, columns)
                        && let Ok(session) = view.session.try_lock()
                    {
                        match session.resize(rows, columns) {
                            Ok(()) => view.dimensions = (rows, columns),
                            Err(error) => tracing::warn!(%error,"Terminal resize failed"),
                        }
                    }
                    let _ = (window, cx);
                });
                entity
            },
            move |bounds, entity, window, cx| {
                let input = entity.clone();
                entity.update(cx, |view, cx| {
                    window.handle_input(&view.focus, ElementInputHandler::new(bounds, input), cx);
                    paint_screen(view, bounds, window, cx);
                });
            },
        )
        .size_full()
    }
}

fn paint_screen(
    view: &TerminalView,
    bounds: Bounds<gpui::Pixels>,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let style = window.text_style();
    let line_height = theme::metric(theme::TERMINAL_LINE);
    for (index, row) in view.snapshot.rows.iter().enumerate() {
        if index as f32 * line_height >= f32::from(bounds.size.height) {
            break;
        }
        let mut text = String::new();
        let mut runs = Vec::new();
        for span in &row.spans {
            text.push_str(&span.text);
            let mut font = style.font();
            if span.bold {
                font.weight = gpui::FontWeight::BOLD;
            }
            let foreground = span
                .foreground
                .map(color)
                .unwrap_or_else(|| rgb(theme::colors().text));
            let background = span.background.map(color);
            runs.push(TextRun {
                len: span.text.len(),
                font,
                color: foreground.into(),
                background_color: background.map(Into::into),
                underline: None,
                strikethrough: None,
            });
        }
        let line = window.text_system().shape_line(
            text.into(),
            theme::scaled(theme::CODE_TEXT),
            &runs,
            None,
        );
        if let Err(error) = line.paint(
            point(bounds.left(), bounds.top() + px(index as f32 * line_height)),
            px(line_height),
            window,
            cx,
        ) {
            tracing::warn!(%error, "Terminal line could not be painted");
        }
    }
    paint_cursor(view, bounds, window);
}

fn color(value: [u8; 3]) -> gpui::Rgba {
    rgb(u32::from_be_bytes([0, value[0], value[1], value[2]]))
}

fn paint_cursor(view: &TerminalView, bounds: Bounds<gpui::Pixels>, window: &mut Window) {
    if !view.snapshot.running
        || !view.snapshot.cursor_visible
        || !view.focus.is_focused(window)
        || view.snapshot.scrollback > 0
    {
        return;
    }
    let (row, column) = view.snapshot.cursor;
    let (cell, line) = (
        theme::metric(theme::TERMINAL_CELL),
        theme::metric(theme::TERMINAL_LINE),
    );
    let origin = point(
        bounds.left() + px(f32::from(column) * cell),
        bounds.top() + px(f32::from(row) * line),
    );
    window.paint_quad(fill(
        Bounds::new(origin, size(px(cell), px(line))),
        rgba(theme::colors().cursor),
    ));
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        theme::column()
            .id(gpui::SharedString::from(self.id.clone()))
            .relative()
            .size_full()
            .bg(rgb(theme::colors().background))
            .font_family(theme::code_font())
            .text_size(theme::scaled(theme::CODE_TEXT))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(MouseButton::Left, move |_, window, _| window.focus(&focus))
            .child(Self::terminal_canvas(cx.entity()))
            .child(controls::measured(
                self.id.clone(),
                self.running(),
                &self.controls,
            ))
            .children(self.error.clone().map(|error| {
                gpui::div()
                    .absolute()
                    .bottom_0()
                    .w_full()
                    .bg(rgb(theme::colors().panel))
                    .p_2()
                    .text_color(rgb(theme::colors().error))
                    .child(error)
            }))
    }
}
