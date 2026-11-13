// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:50:02.412850
//+ Revised: 	2026/11/13 07:53:53.161448
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
        let line = view.lines[0].shaped.as_ref().unwrap();
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
