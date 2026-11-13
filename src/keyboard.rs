// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:17:17.912781
//+ Revised: 	2026/11/12 09:16:33.129699
// -----------------------------------------------------------------------------

//! Encode terminal control keys independently of the desktop event system.

pub fn encode(
    key: &str,
    control: bool,
    alt: bool,
    shift: bool,
    application_cursor: bool,
) -> Option<Vec<u8>> {
    let mut bytes = if control && key.len() == 1 {
        let character = key.as_bytes()[0].to_ascii_uppercase();
        match character {
            b'@'..=b'_' => vec![character & 0x1f],
            b' ' => vec![0],
            b'?' => vec![127],
            _ => return None,
        }
    } else {
        match key {
            "enter" => vec![b'\r'],
            "backspace" => vec![127],
            "escape" => vec![27],
            "tab" if shift => b"\x1b[Z".to_vec(),
            "tab" => vec![b'\t'],
            "up" => arrow(b'A', application_cursor),
            "down" => arrow(b'B', application_cursor),
            "right" => arrow(b'C', application_cursor),
            "left" => arrow(b'D', application_cursor),
            "home" => b"\x1b[H".to_vec(),
            "end" => b"\x1b[F".to_vec(),
            "delete" => b"\x1b[3~".to_vec(),
            "insert" => b"\x1b[2~".to_vec(),
            "pageup" => b"\x1b[5~".to_vec(),
            "pagedown" => b"\x1b[6~".to_vec(),
            "f1" => b"\x1bOP".to_vec(),
            "f2" => b"\x1bOQ".to_vec(),
            "f3" => b"\x1bOR".to_vec(),
            "f4" => b"\x1bOS".to_vec(),
            "f5" => b"\x1b[15~".to_vec(),
            "f6" => b"\x1b[17~".to_vec(),
            "f7" => b"\x1b[18~".to_vec(),
            "f8" => b"\x1b[19~".to_vec(),
            "f9" => b"\x1b[20~".to_vec(),
            "f10" => b"\x1b[21~".to_vec(),
            "f11" => b"\x1b[23~".to_vec(),
            "f12" => b"\x1b[24~".to_vec(),
            _ => return None,
        }
    };
    if alt {
        bytes.insert(0, 27);
    }
    Some(bytes)
}

fn arrow(code: u8, application: bool) -> Vec<u8> {
    vec![27, if application { b'O' } else { b'[' }, code]
}

pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text
        .replace('\u{1b}', "")
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if bracketed {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.into_bytes()
    }
}
