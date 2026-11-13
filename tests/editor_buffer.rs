// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:18.860404
//+ Revised: 	2026/11/12 15:17:56.499899
// -----------------------------------------------------------------------------

use orin::{
    editor::{EditorBuffer, EditorDocument, EditorError, byte_to_utf16, utf16_to_byte},
    project::Project,
};
use std::{fs, path::Path};
use tempfile::TempDir;

#[test]
fn regression_small_edits_cannot_retain_unbounded_history_records() {
    // The byte budget counted text only, allowing millions of tiny heap-allocated records.
    let mut buffer = EditorBuffer::new(String::new());
    for _ in 0..10_001 {
        buffer.break_undo_group();
        let end = buffer.text().len();
        buffer.replace(end..end, "x").unwrap();
    }
    let mut undone = 0;
    while buffer.undo() {
        undone += 1;
    }
    assert_eq!(undone, 10_000);
    assert_eq!(buffer.text(), "x");
    for _ in 0..undone {
        assert!(buffer.redo());
    }
    assert_eq!(buffer.text().len(), 10_001);
    assert!(!buffer.redo());
}

#[test]
fn history_byte_limit_keeps_the_latest_edits_and_new_edits_clear_redo() {
    let size = 1024 * 1024;
    let mut buffer = EditorBuffer::new("a".repeat(size));
    for character in ['b', 'c', 'd', 'e', 'f', 'g'] {
        buffer
            .replace(0..size, &character.to_string().repeat(size))
            .unwrap();
    }
    for _ in 0..4 {
        assert!(buffer.undo());
    }
    assert!(!buffer.undo());
    assert_eq!(buffer.text(), "c".repeat(size));
    buffer.replace(0..size, "new branch").unwrap();
    assert!(!buffer.can_redo());
    assert!(!buffer.redo());
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "c".repeat(size));
}

#[test]
fn unicode_selection_replacement_and_undo_restore_selection() {
    let mut buffer = EditorBuffer::new("a🌱éz".into());
    buffer.set_selection(1..7).unwrap();
    buffer.replace_selection("oak").unwrap();
    assert_eq!(buffer.text(), "aoakz");
    assert_eq!(buffer.selection(), 4..4);
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "a🌱éz");
    assert_eq!(buffer.selection(), 1..7);
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "aoakz");
    assert_eq!(buffer.revision(), 3);
}

#[test]
fn invalid_ranges_leave_buffer_selection_and_history_unchanged() {
    let mut buffer = EditorBuffer::new("🌱".into());
    for range in [1..1, 0..3, 4..5, std::ops::Range { start: 4, end: 0 }] {
        assert_eq!(
            buffer.replace(range.clone(), "x"),
            Err(EditorError::InvalidRange)
        );
        assert_eq!(buffer.set_selection(range), Err(EditorError::InvalidRange));
    }
    assert_eq!(buffer.text(), "🌱");
    assert_eq!(buffer.selection(), 0..0);
    assert!(!buffer.can_undo());
    assert!(!buffer.undo());
    assert!(!buffer.redo());
}

#[test]
fn continuous_typing_groups_words_spaces_and_explicit_boundaries() {
    let mut buffer = EditorBuffer::new(String::new());
    for character in "hello world".chars() {
        buffer.replace_selection(&character.to_string()).unwrap();
    }
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "hello ");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "hello");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "");
    buffer.redo();
    buffer.redo();
    buffer.redo();
    buffer.break_undo_group();
    buffer.replace_selection("s").unwrap();
    buffer.undo();
    assert_eq!(buffer.text(), "hello world");
}

#[test]
fn cursor_movements_and_pastes_create_distinct_undo_steps() {
    let mut buffer = EditorBuffer::new(String::new());
    buffer.replace_selection("paste").unwrap();
    buffer.replace_selection("d").unwrap();
    buffer.undo();
    assert_eq!(buffer.text(), "paste");
    buffer.set_selection(0..0).unwrap();
    buffer.replace_selection("x").unwrap();
    assert!(!buffer.can_redo());
    buffer.undo();
    assert_eq!(buffer.text(), "paste");
    buffer.undo();
    assert_eq!(buffer.text(), "");
}

#[test]
fn edits_at_size_limit_and_noop_replacements_are_safe() {
    let mut buffer = EditorBuffer::new("a".repeat(orin::project::MAX_FILE_BYTES));
    buffer.set_selection(0..0).unwrap();
    assert_eq!(buffer.replace_selection("x"), Err(EditorError::TooLarge));
    assert!(!buffer.can_undo());
    buffer.replace(0..1, "a").unwrap();
    assert_eq!(buffer.revision(), 0);
    buffer.replace(0..1, "").unwrap();
    assert_eq!(buffer.text().len(), orin::project::MAX_FILE_BYTES - 1);
    assert!(buffer.undo());
    assert_eq!(buffer.text().len(), orin::project::MAX_FILE_BYTES);
}

#[test]
fn utf16_offsets_handle_surrogates_combining_text_and_clamping() {
    let text = "a🌱e\u{301}";
    assert_eq!(utf16_to_byte(text, 0), 0);
    assert_eq!(utf16_to_byte(text, 1), 1);
    assert_eq!(utf16_to_byte(text, 2), 1);
    assert_eq!(utf16_to_byte(text, 3), 5);
    assert_eq!(utf16_to_byte(text, 99), text.len());
    assert_eq!(byte_to_utf16(text, 3), 1);
    assert_eq!(byte_to_utf16(text, 5), 3);
    assert_eq!(byte_to_utf16(text, 99), 5);
}

#[test]
fn save_checkpoint_preserves_edits_made_during_background_save() {
    let directory = TempDir::new().unwrap();
    fs::write(directory.path().join("file.rs"), "before").unwrap();
    let project = Project::open(directory.path()).unwrap();
    let file = project.read_file(Path::new("file.rs")).unwrap();
    let mut document = EditorDocument::from_file(file);
    assert!(!document.is_dirty());
    document.buffer_mut().replace(0..6, "saved").unwrap();
    let saved = project
        .save_file(document.original(), document.buffer().text())
        .unwrap();
    document.buffer_mut().replace(0..5, "newer").unwrap();
    document.accept_saved(saved).unwrap();
    assert!(document.is_dirty());
    assert_eq!(document.buffer().text(), "newer");
    assert_eq!(document.original().text, "saved");
    document.buffer_mut().undo();
    assert_eq!(document.buffer().text(), "saved");
    assert!(!document.is_dirty());
}

#[test]
fn unrelated_save_acknowledgement_cannot_replace_document_checkpoint() {
    let directory = TempDir::new().unwrap();
    for name in ["first", "second"] {
        fs::write(directory.path().join(name), name).unwrap();
    }
    let project = Project::open(directory.path()).unwrap();
    let mut document = EditorDocument::from_file(project.read_file(Path::new("first")).unwrap());
    let second = project.read_file(Path::new("second")).unwrap();
    assert_eq!(
        document.accept_saved(second),
        Err(EditorError::WrongDocument)
    );
    assert_eq!(document.path(), Path::new("first"));
    assert_eq!(document.original().text, "first");
}
