// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:39:09.619250
//+ Revised: 	2026/11/14 12:09:13.748467
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
        self.place_at(point).0
    }

    /// The text offset under `point` and the drawn row it is on.
    pub fn place_at(&self, point: Point<Pixels>) -> (usize, usize) {
        let end = self.buffer().text().len();
        let Some(bounds) = self.bounds else {
            return (end, 0);
        };
        let geometry = super::paint::Geometry::current(self.document().is_some());
        let row =
            ((f32::from(point.y - bounds.top()) - geometry.top) / geometry.line).max(0.) as usize;
        let x = (point.x - bounds.left() - gpui::px(geometry.left)).max(gpui::px(0.));
        (self.offset_at_row(row, x).unwrap_or(end), row)
    }

    /// The end of the selection that moves: where the caret is.
    pub fn caret(&self) -> usize {
        let selection = self.buffer().selection();
        if self.selection_reversed {
            selection.start
        } else {
            selection.end
        }
    }

    /// The line of text holding `cursor`, and `cursor` as an index into it.
    fn line_at(&self, cursor: usize) -> Option<(&super::paint::EditorLine, usize)> {
        let entry = self.lines.iter().rfind(|entry| entry.offset <= cursor)?;
        Some((entry, (cursor - entry.offset).min(entry.len)))
    }

    /// The drawn row of a wrapped prompt that `cursor` is on, and that row's text range.
    /// `None` when the text isn't wrapped (a file, or before the first layout).
    pub fn drawn_row(&self, cursor: usize) -> Option<(usize, Range<usize>)> {
        let (entry, index) = self.line_at(cursor)?;
        let (row, _) = entry.caret_place(index, self.row_start)?;
        let range = entry.row_ranges().swap_remove(row);
        Some((
            entry.row + row,
            entry.offset + range.start..entry.offset + range.end,
        ))
    }

    /// Whether `offset` is where drawn row `row` starts after a wrap: the one place a caret
    /// could also be drawn at the end of the row above.
    pub fn starts_row(&self, offset: usize, row: usize) -> bool {
        self.lines
            .iter()
            .find(|entry| row < entry.row + entry.rows)
            .filter(|entry| row > entry.row && entry.wrapped.is_some())
            .is_some_and(|entry| {
                let range = entry.row_ranges().swap_remove(row - entry.row);
                entry.offset + range.start == offset
            })
    }

    /// Scroll just far enough to show the caret when it moved since it was last shown, as
    /// typing, arrow keys and dragging expect. `true` when the view scrolled and must be drawn
    /// again.
    pub fn reveal_caret(&mut self) -> bool {
        let caret = self.caret();
        let viewport = self.scroll.bounds().size;
        // Before the first layout there is no viewport to bring the caret into.
        if self.shown_caret == Some(caret) || viewport.height <= gpui::px(0.) {
            return false;
        }
        let geometry = super::paint::Geometry::current(self.document().is_some());
        let Some((entry, index)) = self.line_at(caret) else {
            return false;
        };
        // A wrapped prompt never scrolls sideways; a file does, once its line is shaped.
        let (row, x) = match entry.caret_place(index, self.row_start) {
            Some((row, _)) => (entry.row + row, None),
            None => (
                entry.row,
                entry.shaped.as_ref().map(|line| line.x_for_index(index)),
            ),
        };
        let before = self.scroll.offset();
        let mut offset = before;
        // The row with the padding above and below it, so the last row shows the box's end.
        let top = gpui::px(row as f32 * geometry.line);
        let bottom = top + gpui::px(geometry.line + 2. * geometry.top);
        offset.y = reveal(offset.y, top, bottom, viewport.height);
        if let Some(x) = x {
            // From the gutter's edge at the first column to a margin past the caret.
            let right = x + gpui::px(geometry.left + CARET_MARGIN);
            offset.x = reveal(offset.x, x, right, viewport.width);
        }
        // A file's off-screen line isn't shaped: once scrolled to, it is, and the next layout
        // finishes the sideways part.
        if x.is_some() || entry.wrapped.is_some() || offset == before {
            self.shown_caret = Some(caret);
        }
        if offset == before {
            return false;
        }
        self.scroll.set_offset(offset);
        true
    }

    /// The text offset closest to `x` on drawn row `row`; `None` below the last row.
    pub fn offset_at_row(&self, row: usize, x: Pixels) -> Option<usize> {
        let entry = self
            .lines
            .iter()
            .find(|entry| row < entry.row + entry.rows)?;
        if let Some(wrapped) = &entry.wrapped {
            let line_height = gpui::px(super::paint::Geometry::current(false).line);
            // The middle of the row, so rounding never picks its neighbour.
            let within = gpui::point(x, line_height * ((row - entry.row) as f32 + 0.5));
            let (Ok(index) | Err(index)) = wrapped.closest_index_for_position(within, line_height);
            return Some(entry.offset + index);
        }
        Some(match &entry.shaped {
            Some(shaped) => entry.offset + shaped.closest_index_for_x(x),
            // A drag past the edge of the window: off-screen lines aren't shaped, so the
            // selection reaches the start or end of that line.
            None if x == gpui::px(0.) => entry.offset,
            None => entry.offset + entry.len,
        })
    }

    /// One drawn row up or down from `cursor`, keeping its horizontal place, in a wrapped
    /// prompt: the offset there and the row. `None` when the text isn't wrapped (a file, or
    /// before the first layout) or there is no row that way.
    pub fn row_above_or_below(&self, cursor: usize, down: bool) -> Option<(usize, usize)> {
        let (entry, index) = self.line_at(cursor)?;
        let (row, x) = entry.caret_place(index, self.row_start)?;
        let row = entry.row + row;
        let target = if down { row + 1 } else { row.checked_sub(1)? };
        Some((self.offset_at_row(target, x)?, target))
    }
}

/// Room kept between the caret and the right edge of a file, so the next characters show.
const CARET_MARGIN: f32 = 24.0;

/// The scroll offset (zero or negative) that shows `start..end` of the content in a viewport
/// `size` long, moving as little as possible from `offset`.
fn reveal(offset: Pixels, start: Pixels, end: Pixels, size: Pixels) -> Pixels {
    if start < -offset {
        -start
    } else if end > -offset + size {
        -(end - size)
    } else {
        offset
    }
}

#[cfg(test)]
mod tests {
    use super::reveal;
    use gpui::px;

    #[test]
    fn unit_reveal_scrolls_only_as_far_as_needed() {
        // A 100-long viewport showing 50..150 of the content.
        let offset = px(-50.);
        assert_eq!(
            reveal(offset, px(60.), px(80.), px(100.)),
            offset,
            "in view"
        );
        assert_eq!(
            reveal(offset, px(20.), px(40.), px(100.)),
            px(-20.),
            "above"
        );
        assert_eq!(
            reveal(offset, px(160.), px(180.), px(100.)),
            px(-80.),
            "below"
        );
        assert_eq!(
            reveal(px(0.), px(0.), px(20.), px(100.)),
            px(0.),
            "at the start"
        );
    }
}
