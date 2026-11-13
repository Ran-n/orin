// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:17.204211
//+ Revised: 	2026/11/12 09:16:32.430017
// -----------------------------------------------------------------------------

//! Unicode-safe edit buffers and explicit document save checkpoints.

mod buffer;
mod document;
mod history;
mod offsets;

pub use buffer::EditorBuffer;
pub use document::EditorDocument;
pub use offsets::{byte_to_utf16, utf16_to_byte};

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditorError {
    #[error("The selection does not lie on UTF-8 character boundaries")]
    InvalidRange,
    #[error("The edited document exceeds the 1 MiB editing limit")]
    TooLarge,
    #[error("The save acknowledgement belongs to a different document")]
    WrongDocument,
}
