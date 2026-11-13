// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:19.074882
//+ Revised: 	2026/11/12 09:16:46.604960
// -----------------------------------------------------------------------------

use orin::editor::{EditorBuffer, byte_to_utf16, utf16_to_byte};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn unicode_boundary_offsets_round_trip(text in ".{0,128}") {
        for boundary in text.char_indices().map(|(offset,_)| offset).chain(std::iter::once(text.len())) {
            prop_assert_eq!(utf16_to_byte(&text, byte_to_utf16(&text, boundary)), boundary);
        }
        for offset in 0..=text.len() + 1 {
            let converted = utf16_to_byte(&text, byte_to_utf16(&text, offset));
            prop_assert!(text.is_char_boundary(converted));
            prop_assert!(converted <= offset.min(text.len()));
        }
    }

    #[test]
    fn edit_undo_redo_round_trip_preserves_unicode(text in ".{0,128}", replacement in ".{1,64}", index in any::<usize>()) {
        let boundaries: Vec<_> = text.char_indices().map(|(offset,_)| offset).chain(std::iter::once(text.len())).collect();
        let position = boundaries[index % boundaries.len()];
        let mut buffer = EditorBuffer::new(text.clone());
        buffer.set_selection(position..position).unwrap();
        buffer.replace_selection(&replacement).unwrap();
        let edited = buffer.text().to_owned();
        prop_assert!(buffer.undo());
        prop_assert_eq!(buffer.text(), text.as_str());
        prop_assert_eq!(buffer.selection(), position..position);
        prop_assert!(buffer.redo());
        prop_assert_eq!(buffer.text(), edited.as_str());
    }
}
