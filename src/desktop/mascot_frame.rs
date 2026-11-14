// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/13 20:02:47.295778
//+ Revised: 	2026/11/13 20:02:47.295778
// -----------------------------------------------------------------------------

//! Frames the mascot for display: its transparent margin is trimmed so the art fills the space it is given.
//!
//! The approved PNG keeps a wide transparent border, which made the acorn look small at icon sizes.
//! The file itself stays unchanged (ADR-003); the border is dropped when it is loaded. This module is
//! also included by `build.rs` for the Windows executable icon, so it depends on `image` alone.

use image::{RgbaImage, imageops};

/// Alpha above which a pixel counts as part of the art; fainter pixels are anti-aliasing haze.
const VISIBLE_ALPHA: u8 = 16;

/// `source` cropped to its visible art and centred in the smallest transparent square that holds it.
/// An image with no visible pixel is returned unchanged.
pub fn trimmed_square(source: &RgbaImage) -> RgbaImage {
    let Some((left, top, right, bottom)) = visible_bounds(source) else {
        return source.clone();
    };
    let (width, height) = (right - left + 1, bottom - top + 1);
    let side = width.max(height);
    let art = imageops::crop_imm(source, left, top, width, height).to_image();
    let mut square = RgbaImage::new(side, side);
    imageops::replace(
        &mut square,
        &art,
        i64::from((side - width) / 2),
        i64::from((side - height) / 2),
    );
    square
}

/// The inclusive `(left, top, right, bottom)` box around every visible pixel.
fn visible_bounds(source: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    source
        .enumerate_pixels()
        .filter(|(_, _, pixel)| pixel[3] > VISIBLE_ALPHA)
        .fold(None, |bounds, (x, y, _)| {
            Some(match bounds {
                None => (x, y, x, y),
                Some((left, top, right, bottom)) => {
                    (left.min(x), top.min(y), right.max(x), bottom.max(y))
                }
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    const INK: Rgba<u8> = Rgba([200, 120, 60, 255]);

    fn canvas_with_art(left: u32, top: u32, width: u32, height: u32) -> RgbaImage {
        let mut image = RgbaImage::new(100, 100);
        for y in top..top + height {
            for x in left..left + width {
                image.put_pixel(x, y, INK);
            }
        }
        image
    }

    #[test]
    fn tall_art_fills_the_height_and_is_centred_across() {
        let trimmed = trimmed_square(&canvas_with_art(30, 10, 20, 60));
        assert_eq!(trimmed.dimensions(), (60, 60));
        assert_eq!(*trimmed.get_pixel(20, 0), INK);
        assert_eq!(*trimmed.get_pixel(39, 59), INK);
        assert_eq!(trimmed.get_pixel(19, 30)[3], 0);
        assert_eq!(trimmed.get_pixel(40, 30)[3], 0);
    }

    #[test]
    fn wide_art_fills_the_width_and_is_centred_down() {
        let trimmed = trimmed_square(&canvas_with_art(5, 40, 80, 20));
        assert_eq!(trimmed.dimensions(), (80, 80));
        assert_eq!(*trimmed.get_pixel(0, 30), INK);
        assert_eq!(*trimmed.get_pixel(79, 49), INK);
        assert_eq!(trimmed.get_pixel(40, 29)[3], 0);
        assert_eq!(trimmed.get_pixel(40, 50)[3], 0);
    }

    #[test]
    fn faint_haze_does_not_widen_the_frame() {
        let mut image = canvas_with_art(40, 40, 10, 10);
        image.put_pixel(0, 0, Rgba([255, 255, 255, VISIBLE_ALPHA]));
        assert_eq!(trimmed_square(&image).dimensions(), (10, 10));
    }

    #[test]
    fn art_touching_every_edge_and_empty_images_are_unchanged() {
        let full = canvas_with_art(0, 0, 100, 100);
        assert_eq!(trimmed_square(&full), full);
        let empty = RgbaImage::new(8, 8);
        assert_eq!(trimmed_square(&empty), empty);
    }
}
