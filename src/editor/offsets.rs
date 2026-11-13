// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:34:17.960063
//+ Revised: 	2026/11/12 09:16:32.772593
// -----------------------------------------------------------------------------

//! Explicit conversion between GPUI UTF-16 positions and Rust UTF-8 byte offsets.

pub fn utf16_to_byte(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_position = 0;
    for (byte_offset, character) in text.char_indices() {
        if utf16_position + character.len_utf16() > utf16_offset {
            return byte_offset;
        }
        utf16_position += character.len_utf16();
    }
    text.len()
}

pub fn byte_to_utf16(text: &str, byte_offset: usize) -> usize {
    text.char_indices()
        .take_while(|(offset, character)| {
            offset + character.len_utf8() <= byte_offset.min(text.len())
        })
        .map(|(_, character)| character.len_utf16())
        .sum()
}
