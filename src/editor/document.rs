// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:17.796174
//+ Revised: 	2026/11/12 09:16:31.732806
// -----------------------------------------------------------------------------

//! Saved checkpoints stay separate from edits made while a background save runs.

use super::{EditorBuffer, EditorError};
use crate::project::FileContent;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct EditorDocument {
    original: FileContent,
    buffer: EditorBuffer,
}

impl EditorDocument {
    pub fn from_file(file: FileContent) -> Self {
        Self {
            buffer: EditorBuffer::new(file.text.clone()),
            original: file,
        }
    }

    pub fn path(&self) -> &Path {
        &self.original.relative_path
    }
    pub fn original(&self) -> &FileContent {
        &self.original
    }
    pub fn buffer(&self) -> &EditorBuffer {
        &self.buffer
    }
    pub fn buffer_mut(&mut self) -> &mut EditorBuffer {
        &mut self.buffer
    }
    pub fn is_dirty(&self) -> bool {
        self.buffer.text() != self.original.text
    }

    pub fn accept_saved(&mut self, saved: FileContent) -> Result<(), EditorError> {
        if saved.relative_path != self.original.relative_path {
            return Err(EditorError::WrongDocument);
        }
        self.original = saved;
        self.buffer.break_undo_group();
        Ok(())
    }
}
