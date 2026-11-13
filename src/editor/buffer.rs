// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:17.467878
//+ Revised: 	2026/11/12 15:11:30.721151
// -----------------------------------------------------------------------------

//! Text mutation never permits splitting a Unicode scalar value.

use super::{
    EditorError,
    history::{Edit, History},
};
use crate::project::MAX_FILE_BYTES;
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct EditorBuffer {
    text: String,
    selection: Range<usize>,
    history: History,
    revision: u64,
}

impl EditorBuffer {
    pub fn new(text: String) -> Self {
        Self {
            text,
            selection: 0..0,
            history: History::default(),
            revision: 0,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn can_undo(&self) -> bool {
        !self.history.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.history.redo.is_empty()
    }
    pub fn break_undo_group(&mut self) {
        self.history.break_group = true;
    }

    pub fn set_selection(&mut self, selection: Range<usize>) -> Result<(), EditorError> {
        self.validate_range(&selection)?;
        self.selection = selection;
        self.break_undo_group();
        Ok(())
    }

    pub fn replace_selection(&mut self, replacement: &str) -> Result<(), EditorError> {
        self.replace(self.selection(), replacement)
    }

    pub fn replace(&mut self, range: Range<usize>, replacement: &str) -> Result<(), EditorError> {
        self.validate_range(&range)?;
        if self.text.len() - range.len() + replacement.len() > MAX_FILE_BYTES {
            return Err(EditorError::TooLarge);
        }
        if self.text[range.clone()] == *replacement {
            return Ok(());
        }
        let cursor = range.start + replacement.len();
        let edit = Edit::new(
            range.start,
            self.text[range.clone()].into(),
            replacement.into(),
            self.selection(),
            cursor..cursor,
        );
        self.text.replace_range(range, replacement);
        self.selection = cursor..cursor;
        self.history.push(edit);
        self.revision += 1;
        tracing::debug!(target: "orin::editor", revision = self.revision, "document edit applied");
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.history.undo.pop_back() else {
            return false;
        };
        self.text
            .replace_range(edit.start..edit.start + edit.inserted.len(), &edit.deleted);
        self.selection = edit.before_selection.clone();
        self.history.redo.push(edit);
        self.break_undo_group();
        self.revision += 1;
        tracing::debug!(target: "orin::editor", revision = self.revision, "document edit undone");
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.history.redo.pop() else {
            return false;
        };
        self.text
            .replace_range(edit.start..edit.start + edit.deleted.len(), &edit.inserted);
        self.selection = edit.after_selection.clone();
        self.history.undo.push_back(edit);
        self.break_undo_group();
        self.revision += 1;
        tracing::debug!(target: "orin::editor", revision = self.revision, "document edit redone");
        true
    }

    fn validate_range(&self, range: &Range<usize>) -> Result<(), EditorError> {
        if range.start > range.end
            || range.end > self.text.len()
            || !self.text.is_char_boundary(range.start)
            || !self.text.is_char_boundary(range.end)
        {
            return Err(EditorError::InvalidRange);
        }
        Ok(())
    }
}
