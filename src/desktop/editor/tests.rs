// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:50:02.412850
//+ Revised: 	2026/11/14 12:09:14.480009
// -----------------------------------------------------------------------------

//! Headless native-widget contracts; OS pixels remain covered by rendered tests.

use super::{EditorAction, TextEditor};
use crate::project::{MAX_FILE_BYTES, Project};
use gpui::{
    Context, Entity, EntityInputHandler, Modifiers, MouseButton, TestAppContext, VisualTestContext,
    Window, point, px,
};
use std::{cell::RefCell, fs, path::Path, rc::Rc};
use tempfile::TempDir;

fn prompt(cx: &mut TestAppContext) -> (Entity<TextEditor>, &mut VisualTestContext) {
    let (editor, cx) =
        cx.add_window_view(|_, cx| TextEditor::prompt("test-prompt", Default::default(), cx));
    update(&editor, cx, |view, window, _| window.focus(&view.focus));
    cx.run_until_parked();
    (editor, cx)
}

fn update<R>(
    editor: &Entity<TextEditor>,
    cx: &mut VisualTestContext,
    action: impl FnOnce(&mut TextEditor, &mut Window, &mut Context<TextEditor>) -> R,
) -> R {
    cx.update(|window, cx| editor.update(cx, |view, cx| action(view, window, cx)))
}

fn file_editor<'a>(
    cx: &'a mut TestAppContext,
    text: &str,
) -> (
    TempDir,
    Project,
    Entity<TextEditor>,
    &'a mut VisualTestContext,
) {
    let directory = TempDir::new().unwrap();
    fs::write(directory.path().join("file.rs"), text).unwrap();
    let project = Project::open(directory.path()).unwrap();
    let file = project.read_file(Path::new("file.rs")).unwrap();
    let (editor, cx) = cx.add_window_view(|_, cx| TextEditor::file(file, Default::default(), cx));
    update(&editor, cx, |view, window, _| window.focus(&view.focus));
    cx.run_until_parked();
    (directory, project, editor, cx)
}

#[gpui::test]
fn prompt_renders_measured_native_input_and_accepts_keyboard_text(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    cx.simulate_input("Intention 🌱");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().text(), "Intention 🌱");
        assert!(view.document().is_none());
        assert!(!view.dirty());
        let controls = view.controls.borrow();
        let bounds = controls
            .get("test-prompt")
            .expect("native input measured during paint");
        assert!(bounds.enabled && bounds.width > 0. && bounds.height > 0.);
        assert!(view.bounds.is_some());
        assert!(!view.lines.is_empty());
    });
}

#[gpui::test]
fn utf16_replacement_and_adjusted_ranges_do_not_split_surrogates(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    update(&editor, cx, |view, window, cx| {
        view.replace_text_in_range(None, "a🌱éz", window, cx);
        let mut adjusted = None;
        assert_eq!(
            view.text_for_range(2..3, &mut adjusted, window, cx)
                .as_deref(),
            Some("🌱")
        );
        assert_eq!(adjusted, Some(1..3));
        view.replace_text_in_range(Some(1..3), "oak", window, cx);
        assert_eq!(view.buffer().text(), "aoakéz");
        assert_eq!(
            view.selected_text_range(false, window, cx).unwrap().range,
            4..4
        );
        assert_eq!(
            view.text_for_range(99..100, &mut adjusted, window, cx)
                .as_deref(),
            Some("")
        );
        assert_eq!(adjusted, Some(6..6));
    });
}

#[gpui::test]
fn ime_composition_replaces_marked_text_and_commits_without_losing_unicode(
    cx: &mut TestAppContext,
) {
    let (editor, cx) = prompt(cx);
    update(&editor, cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "🌱é", Some(2..3), window, cx);
        assert_eq!(view.marked_text_range(window, cx), Some(0..3));
        assert_eq!(
            view.selected_text_range(false, window, cx).unwrap().range,
            2..3
        );
        view.replace_and_mark_text_in_range(None, "oak", Some(3..3), window, cx);
        assert_eq!(view.buffer().text(), "oak");
        assert_eq!(view.marked_text_range(window, cx), Some(0..3));
        view.replace_text_in_range(None, "carballo", window, cx);
        assert_eq!(view.buffer().text(), "carballo");
        assert!(view.marked_text_range(window, cx).is_none());
        view.undo(cx);
        assert_eq!(view.buffer().text(), "oak");
        view.redo(cx);
        assert_eq!(view.buffer().text(), "carballo");
    });
}

#[gpui::test]
fn oversized_ime_update_preserves_original_marked_range_and_exposes_error(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    update(&editor, cx, |view, window, cx| {
        view.replace_and_mark_text_in_range(None, "é", Some(1..1), window, cx);
        let original_mark = view.marked.clone();
        let selection = view.buffer().selection();
        // Regression: a refused edit must not install an out-of-bounds composition range.
        view.replace_and_mark_text_in_range(
            None,
            &"x".repeat(MAX_FILE_BYTES + 1),
            Some(1..1),
            window,
            cx,
        );
        assert_eq!(view.buffer().text(), "é");
        assert_eq!(view.marked, original_mark);
        assert_eq!(view.buffer().selection(), selection);
        assert!(view.error.is_some());
        view.replace_text_in_range(None, &"x".repeat(MAX_FILE_BYTES + 1), window, cx);
        assert_eq!(view.buffer().text(), "é");
        assert_eq!(view.marked, original_mark);
        assert_eq!(view.buffer().selection(), selection);
        view.unmark_text(window, cx);
        assert!(view.marked_text_range(window, cx).is_none());
    });
}

#[gpui::test]
fn clipboard_paste_cut_undo_and_redo_use_native_shortcut_wiring(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    cx.update(|_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string("a🌱é".into())));
    cx.simulate_keystrokes("ctrl-v ctrl-a ctrl-x");
    update(&editor, cx, |view, _, _| {
        assert!(view.buffer().text().is_empty())
    });
    cx.simulate_keystrokes("ctrl-z");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().text(), "a🌱é")
    });
    cx.simulate_keystrokes("ctrl-shift-z");
    update(&editor, cx, |view, _, _| {
        assert!(view.buffer().text().is_empty())
    });
}

#[gpui::test]
fn shift_navigation_extends_and_contracts_selection_by_grapheme(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    cx.simulate_input("a🌱e\u{301}");
    cx.simulate_keystrokes("shift-left");
    update(&editor, cx, |view, window, cx| {
        assert_eq!(view.buffer().selection(), 5..8);
        assert!(
            view.selected_text_range(false, window, cx)
                .unwrap()
                .reversed
        );
    });
    cx.simulate_keystrokes("shift-left shift-right");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().selection(), 5..8)
    });
    cx.simulate_keystrokes("right");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().selection(), 8..8)
    });
}

#[gpui::test]
fn grapheme_deletion_and_line_navigation_keep_valid_cursor_offsets(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    cx.simulate_input("a🌱e\u{301}\nsecond");
    cx.simulate_keystrokes("home up end backspace");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().text(), "a🌱\nsecond");
        assert_eq!(view.buffer().selection(), 5..5);
    });
    cx.simulate_keystrokes("backspace");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().text(), "a\nsecond")
    });
}

#[gpui::test]
fn mouse_drag_uses_shaped_text_geometry_for_unicode_selection(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    cx.simulate_input("a🌱é");
    let (start, end) = update(&editor, cx, |view, _, _| {
        let bounds = view.bounds.unwrap();
        // A prompt's lines are wrapped; this short one is a single drawn row.
        let line = &view.lines[0].wrapped.as_ref().unwrap().unwrapped_layout;
        let y = bounds.top() + px(20.);
        (
            point(bounds.left() + px(12.) + line.x_for_index(1), y),
            point(bounds.left() + px(12.) + line.x_for_index(5), y),
        )
    });
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().selection(), 1..5);
        assert!(!view.dragging);
    });
}

#[gpui::test]
fn enter_submits_prompt_and_shift_enter_inserts_newline(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let _subscription = editor.update(cx, |_, cx| {
        cx.subscribe(&editor, move |_, _, action, _| {
            output.borrow_mut().push(action.clone())
        })
    });
    cx.simulate_input("intention");
    cx.simulate_keystrokes("enter shift-enter ctrl-s");
    assert_eq!(events.borrow().len(), 1);
    assert!(matches!(events.borrow()[0], EditorAction::Submit));
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().text(), "intention\n")
    });
}

#[gpui::test]
fn file_save_event_and_acknowledgement_keep_inflight_edits_dirty(cx: &mut TestAppContext) {
    let (_directory, project, editor, cx) = file_editor(cx, "before\r\n");
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let _subscription = editor.update(cx, |_, cx| {
        cx.subscribe(&editor, move |_, _, action, _| {
            output.borrow_mut().push(action.clone())
        })
    });
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("saved");
    cx.simulate_keystrokes("ctrl-s");
    assert!(matches!(events.borrow().as_slice(), [EditorAction::Save]));
    let saved = update(&editor, cx, |view, _, _| {
        project
            .save_file(view.document().unwrap().original(), view.buffer().text())
            .unwrap()
    });
    cx.simulate_input(" newer");
    update(&editor, cx, |view, _, cx| {
        view.saving = true;
        view.conflict = true;
        view.accept_saved(saved, cx);
        assert!(view.dirty());
        assert!(!view.saving && !view.conflict);
        assert_eq!(view.buffer().text(), "saved newer");
        assert_eq!(view.document().unwrap().original().text, "saved");
    });
}

#[gpui::test]
fn end_navigation_keeps_crlf_whole_before_grapheme_backspace(cx: &mut TestAppContext) {
    let (_directory, _project, editor, cx) = file_editor(cx, "a🌱\r\nsecond\r\n");
    // Regression: End must not place the caret between CR and LF.
    cx.simulate_keystrokes("end backspace");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().text(), "a\r\nsecond\r\n");
        assert_eq!(view.buffer().selection(), 1..1);
    });
}

#[gpui::test]
fn regression_backspace_and_delete_remove_a_selection_made_leftwards(cx: &mut TestAppContext) {
    // With the caret at the start of a selection, both keys used to delete nothing.
    let (editor, cx) = prompt(cx);
    cx.simulate_input("abcdef");
    cx.simulate_keystrokes("shift-left shift-left backspace");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().text(), "abcd");
        assert_eq!(view.buffer().selection(), 4..4);
    });
    cx.simulate_keystrokes("shift-left shift-left delete");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().text(), "ab");
        assert_eq!(view.buffer().selection(), 2..2);
    });
}

#[gpui::test]
fn regression_up_and_down_never_land_between_cr_and_lf(cx: &mut TestAppContext) {
    let (_directory, _project, editor, cx) = file_editor(cx, "ab\r\nlonger line\r\n");
    // From the end of the long line, Up lands at the end of "ab", before its CR.
    cx.simulate_keystrokes("down end up");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().selection(), 2..2);
    });
    cx.simulate_keystrokes("backspace");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().text(), "a\r\nlonger line\r\n");
    });
}

#[gpui::test]
fn unit_a_long_file_shapes_only_the_lines_on_screen(cx: &mut TestAppContext) {
    let text: String = (0..5_000).map(|line| format!("line {line}\n")).collect();
    let (_directory, _project, editor, cx) = file_editor(cx, &text);
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.lines.len(), 5_001, "every line keeps its offset");
        let shaped = view
            .lines
            .iter()
            .filter(|line| line.shaped.is_some())
            .count();
        assert!(shaped > 0 && shaped < 500, "{shaped} lines shaped");
        assert!(
            view.lines[0].shaped.is_some(),
            "the top of the file is on screen"
        );
        assert_eq!(view.lines[1].offset, "line 0\n".len());
    });
}

#[gpui::test]
fn prompt_wraps_long_lines_and_moves_and_clicks_by_drawn_row(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    // Far wider than any window: a prompt used to scroll sideways instead of wrapping.
    let text = "intention ".repeat(80).trim_end().to_owned();
    cx.simulate_input(&text);
    cx.run_until_parked();
    let rows = update(&editor, cx, |view, _, _| {
        assert_eq!(view.lines.len(), 1, "still one line of text");
        let line = &view.lines[0];
        assert!(line.rows > 1, "the long line is drawn on several rows");
        assert!(line.wrapped.is_some() && line.shaped.is_none());
        assert_eq!(view.visual_rows, line.rows);
        // The rows cover the line exactly, in order, with no gap or overlap.
        let ranges = line.row_ranges();
        assert_eq!(ranges.len(), line.rows);
        assert_eq!(ranges.first().unwrap().start, 0);
        assert_eq!(ranges.last().unwrap().end, text.len());
        assert!(ranges.windows(2).all(|pair| pair[0].end == pair[1].start));
        assert_eq!(view.buffer().selection(), text.len()..text.len());
        line.rows
    });

    // Up moves one drawn row within the same line, not to the start of the text.
    cx.simulate_keystrokes("up");
    let above = update(&editor, cx, |view, _, _| view.buffer().selection().end);
    assert!(above > 0 && above < text.len(), "up landed at {above}");
    cx.simulate_keystrokes("down");
    let below = update(&editor, cx, |view, _, _| view.buffer().selection().end);
    assert!(below > above, "down returned to a later row");
    // On the first and last rows there is nowhere further to go.
    for _ in 0..rows + 2 {
        cx.simulate_keystrokes("up");
    }
    let top = update(&editor, cx, |view, _, _| {
        let cursor = view.buffer().selection().end;
        assert!(view.lines[0].row_ranges()[0].contains(&cursor) || cursor == 0);
        cursor
    });
    cx.simulate_keystrokes("up");
    assert_eq!(
        update(&editor, cx, |view, _, _| view.buffer().selection().end),
        top
    );

    // A click on the second drawn row lands in that row's text.
    update(&editor, cx, |view, _, _| {
        let second = view.lines[0].row_ranges()[1].clone();
        let clicked = view.offset_at_row(1, px(1.)).unwrap();
        assert!(
            second.contains(&clicked) || clicked == second.start,
            "{clicked} is outside {second:?}"
        );
        assert_eq!(view.offset_at_row(0, px(0.)), Some(0));
        assert_eq!(view.offset_at_row(rows + 5, px(0.)), None, "below the text");
    });

    // A typed line break starts after the wrapped rows.
    update(&editor, cx, |view, _, cx| {
        let end = view.buffer().text().len();
        view.select(end..end, cx);
    });
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("next");
    cx.run_until_parked();
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.lines.len(), 2);
        assert_eq!(view.lines[1].row, view.lines[0].rows);
        assert_eq!(view.visual_rows, view.lines[0].rows + 1);
    });
}

#[gpui::test]
fn home_and_end_move_within_the_drawn_row_of_a_wrapped_prompt(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    let text = "intention ".repeat(80).trim_end().to_owned();
    cx.simulate_input(&text);
    cx.run_until_parked();
    let ranges = update(&editor, cx, |view, _, _| view.lines[0].row_ranges());
    assert!(ranges.len() >= 3, "the text wraps onto several rows");
    let caret = |editor: &Entity<TextEditor>, cx: &mut VisualTestContext| {
        update(editor, cx, |view, _, _| {
            let cursor = view.caret();
            (cursor, view.drawn_row(cursor).map(|(row, _)| row))
        })
    };
    let last = ranges.len() - 1;

    // On the last row, Home goes to that row's start, not to the start of the whole line.
    cx.simulate_keystrokes("home");
    assert_eq!(caret(&editor, cx), (ranges[last].start, Some(last)));
    // That offset is also where the row above ends; the caret is drawn on the row it was
    // sent to, and Home again stays put.
    cx.simulate_keystrokes("home");
    assert_eq!(caret(&editor, cx), (ranges[last].start, Some(last)));
    cx.simulate_keystrokes("end");
    assert_eq!(caret(&editor, cx), (text.len(), Some(last)));

    // A row in the middle: End stops at the end of that row, Home at its start.
    cx.simulate_keystrokes("up");
    let (_, row) = caret(&editor, cx);
    assert_eq!(row, Some(last - 1));
    cx.simulate_keystrokes("end");
    assert_eq!(caret(&editor, cx), (ranges[last - 1].end, Some(last - 1)));
    cx.simulate_keystrokes("home");
    assert_eq!(caret(&editor, cx), (ranges[last - 1].start, Some(last - 1)));
    // From a row's start, Down lands on the start of the next row, not the end of this one.
    cx.simulate_keystrokes("down");
    assert_eq!(caret(&editor, cx), (ranges[last].start, Some(last)));

    // Shift+Home selects back to the start of the row only.
    cx.simulate_keystrokes("end shift-home");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.buffer().selection(), ranges[last].start..text.len());
    });

    // Typing ends the row-start placement: the caret follows the text again.
    cx.simulate_keystrokes("home");
    cx.simulate_input("x");
    update(&editor, cx, |view, _, _| assert!(!view.row_start));

    // The first row's Home is the start of the text.
    for _ in 0..ranges.len() {
        cx.simulate_keystrokes("up");
    }
    cx.simulate_keystrokes("home");
    assert_eq!(caret(&editor, cx), (0, Some(0)));
}

#[gpui::test]
fn a_file_keeps_home_and_end_on_its_lines(cx: &mut TestAppContext) {
    let (_directory, _project, editor, cx) = file_editor(cx, "first line\r\nsecond\n");
    update(&editor, cx, |view, _, cx| view.select(3..3, cx));
    cx.simulate_keystrokes("end");
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.caret(), "first line".len(), "before the CR");
        assert_eq!(view.drawn_row(view.caret()), None, "files aren't wrapped");
    });
    cx.simulate_keystrokes("home");
    update(&editor, cx, |view, _, _| assert_eq!(view.caret(), 0));
}

#[gpui::test]
fn the_view_scrolls_to_keep_the_caret_in_sight(cx: &mut TestAppContext) {
    let (editor, cx) = prompt(cx);
    // Far more rows than any window is tall.
    update(&editor, cx, |view, _, cx| {
        view.replace(0..0, &"row\n".repeat(400), cx);
    });
    cx.run_until_parked();
    let line = super::paint::Geometry::current(false).line;
    let (viewport, scrolled) = update(&editor, cx, |view, _, _| {
        assert_eq!(view.caret(), view.buffer().text().len());
        (
            f32::from(view.scroll.bounds().size.height),
            -f32::from(view.scroll.offset().y),
        )
    });
    assert!(
        viewport > 0. && viewport < 400. * line,
        "the text overflows"
    );
    // The caret is on the last row (401, empty); typing there used to happen out of sight.
    let caret_top = 400. * line;
    assert!(
        scrolled > 0. && caret_top >= scrolled && caret_top + line <= scrolled + viewport,
        "row at {caret_top} is outside {scrolled}..{}",
        scrolled + viewport
    );

    // Moving the caret back to the start brings the start back.
    update(&editor, cx, |view, _, cx| view.select(0..0, cx));
    cx.run_until_parked();
    update(&editor, cx, |view, _, _| {
        assert_eq!(f32::from(view.scroll.offset().y), 0.);
    });

    // With the caret still, scrolling elsewhere is left alone.
    update(&editor, cx, |view, _, cx| {
        view.scroll.set_offset(point(px(0.), px(-10. * line)));
        cx.notify();
    });
    cx.run_until_parked();
    update(&editor, cx, |view, _, _| {
        assert_eq!(f32::from(view.scroll.offset().y), -10. * line);
    });
    // The next key brings the caret back into view.
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    update(&editor, cx, |view, _, _| {
        assert_eq!(f32::from(view.scroll.offset().y), 0.);
    });
}

#[gpui::test]
fn a_file_scrolls_sideways_to_a_caret_past_its_edge(cx: &mut TestAppContext) {
    let long = "let value = 1; ".repeat(400);
    let (_directory, _project, editor, cx) = file_editor(cx, &long);
    update(&editor, cx, |view, _, _| {
        assert_eq!(f32::from(view.scroll.offset().x), 0.);
    });
    cx.simulate_keystrokes("end");
    cx.run_until_parked();
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.caret(), long.len());
        assert!(
            f32::from(view.scroll.offset().x) < 0.,
            "the end of the long line is brought into view"
        );
    });
    cx.simulate_keystrokes("home");
    cx.run_until_parked();
    update(&editor, cx, |view, _, _| {
        assert_eq!(f32::from(view.scroll.offset().x), 0.);
    });
}

#[gpui::test]
fn files_are_never_wrapped(cx: &mut TestAppContext) {
    let long = "let value = 1; ".repeat(200);
    let (_directory, _project, editor, cx) = file_editor(cx, &long);
    update(&editor, cx, |view, _, _| {
        assert_eq!(view.lines.len(), 1);
        assert_eq!(view.lines[0].rows, 1, "a file's long line scrolls sideways");
        assert!(view.lines[0].wrapped.is_none());
        assert_eq!(view.lines[0].row_ranges(), vec![0..long.len()]);
        assert_eq!(view.row_above_or_below(10, true), None);
    });
}
