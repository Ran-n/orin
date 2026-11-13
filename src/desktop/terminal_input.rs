// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:29:21.582171
//+ Revised: 	2026/11/12 09:16:29.487364
// -----------------------------------------------------------------------------

//! Native key and IME handling; committed text goes directly to the active PTY.

use super::terminal_view::TerminalView;
use gpui::{
    Bounds, Context, EntityInputHandler, KeyDownEvent, Pixels, Point, UTF16Selection, Window,
};
use std::ops::Range;

impl TerminalView {
    pub fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke.key;
        let modifiers = event.keystroke.modifiers;
        let clipboard = modifiers.platform || (modifiers.control && modifiers.shift);
        if clipboard && key == "c" {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(self.snapshot.text.clone()));
            cx.stop_propagation();
            return;
        }
        if clipboard && key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.send(
                    &crate::keyboard::paste(&text, self.snapshot.bracketed_paste),
                    cx,
                );
            }
            cx.stop_propagation();
            return;
        }
        if modifiers.shift && (key == "pageup" || key == "pagedown") {
            let direction = if key == "pageup" { 12 } else { -12 };
            if let Ok(session) = self.session.try_lock()
                && let Err(error) = session.scroll(direction)
            {
                tracing::warn!(%error,"Terminal history failed");
            }
            cx.stop_propagation();
            return;
        }
        if let Some(bytes) = crate::keyboard::encode(
            key,
            modifiers.control,
            modifiers.alt,
            modifiers.shift,
            self.snapshot.application_cursor,
        ) {
            self.send(&bytes, cx);
            cx.stop_propagation();
        }
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        *adjusted = Some(0..self.composition.encode_utf16().count());
        Some(self.composition.clone())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self.composition.encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.composition.is_empty()).then(|| 0..self.composition.encode_utf16().count())
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.composition.clear();
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composition.clear();
        self.send(text.as_bytes(), cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composition = text.to_owned();
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}
