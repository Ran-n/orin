// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:39:09.448204
//+ Revised: 	2026/11/13 07:53:53.938409
// -----------------------------------------------------------------------------

//! A native multiline editor reused by files and agent prompt drafts.

mod input;
mod keys;
mod paint;
#[cfg(all(test, feature = "test-support"))]
mod tests;

use super::{
    controls::{self, Controls},
    theme,
};
use crate::{
    editor::{EditorBuffer, EditorDocument},
    project::FileContent,
};
use gpui::{
    Bounds, Context, EventEmitter, FocusHandle, MouseButton, Pixels, Window, div, prelude::*, px,
    rgb,
};
use std::ops::Range;

pub enum TextValue {
    Document(Box<EditorDocument>),
    Prompt(EditorBuffer),
}

#[derive(Clone)]
pub enum EditorAction {
    Save,
    Submit,
}

pub struct TextEditor {
    pub value: TextValue,
    pub focus: FocusHandle,
    pub saving: bool,
    pub conflict: bool,
    pub error: Option<String>,
    pub marked: Option<Range<usize>>,
    pub bounds: Option<Bounds<Pixels>>,
    pub lines: Vec<paint::EditorLine>,
    pub id: String,
    pub controls: Controls,
    pub anchor: usize,
    pub dragging: bool,
    pub selection_reversed: bool,
    /// Colours code by language; `None` for prompts and unknown file types.
    pub highlighter: Option<super::syntax::Highlighter>,
}

impl TextEditor {
    pub fn file(file: FileContent, controls: Controls, cx: &mut Context<Self>) -> Self {
        let highlighter = super::syntax::Highlighter::for_path(&file.relative_path);
        let mut editor = Self::new(
            TextValue::Document(Box::new(EditorDocument::from_file(file))),
            "editor-input",
            controls,
            cx,
        );
        editor.highlighter = highlighter;
        editor
    }
    pub fn prompt(id: &str, controls: Controls, cx: &mut Context<Self>) -> Self {
        Self::new(
            TextValue::Prompt(EditorBuffer::new(String::new())),
            id,
            controls,
            cx,
        )
    }
    fn new(value: TextValue, id: &str, controls: Controls, cx: &mut Context<Self>) -> Self {
        Self {
            value,
            focus: cx.focus_handle(),
            saving: false,
            conflict: false,
            error: None,
            marked: None,
            bounds: None,
            lines: vec![],
            id: id.into(),
            controls,
            anchor: 0,
            dragging: false,
            selection_reversed: false,
            highlighter: None,
        }
    }
    pub fn buffer(&self) -> &EditorBuffer {
        match &self.value {
            TextValue::Document(document) => document.buffer(),
            TextValue::Prompt(buffer) => buffer,
        }
    }
    pub fn buffer_mut(&mut self) -> &mut EditorBuffer {
        match &mut self.value {
            TextValue::Document(document) => document.buffer_mut(),
            TextValue::Prompt(buffer) => buffer,
        }
    }
    pub fn document(&self) -> Option<&EditorDocument> {
        match &self.value {
            TextValue::Document(document) => Some(document),
            _ => None,
        }
    }
    pub fn dirty(&self) -> bool {
        self.document().is_some_and(EditorDocument::is_dirty)
    }
    pub fn undo(&mut self, cx: &mut Context<Self>) {
        self.buffer_mut().undo();
        self.marked = None;
        cx.notify();
    }
    pub fn redo(&mut self, cx: &mut Context<Self>) {
        self.buffer_mut().redo();
        self.marked = None;
        cx.notify();
    }
    /// Apply an edit from typing or IME input. A refused edit is already logged and shown in
    /// the editor by `try_replace`, so input handlers have nothing more to do with the error.
    pub fn replace(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        if self.try_replace(range, text, cx).is_err() {
            tracing::debug!(target: "orin::editor", "input edit refused and reported");
        }
    }
    pub fn try_replace(
        &mut self,
        range: Range<usize>,
        text: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), crate::editor::EditorError> {
        if let Err(error) = self.buffer_mut().replace(range, text) {
            tracing::warn!(%error, "Text edit refused");
            self.error = Some(
                "The edit could not be applied. Text must stay within the 1 MiB limit.".into(),
            );
            cx.notify();
            return Err(error);
        }
        self.selection_reversed = false;
        self.error = None;
        cx.notify();
        Ok(())
    }
    pub fn select(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        if let Err(error) = self.buffer_mut().set_selection(range) {
            tracing::warn!(%error, "Text selection refused");
        }
        self.selection_reversed = false;
        cx.notify();
    }
    pub fn accept_saved(&mut self, file: FileContent, cx: &mut Context<Self>) {
        self.saving = false;
        if let TextValue::Document(document) = &mut self.value
            && let Err(error) = document.accept_saved(file)
        {
            tracing::error!(%error, "Save acknowledgement refused");
            self.error = Some(
                "The saved file did not match the open document. Reload it before editing further."
                    .into(),
            );
        }
        self.conflict = false;
        cx.notify();
    }
}

impl EventEmitter<EditorAction> for TextEditor {}

impl Render for TextEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text = self.buffer().text();
        let height = (text.lines().count().max(1) as f32 * theme::metric(theme::CODE_LINE)
            + theme::metric(48.))
        .max(theme::metric(110.));
        let width = (text
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0) as f32
            * theme::metric(8.)
            + theme::metric(80.))
        .max(200.);
        let entity = cx.entity();
        theme::column()
            .id(gpui::SharedString::from(self.id.clone()))
            .relative()
            .size_full()
            .min_h_0()
            .bg(rgb(theme::colors().background))
            .font_family(theme::code_font())
            .text_size(theme::scaled(theme::CODE_TEXT))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, _, _| view.dragging = false),
            )
            .child(
                div()
                    .id(gpui::SharedString::from(format!("{}-scroll", self.id)))
                    .size_full()
                    .overflow_scroll()
                    .child(
                        paint::text_canvas(entity)
                            .w(px(width))
                            .min_w_full()
                            .h(px(height))
                            .min_h_full(),
                    ),
            )
            .child(controls::measured(self.id.clone(), true, &self.controls))
            .children(self.error.clone().map(|error| {
                div()
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
