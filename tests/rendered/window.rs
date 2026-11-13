// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:22.766174
//+ Revised: 	2026/11/12 09:16:52.339726
// -----------------------------------------------------------------------------

use std::{
    sync::Once,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    UI::{
        HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
            SetProcessDpiAwarenessContext,
        },
        WindowsAndMessaging::*,
    },
};
use windows::core::BOOL;

pub struct AppWindow {
    pub handle: HWND,
}

struct Search {
    process: u32,
    handle: HWND,
}

unsafe extern "system" fn find_process_window(handle: HWND, argument: LPARAM) -> BOOL {
    // EnumWindows invokes this callback synchronously while Search remains alive.
    let search = unsafe { &mut *(argument.0 as *mut Search) };
    let mut process = 0;
    unsafe { GetWindowThreadProcessId(handle, Some(&mut process)) };
    if process == search.process && unsafe { IsWindowVisible(handle) }.as_bool() {
        search.handle = handle;
        return BOOL(0);
    }
    BOOL(1)
}

impl AppWindow {
    pub fn wait(process: u32) -> Result<Self> {
        static DPI_AWARE: Once = Once::new();
        DPI_AWARE.call_once(|| {
            // Failure means a test process manifest already selected its DPI mode.
            let _ = unsafe {
                SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
            };
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let mut search = Search {
                process,
                handle: HWND::default(),
            };
            // Early termination makes EnumWindows return an error; handle is authoritative.
            let _ = unsafe {
                EnumWindows(
                    Some(find_process_window),
                    LPARAM(&mut search as *mut Search as isize),
                )
            };
            if !search.handle.0.is_null() {
                return Ok(Self {
                    handle: search.handle,
                });
            }
            if Instant::now() > deadline {
                bail!("Orin process {process} did not open a native window");
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn dimensions(&self) -> Result<(u32, u32)> {
        let mut bounds = RECT::default();
        unsafe { GetClientRect(self.handle, &mut bounds) }.context("query native client bounds")?;
        Ok((
            (bounds.right - bounds.left) as u32,
            (bounds.bottom - bounds.top) as u32,
        ))
    }

    /// Move the pointer to the status bar's empty left corner, where nothing reacts to hover.
    pub fn park_pointer(&self) -> Result<()> {
        let (_, height) = self.dimensions()?;
        let scale = unsafe { GetDpiForWindow(self.handle) } as f64 / 96.0;
        let y = f64::from(height) / scale - 3.0;
        unsafe {
            PostMessageW(
                Some(self.handle),
                WM_MOUSEMOVE,
                WPARAM(0),
                self.coordinate(2.0, y),
            )?;
        }
        Ok(())
    }

    /// Logical coordinates to the device-pixel `LPARAM` mouse messages carry.
    fn coordinate(&self, x: f64, y: f64) -> LPARAM {
        let scale = unsafe { GetDpiForWindow(self.handle) } as f64 / 96.0;
        LPARAM((((y * scale).round() as u32) << 16 | (x * scale).round() as u32) as isize)
    }

    /// Press at `from`, move in steps with the left button held, and release at `to`.
    pub fn drag(&self, from: (f64, f64), to: (f64, f64)) -> Result<()> {
        const STEPS: u32 = 8;
        unsafe {
            PostMessageW(
                Some(self.handle),
                WM_MOUSEMOVE,
                WPARAM(0),
                self.coordinate(from.0, from.1),
            )?;
            PostMessageW(
                Some(self.handle),
                WM_LBUTTONDOWN,
                WPARAM(1),
                self.coordinate(from.0, from.1),
            )?;
            for step in 1..=STEPS {
                let progress = f64::from(step) / f64::from(STEPS);
                let x = from.0 + (to.0 - from.0) * progress;
                let y = from.1 + (to.1 - from.1) * progress;
                PostMessageW(
                    Some(self.handle),
                    WM_MOUSEMOVE,
                    WPARAM(1),
                    self.coordinate(x, y),
                )?;
            }
            PostMessageW(
                Some(self.handle),
                WM_LBUTTONUP,
                WPARAM(0),
                self.coordinate(to.0, to.1),
            )?;
        }
        Ok(())
    }

    pub fn click(&self, x: f64, y: f64) -> Result<()> {
        let coordinate = self.coordinate(x, y);
        unsafe {
            PostMessageW(Some(self.handle), WM_MOUSEMOVE, WPARAM(0), coordinate)?;
            PostMessageW(Some(self.handle), WM_LBUTTONDOWN, WPARAM(1), coordinate)?;
            PostMessageW(Some(self.handle), WM_LBUTTONUP, WPARAM(0), coordinate)?;
        }
        Ok(())
    }

    pub fn type_text(&self, text: &str) -> Result<()> {
        for character in text.encode_utf16() {
            unsafe {
                PostMessageW(
                    Some(self.handle),
                    WM_CHAR,
                    WPARAM(character as usize),
                    LPARAM(1),
                )
            }?;
        }
        Ok(())
    }

    pub fn enter(&self) -> Result<()> {
        unsafe {
            PostMessageW(
                Some(self.handle),
                WM_KEYDOWN,
                WPARAM(0x0D),
                LPARAM(0x001C0001),
            )?;
            PostMessageW(
                Some(self.handle),
                WM_KEYUP,
                WPARAM(0x0D),
                LPARAM(0xC01C0001u32 as isize),
            )?;
        }
        Ok(())
    }

    pub fn close(&self) {
        let _ = unsafe { PostMessageW(Some(self.handle), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    }
}
