// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:43:07.123133
//+ Revised: 	2026/11/12 09:16:40.919605
// -----------------------------------------------------------------------------

//! Replies required by interactive terminals, including ConPTY cursor inheritance.

/// A query the shell sent that the terminal must answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Query {
    /// `ESC[6n`: where is the cursor?
    CursorPosition,
    /// `ESC[5n`: are you working?
    Status,
    /// `ESC[c`: primary device attributes.
    Device,
    /// `ESC[>c`: secondary device attributes.
    SecondaryDevice,
}

impl Query {
    /// The reply, with `cursor` (row, column; zero-based) as the screen stands at the query.
    pub(super) fn reply(self, cursor: (u16, u16)) -> Vec<u8> {
        match self {
            Self::CursorPosition => format!("\x1b[{};{}R", cursor.0 + 1, cursor.1 + 1).into_bytes(),
            Self::Status => b"\x1b[0n".to_vec(),
            Self::Device => b"\x1b[?1;2c".to_vec(),
            Self::SecondaryDevice => b"\x1b[>0;1;0c".to_vec(),
        }
    }
}

/// Spots queries in the output byte by byte, so one split across reads is still answered.
#[derive(Default)]
pub(super) struct QueryResponder {
    tail: Vec<u8>,
}

impl QueryResponder {
    /// Feed one output byte; returns the query it completes, if any.
    pub(super) fn feed(&mut self, byte: u8) -> Option<Query> {
        self.tail.push(byte);
        if self.tail.len() > 8 {
            self.tail.remove(0);
        }
        let query = if self.tail.ends_with(b"\x1b[6n") {
            Query::CursorPosition
        } else if self.tail.ends_with(b"\x1b[5n") {
            Query::Status
        } else if self.tail.ends_with(b"\x1b[c") || self.tail.ends_with(b"\x1b[0c") {
            Query::Device
        } else if self.tail.ends_with(b"\x1b[>c") || self.tail.ends_with(b"\x1b[>0c") {
            Query::SecondaryDevice
        } else {
            return None;
        };
        self.tail.clear();
        Some(query)
    }
}

#[cfg(test)]
mod tests {
    use super::{Query, QueryResponder};

    fn replies(responder: &mut QueryResponder, bytes: &[u8], cursor: (u16, u16)) -> Vec<u8> {
        bytes
            .iter()
            .filter_map(|byte| responder.feed(*byte))
            .flat_map(|query| query.reply(cursor))
            .collect()
    }

    #[test]
    fn regression_conpty_cursor_inheritance_query_is_answered_across_chunks() {
        // ConPTY startup and ClosePseudoConsole used to hang waiting for this CPR.
        let mut responder = QueryResponder::default();
        assert!(replies(&mut responder, b"\x1b[", (2, 4)).is_empty());
        assert_eq!(replies(&mut responder, b"6n", (2, 4)), b"\x1b[3;5R");
    }

    #[test]
    fn status_and_device_queries_have_distinct_responses() {
        let mut responder = QueryResponder::default();
        assert_eq!(
            replies(&mut responder, b"\x1b[5n\x1b[c\x1b[>c", (0, 0)),
            b"\x1b[0n\x1b[?1;2c\x1b[>0;1;0c"
        );
        assert!(replies(&mut responder, "ordinary 智 output".as_bytes(), (0, 0)).is_empty());
    }

    #[test]
    fn unit_each_query_is_reported_where_it_ends() {
        let mut responder = QueryResponder::default();
        let ends: Vec<(usize, Query)> = b"ab\x1b[6ncd"
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| responder.feed(*byte).map(|query| (index, query)))
            .collect();
        assert_eq!(ends, [(5, Query::CursorPosition)]);
    }
}
