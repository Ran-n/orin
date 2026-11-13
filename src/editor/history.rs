// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:17.630377
//+ Revised: 	2026/11/12 15:11:30.644835
// -----------------------------------------------------------------------------

//! Delta history with word-sized typing groups and bounded retained undo memory.

use std::{collections::VecDeque, ops::Range};

const MAX_HISTORY_BYTES: usize = 8 * 1024 * 1024;
const MAX_HISTORY_EDITS: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    Word,
    Space,
}

#[derive(Clone, Debug)]
pub(super) struct Edit {
    pub start: usize,
    pub deleted: String,
    pub inserted: String,
    pub before_selection: Range<usize>,
    pub after_selection: Range<usize>,
    group: Option<Group>,
}

impl Edit {
    pub fn new(
        start: usize,
        deleted: String,
        inserted: String,
        before_selection: Range<usize>,
        after_selection: Range<usize>,
    ) -> Self {
        let group = typing_group(&deleted, &inserted);
        Self {
            start,
            deleted,
            inserted,
            before_selection,
            after_selection,
            group,
        }
    }

    fn can_merge(&self, next: &Self) -> bool {
        self.group.is_some()
            && self.group == next.group
            && self.start + self.inserted.len() == next.start
            && self.after_selection == next.before_selection
    }

    fn bytes(&self) -> usize {
        self.deleted.len() + self.inserted.len()
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct History {
    pub undo: VecDeque<Edit>,
    pub redo: Vec<Edit>,
    pub break_group: bool,
    bytes: usize,
}

impl History {
    pub fn push(&mut self, edit: Edit) {
        self.bytes -= self.redo.iter().map(Edit::bytes).sum::<usize>();
        self.redo.clear();
        self.bytes += edit.bytes();
        if !self.break_group
            && let Some(previous) = self.undo.back_mut()
            && previous.can_merge(&edit)
        {
            previous.inserted.push_str(&edit.inserted);
            previous.after_selection = edit.after_selection;
        } else {
            self.undo.push_back(edit);
        }
        self.break_group = false;
        while (self.bytes > MAX_HISTORY_BYTES || self.undo.len() > MAX_HISTORY_EDITS)
            && self.undo.len() > 1
        {
            if let Some(oldest) = self.undo.pop_front() {
                self.bytes -= oldest.bytes();
            }
        }
    }
}

fn typing_group(deleted: &str, inserted: &str) -> Option<Group> {
    if !deleted.is_empty() || inserted.chars().count() != 1 {
        return None;
    }
    let character = inserted.chars().next()?;
    if character.is_alphanumeric() || character == '_' {
        Some(Group::Word)
    } else if character == ' ' || character == '\t' {
        Some(Group::Space)
    } else {
        None
    }
}
