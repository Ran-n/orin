// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:39:09.619250
//+ Revised: 	2026/11/13 07:53:53.322262
// -----------------------------------------------------------------------------

//! Platform text services use UTF-16; file buffers use validated UTF-8 offsets.

use super::TextEditor;
use crate::editor::{byte_to_utf16, utf16_to_byte};
use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};
use std::ops::Range;

impl EntityInputHandler for TextEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.buffer().text();
        let start = utf16_to_byte(text, range.start);
        let end = utf16_to_byte(text, range.end).max(start);
        *adjusted = Some(byte_to_utf16(text, start)..byte_to_utf16(text, end));
        Some(text[start..end].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let range = self.buffer().selection();
        let text = self.buffer().text();
        Some(UTF16Selection {
            range: byte_to_utf16(text, range.start)..byte_to_utf16(text, range.end),
            reversed: self.selection_reversed,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let text = self.buffer().text();
        self.marked
            .clone()
            .map(|range| byte_to_utf16(text, range.start)..byte_to_utf16(text, range.end))
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.input_range(range);
        if self.try_replace(range, text, cx).is_ok() {
            self.marked = None;
        }
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.input_range(range);
        let start = range.start;
        if self.try_replace(range, text, cx).is_err() {
            return;
        }
        self.marked = (!text.is_empty()).then_some(start..start + text.len());
        if let Some(selected) = selected {
            self.select(
                start + utf16_to_byte(text, selected.start)
                    ..start + utf16_to_byte(text, selected.end),
                cx,
            );
        }
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
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(byte_to_utf16(self.buffer().text(), self.offset_at(point)))
    }
}

impl TextEditor {
    fn input_range(&self, range: Option<Range<usize>>) -> Range<usize> {
        let text = self.buffer().text();
        range
            .map(|range| utf16_to_byte(text, range.start)..utf16_to_byte(text, range.end))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.buffer().selection())
    }
    pub fn offset_at(&self, point: Point<Pixels>) -> usize {
        let Some(bounds) = self.bounds else {
            return self.buffer().text().len();
        };
        let geometry = super::paint::Geometry::current(self.document().is_some());
        let line =
            ((f32::from(point.y - bounds.top()) - geometry.top) / geometry.line).max(0.) as usize;
        let x = (point.x - bounds.left() - gpui::px(geometry.left)).max(gpui::px(0.));
        self.lines
            .get(line)
            .map(|entry| match &entry.shaped {
                Some(shaped) => entry.offset + shaped.closest_index_for_x(x),
                // A drag past the edge of the window: off-screen lines aren't shaped, so the
                // selection reaches the start or end of that line.
                None if x == gpui::px(0.) => entry.offset,
                None => entry.offset + entry.len,
            })
            .unwrap_or(self.buffer().text().len())
    }
}
