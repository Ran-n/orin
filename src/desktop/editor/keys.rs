// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:39:09.953498
//+ Revised: 	2026/11/13 07:53:53.098771
// -----------------------------------------------------------------------------

//! Text navigation, clipboard, history, and explicit save/submit actions.

use super::{EditorAction, TextEditor};
use gpui::{Context, KeyDownEvent, MouseDownEvent, MouseMoveEvent, Window};
use unicode_segmentation::UnicodeSegmentation;

impl TextEditor {
    pub fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if (modifiers.control || modifiers.platform) && self.shortcut(key, modifiers.shift, cx) {
            cx.stop_propagation();
            return;
        }
        let selection = self.buffer().selection();
        let text = self.buffer().text();
        let cursor = if self.selection_reversed {
            selection.start
        } else {
            selection.end
        };
        if matches!(key, "left" | "right" | "home" | "end" | "up" | "down") {
            let position = match key {
                "left" if !modifiers.shift && !selection.is_empty() => selection.start,
                "right" if !modifiers.shift && !selection.is_empty() => selection.end,
                "left" => previous(text, cursor),
                "right" => next(text, cursor),
                "home" => text[..cursor].rfind('\n').map_or(0, |index| index + 1),
                "end" => {
                    let end = text[cursor..]
                        .find('\n')
                        .map_or(text.len(), |index| cursor + index);
                    if end > 0 && text.as_bytes()[end - 1] == b'\r' {
                        end - 1
                    } else {
                        end
                    }
                }
                _ => vertical(text, cursor, key == "down"),
            };
            self.move_cursor(position, modifiers.shift, cx);
            cx.stop_propagation();
            return;
        }
        match key {
            "enter" if self.document().is_none() && !modifiers.shift => {
                cx.emit(EditorAction::Submit)
            }
            "enter" => {
                let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
                self.replace(selection, newline, cx);
            }
            "tab" => self.replace(selection, "    ", cx),
            // A selection is deleted whole, whichever end the caret is at.
            "backspace" | "delete" if !selection.is_empty() => self.replace(selection, "", cx),
            "backspace" => self.replace(previous(text, cursor)..cursor, "", cx),
            "delete" => self.replace(cursor..next(text, cursor), "", cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn move_cursor(&mut self, position: usize, selecting: bool, cx: &mut Context<Self>) {
        let range = self.buffer().selection();
        if selecting {
            if range.is_empty() {
                self.anchor = range.start;
            }
            let reversed = position < self.anchor;
            self.select(position.min(self.anchor)..position.max(self.anchor), cx);
            self.selection_reversed = reversed;
        } else {
            self.anchor = position;
            self.select(position..position, cx);
        }
    }

    fn shortcut(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        let range = self.buffer().selection();
        match key {
            "a" => self.select(0..self.buffer().text().len(), cx),
            "c" | "x" => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                    self.buffer().text()[range.clone()].into(),
                ));
                if key == "x" {
                    self.replace(range, "", cx);
                }
            }
            // Ctrl+Shift+V is left to the workspace, which toggles the Markdown preview.
            "v" if !shift => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.replace(range, &text, cx);
                }
            }
            "z" if shift => self.redo(cx),
            "z" => self.undo(cx),
            "y" => self.redo(cx),
            "s" if self.document().is_some() => cx.emit(EditorAction::Save),
            _ => return false,
        }
        true
    }

    pub fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus);
        let position = self.offset_at(event.position);
        self.anchor = position;
        self.dragging = true;
        self.select(position..position, cx);
    }
    pub fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.dragging {
            let position = self.offset_at(event.position);
            self.select(self.anchor.min(position)..self.anchor.max(position), cx);
            self.selection_reversed = position < self.anchor;
        }
    }
}

fn previous(text: &str, cursor: usize) -> usize {
    text[..cursor]
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(index, _)| index)
}
fn next(text: &str, cursor: usize) -> usize {
    text[cursor..]
        .grapheme_indices(true)
        .nth(1)
        .map_or(text.len(), |(index, _)| cursor + index)
}
fn vertical(text: &str, cursor: usize, down: bool) -> usize {
    let start = text[..cursor].rfind('\n').map_or(0, |index| index + 1);
    let column = text[start..cursor].graphemes(true).count();
    let destination = if down {
        text[cursor..].find('\n').map(|index| cursor + index + 1)
    } else if start > 0 {
        Some(text[..start - 1].rfind('\n').map_or(0, |index| index + 1))
    } else {
        None
    };
    let Some(start) = destination else {
        return cursor;
    };
    let mut end = text[start..]
        .find('\n')
        .map_or(text.len(), |index| start + index);
    // A CRLF line ends before its CR, so a long column never lands between CR and LF.
    if end > start && text.as_bytes()[end - 1] == b'\r' {
        end -= 1;
    }
    start
        + text[start..end]
            .grapheme_indices(true)
            .nth(column)
            .map_or(end - start, |(index, _)| index)
}
