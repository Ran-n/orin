// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 14:58:23.286368
//+ Revised: 	2026/11/12 15:01:44.980394
// -----------------------------------------------------------------------------

#![cfg(all(windows, feature = "desktop"))]

//! The built executable carries the mascot as icon resource 1, which GPUI's Windows backend loads
//! for the window class: without it the title bar and taskbar show the generic application icon.

use windows::{
    Win32::{
        Foundation::FreeLibrary,
        System::LibraryLoader::{FindResourceW, LOAD_LIBRARY_AS_DATAFILE, LoadLibraryExW},
        UI::WindowsAndMessaging::RT_GROUP_ICON,
    },
    core::{HSTRING, PCWSTR},
};

#[test]
fn regression_executable_embeds_icon_resource_one_for_title_bar_and_taskbar() {
    let exe = HSTRING::from(env!("CARGO_BIN_EXE_orin"));
    let module = unsafe { LoadLibraryExW(&exe, None, LOAD_LIBRARY_AS_DATAFILE) }
        .expect("load orin.exe as data");
    let group = unsafe { FindResourceW(Some(module), PCWSTR(1 as _), RT_GROUP_ICON) };
    unsafe { FreeLibrary(module) }.expect("free orin.exe");
    assert!(
        !group.is_invalid(),
        "orin.exe has no icon group with resource ID 1"
    );
}
