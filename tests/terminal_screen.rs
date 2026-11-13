// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:52.928424
//+ Revised: 	2026/11/12 09:16:54.114448
// -----------------------------------------------------------------------------

use orin::terminal::TerminalScreen;
use proptest::prelude::*;

#[test]
fn unit_screen_preserves_truecolor_styles_cursor_and_terminal_modes() {
    let mut screen = TerminalScreen::new(4, 20).unwrap();
    screen.process(
        b"\x1b[1;3;4;38;2;12;34;56;48;2;65;43;21mHello\x1b[0m\x1b[?1h\x1b[?2004h\x1b[?25l",
    );
    let snapshot = screen.snapshot();
    let span = &snapshot.rows[0].spans[0];
    assert_eq!(span.text, "Hello");
    assert_eq!(span.foreground, Some([12, 34, 56]));
    assert_eq!(span.background, Some([65, 43, 21]));
    assert!(span.bold && span.italic && span.underline);
    assert_eq!(snapshot.cursor, (0, 5));
    assert!(!snapshot.cursor_visible);
    assert!(snapshot.application_cursor && snapshot.bracketed_paste);
}

#[test]
fn unit_screen_handles_carriage_return_erase_cursor_movement_and_alt_screen() {
    let mut screen = TerminalScreen::new(4, 20).unwrap();
    screen.process(b"starting\rfinished\x1b[K\r\nready");
    assert_eq!(screen.snapshot().text, "finished\nready");
    screen.process(b"\x1b[?1049hfull-screen-agent");
    assert_eq!(screen.snapshot().text, "full-screen-agent");
    screen.process(b"\x1b[?1049l");
    assert_eq!(screen.snapshot().text, "finished\nready");
}

#[test]
fn unit_screen_retains_unicode_combining_wide_cells_and_blank_backgrounds() {
    let mut screen = TerminalScreen::new(4, 20).unwrap();
    screen.process("e\u{301}智ñ".as_bytes());
    let snapshot = screen.snapshot();
    assert_eq!(snapshot.text, "e\u{301}智ñ");
    assert_eq!(snapshot.cursor, (0, 4));
    assert_eq!(snapshot.rows[0].spans[0].text.trim_end(), "e\u{301}智ñ");
    screen.process(b"\x1b[41m \x1b[0m");
    assert!(
        screen.snapshot().rows[0]
            .spans
            .iter()
            .any(|span| span.text == " " && span.background == Some([205, 0, 0]))
    );
}

#[test]
fn unit_screen_retains_history_and_clamps_scroll_offsets() {
    let mut screen = TerminalScreen::new(2, 20).unwrap();
    screen.process(b"first\r\nsecond\r\nthird");
    assert_eq!(screen.snapshot().text, "second\nthird");
    screen.scroll(1);
    assert_eq!(screen.snapshot().text, "first\nsecond");
    assert_eq!(screen.snapshot().scrollback, 1);
    screen.scroll(i32::MAX);
    assert_eq!(screen.snapshot().scrollback, 1);
    screen.scroll(i32::MIN);
    assert_eq!(screen.snapshot().scrollback, 0);
}

#[test]
fn unit_all_indexed_colors_and_reverse_video_are_preserved() {
    let mut screen = TerminalScreen::new(4, 20).unwrap();
    for index in 0..=255 {
        screen.process(format!("\x1b[H\x1b[38;5;{index};7mX").as_bytes());
        let span = screen.snapshot().rows[0].spans[0].clone();
        assert!(span.foreground.is_some());
        assert!(span.inverse);
    }
    screen.process(b"\x1b[H\x1b[38;5;232mX");
    assert_eq!(
        screen.snapshot().rows[0].spans[0].foreground,
        Some([8, 8, 8])
    );
    screen.process(b"\x1b[H\x1b[38;5;231mX");
    assert_eq!(
        screen.snapshot().rows[0].spans[0].foreground,
        Some([255, 255, 255])
    );
}

#[test]
fn unit_invalid_sizes_are_rejected_without_damaging_existing_grid() {
    let mut screen = TerminalScreen::new(4, 20).unwrap();
    screen.process(b"keep me");
    assert!(screen.resize(0, 20).is_err());
    assert!(TerminalScreen::new(4, 0).is_err());
    assert!(TerminalScreen::new(513, 20).is_err());
    assert!(TerminalScreen::new(4, 1_025).is_err());
    assert_eq!(screen.snapshot().text, "keep me");
}

proptest! {
    #[test]
    fn property_stream_chunking_cannot_change_terminal_state(bytes in prop::collection::vec(any::<u8>(), 0..512), split in 0usize..512) {
        let mut whole = TerminalScreen::new(12, 40).unwrap();
        let mut chunks = TerminalScreen::new(12, 40).unwrap();
        whole.process(&bytes);
        let split = split.min(bytes.len());
        chunks.process(&bytes[..split]); chunks.process(&bytes[split..]);
        prop_assert_eq!(whole.snapshot(), chunks.snapshot());
    }

    #[test]
    fn property_grid_dimensions_survive_arbitrary_output_and_resizing(rows in 1u16..30, cols in 2u16..80, bytes in prop::collection::vec(any::<u8>(), 0..256)) {
        let mut screen = TerminalScreen::new(4, 20).unwrap();
        screen.process(&bytes);
        screen.resize(rows, cols).unwrap();
        let snapshot = screen.snapshot();
        prop_assert_eq!(snapshot.rows.len(), usize::from(rows));
        prop_assert!(snapshot.cursor.0 < rows);
        prop_assert!(snapshot.cursor.1 <= cols);
        prop_assert!(snapshot.rows.iter().all(|row| !row.spans.is_empty()));
    }
}
