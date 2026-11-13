// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:22.918300
//+ Revised: 	2026/11/12 09:16:49.395071
// -----------------------------------------------------------------------------

use std::{
    mem::size_of,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, bail, ensure};
use image::RgbImage;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::*,
    Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow},
};

use super::window::AppWindow;

struct CaptureSurface {
    window: HWND,
    source: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}

impl CaptureSurface {
    fn new(window: HWND, width: u32, height: u32) -> Result<Self> {
        let mut surface = Self {
            window,
            source: HDC::default(),
            memory: HDC::default(),
            bitmap: HBITMAP::default(),
            previous: HGDIOBJ::default(),
        };
        surface.source = unsafe { GetDC(Some(window)) };
        ensure!(!surface.source.0.is_null(), "GetDC failed");
        surface.memory = unsafe { CreateCompatibleDC(Some(surface.source)) };
        ensure!(!surface.memory.0.is_null(), "CreateCompatibleDC failed");
        surface.bitmap =
            unsafe { CreateCompatibleBitmap(surface.source, width as i32, height as i32) };
        ensure!(!surface.bitmap.0.is_null(), "CreateCompatibleBitmap failed");
        surface.previous = unsafe { SelectObject(surface.memory, surface.bitmap.into()) };
        ensure!(!surface.previous.0.is_null(), "SelectObject failed");
        Ok(surface)
    }

    fn pixels(&self, width: u32, height: u32) -> Result<RgbImage> {
        ensure!(
            unsafe { PrintWindow(self.window, self.memory, PRINT_WINDOW_FLAGS(3)) }.as_bool(),
            "PrintWindow failed"
        );
        let mut information = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0_u8; width as usize * height as usize * 4];
        unsafe { SelectObject(self.memory, self.previous) };
        let lines = unsafe {
            GetDIBits(
                self.source,
                self.bitmap,
                0,
                height,
                Some(pixels.as_mut_ptr().cast()),
                &mut information,
                DIB_RGB_COLORS,
            )
        };
        ensure!(
            lines == height as i32,
            "GetDIBits copied {lines}/{height} rows"
        );
        let rgb = pixels
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[2], pixel[1], pixel[0]])
            .collect();
        Ok(RgbImage::from_raw(width, height, rgb).expect("validated pixel buffer length"))
    }
}

impl Drop for CaptureSurface {
    fn drop(&mut self) {
        unsafe {
            if !self.previous.0.is_null() {
                SelectObject(self.memory, self.previous);
            }
            if !self.bitmap.0.is_null() {
                let _ = DeleteObject(self.bitmap.into());
            }
            if !self.memory.0.is_null() {
                let _ = DeleteDC(self.memory);
            }
            if !self.source.0.is_null() {
                ReleaseDC(Some(self.window), self.source);
            }
        }
    }
}

impl AppWindow {
    fn capture(&self) -> Result<RgbImage> {
        let (width, height) = self.dimensions()?;
        ensure!(width > 0 && height > 0, "native client has no pixels");
        CaptureSurface::new(self.handle, width, height)?.pixels(width, height)
    }

    /// Wait until two captures match, after moving the pointer off every control so no hover
    /// highlight or tooltip from the last click ends up in the image.
    pub fn capture_settled(&self) -> Result<RgbImage> {
        self.park_pointer()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut previous = self.capture()?;
        loop {
            thread::sleep(Duration::from_millis(250));
            let current = self.capture()?;
            if current == previous {
                return Ok(current);
            }
            if Instant::now() >= deadline {
                bail!("native window did not settle for screenshot");
            }
            previous = current;
        }
    }
}
