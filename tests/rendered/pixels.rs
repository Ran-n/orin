// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:18:35.541331
//+ Revised: 	2026/11/13 07:53:54.257339
// -----------------------------------------------------------------------------

use anyhow::{Context, Result, ensure};
use image::{Rgb, RgbImage};
use serde_json::Value;

/// `GRUVBOX_DARK.disabled_text` and `.disabled_bg` (the default theme); keep in step with `src/desktop/theme.rs`.
const DISABLED_TEXT: [u8; 3] = [0x66, 0x5c, 0x54];
const DISABLED_BG: [u8; 3] = [0x2e, 0x2c, 0x2b];
/// The Gruvbox Dark panel and window colours sit 4 and 6 steps from `DISABLED_BG`, so the fill is
/// matched exactly (flat fills are drawn exactly); a looser match counts the bare panel as dimmed.
const FILL_TOLERANCE: u8 = 1;
const PANEL: [u8; 3] = [0x32, 0x30, 0x2f];
const BACKGROUND: [u8; 3] = [0x28, 0x28, 0x28];

pub fn assert_visible_controls(image: &RgbImage, state: &Value) -> Result<()> {
    let controls = state["controls"]
        .as_array()
        .context("state.controls must be an array")?;
    ensure!(
        !controls.is_empty(),
        "real app reported no rendered controls"
    );
    let scale = image.width() as f64
        / state["window_width"]
            .as_f64()
            .unwrap_or(image.width() as f64);
    for control in controls {
        let coordinate = |name: &str| {
            control[name]
                .as_f64()
                .with_context(|| format!("invalid {name}: {control}"))
        };
        let (x, y, width, height) = (
            coordinate("x")?,
            coordinate("y")?,
            coordinate("width")?,
            coordinate("height")?,
        );
        ensure!(
            width > 0.0 && height > 0.0,
            "control has no rendered area: {control}"
        );
        ensure!(
            x >= -1.0
                && y >= -1.0
                && (x + width) * scale <= image.width() as f64 + 2.0
                && (y + height) * scale <= image.height() as f64 + 2.0,
            "control is clipped: {control}"
        );
    }
    // Columns, session tabs, account cards, file rows, recent-folder rows, the Welcome page, and open menus
    // contain other controls by design; only leaf controls must not overlap.
    let is_recent_row = |id: &str| {
        id.strip_prefix("recent-")
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()))
    };
    let is_menu = |id: &str| id == "account-menu" || id.starts_with("agent-option-menu-");
    let is_container = |control: &Value| {
        control["id"].as_str().is_some_and(|id| {
            // File rows hold their Git badge.
            [
                "column-",
                "rail-",
                "agent-session-",
                "account-card-",
                "file:",
                "folder:",
            ]
            .iter()
            .any(|prefix| id.starts_with(prefix))
                || id == "welcome-page"
                || is_recent_row(id)
                || is_menu(id)
        })
    };
    let area = |control: &Value| {
        let value = |name: &str| control[name].as_f64().unwrap_or_default();
        (value("x"), value("y"), value("width"), value("height"))
    };
    let overlaps = |a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)| {
        let horizontal = (a.0 + a.2).min(b.0 + b.2) - a.0.max(b.0);
        let vertical = (a.1 + a.3).min(b.1 + b.3) - a.1.max(b.1);
        horizontal > 2.0 && vertical > 2.0
    };
    // An open menu floats over the workspace: the controls it covers are hidden, not overlapped.
    let menus: Vec<(f64, f64, f64, f64)> = controls
        .iter()
        .filter(|control| control["id"].as_str().is_some_and(is_menu))
        .map(area)
        .collect();
    let in_menu = |control: &Value| {
        control["id"].as_str().is_some_and(|id| {
            id.starts_with("account-menu-")
                || (id.starts_with("agent-option-") && !id.starts_with("agent-option-chip-"))
        })
    };
    let leaves: Vec<&Value> = controls
        .iter()
        .filter(|control| !is_container(control))
        .filter(|control| {
            in_menu(control) || !menus.iter().any(|menu| overlaps(*menu, area(control)))
        })
        .collect();
    for (index, left) in leaves.iter().enumerate() {
        for right in leaves.iter().skip(index + 1) {
            let coordinate =
                |control: &Value, name: &str| control[name].as_f64().unwrap_or_default();
            let horizontal = (coordinate(left, "x") + coordinate(left, "width"))
                .min(coordinate(right, "x") + coordinate(right, "width"))
                - coordinate(left, "x").max(coordinate(right, "x"));
            let vertical = (coordinate(left, "y") + coordinate(left, "height"))
                .min(coordinate(right, "y") + coordinate(right, "height"))
                - coordinate(left, "y").max(coordinate(right, "y"));
            ensure!(
                horizontal <= 2.0 || vertical <= 2.0,
                "rendered controls overlap: {} and {}",
                left["id"],
                right["id"]
            );
        }
    }
    Ok(())
}

pub fn assert_real_pixels(image: &RgbImage) -> Result<()> {
    let first = image.get_pixel(0, 0);
    let varying = image
        .pixels()
        .filter(|pixel| {
            pixel
                .0
                .iter()
                .zip(first.0)
                .any(|(left, right)| left.abs_diff(right) > 12)
        })
        .count();
    ensure!(
        varying > image.width() as usize * 12,
        "PrintWindow returned a blank or mostly blank capture"
    );
    Ok(())
}

pub fn assert_disabled_agents(image: &RgbImage, state: &Value) -> Result<()> {
    let controls = state["controls"]
        .as_array()
        .context("state.controls must be an array")?;
    let scale = image.width() as f64
        / state["window_width"]
            .as_f64()
            .unwrap_or(image.width() as f64);
    for control in controls.iter().filter(|control| {
        control["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("agent-"))
    }) {
        let value = |name: &str| control[name].as_f64().unwrap_or_default() * scale;
        let (x, y, width, height) = (
            value("x") as u32,
            value("y") as u32,
            value("width") as u32,
            value("height") as u32,
        );
        let mut text_pixels = 0;
        let mut background_pixels = 0;
        for row in y..(y + height).min(image.height()) {
            for column in x..(x + width).min(image.width()) {
                let pixel = image.get_pixel(column, row);
                if near(pixel, DISABLED_TEXT) {
                    text_pixels += 1;
                }
                if near_within(pixel, DISABLED_BG, FILL_TOLERANCE) {
                    background_pixels += 1;
                }
            }
        }
        ensure!(
            text_pixels > 5 && background_pixels > 20,
            "disabled agent is not visibly dimmed: {control}"
        );
    }
    Ok(())
}

/// A Files row's horizontal layout at the default text size, in logical pixels; keep in step with
/// `src/desktop/explorer.rs` (`pl(8 + depth * INDENT)`, chevron slot, `gap_1`) and `ICON_SIZE`.
const ROW_PADDING: f64 = 8.0;
const ROW_INDENT: f64 = 12.0;
const ROW_ICON: f64 = 14.0;
const ROW_GAP: f64 = 4.0;

/// Assert the file-type glyph of Files row `control_id` (`file:…` or `folder:…`) is drawn in `rgb`.
pub fn assert_icon_color(
    image: &RgbImage,
    state: &Value,
    control_id: &str,
    rgb: [u8; 3],
) -> Result<()> {
    let start = row_indent(control_id) + ROW_ICON + ROW_GAP;
    assert_row_color(
        image,
        state,
        control_id,
        start,
        start + ROW_ICON,
        rgb,
        "icon",
    )
}

/// Assert the name of Files row `control_id` is drawn in `rgb`.
pub fn assert_label_color(
    image: &RgbImage,
    state: &Value,
    control_id: &str,
    rgb: [u8; 3],
) -> Result<()> {
    let start = row_indent(control_id) + 2.0 * (ROW_ICON + ROW_GAP);
    assert_row_color(image, state, control_id, start, f64::MAX, rgb, "label")
}

fn row_indent(control_id: &str) -> f64 {
    let path = control_id.split_once(':').map_or("", |(_, path)| path);
    ROW_PADDING + path.matches('/').count() as f64 * ROW_INDENT
}

fn assert_row_color(
    image: &RgbImage,
    state: &Value,
    control_id: &str,
    from: f64,
    to: f64,
    rgb: [u8; 3],
    part: &str,
) -> Result<()> {
    let control = state["controls"]
        .as_array()
        .context("state.controls must be an array")?
        .iter()
        .find(|control| control["id"] == control_id)
        .with_context(|| format!("missing control {control_id}"))?;
    let scale = image.width() as f64
        / state["window_width"]
            .as_f64()
            .unwrap_or(image.width() as f64);
    let value = |name: &str| control[name].as_f64().unwrap_or_default();
    let left = ((value("x") + from) * scale) as u32;
    let right = (((value("x") + to.min(value("width"))) * scale) as u32).min(image.width());
    let top = (value("y") * scale) as u32;
    let bottom = (((value("y") + value("height")) * scale) as u32).min(image.height());
    let matching = (top..bottom)
        .flat_map(|row| (left..right).map(move |column| (column, row)))
        .filter(|&(column, row)| near(image.get_pixel(column, row), rgb))
        .count();
    ensure!(
        matching >= 3,
        "{control_id} {part} has no #{:02x}{:02x}{:02x} pixels ({matching})",
        rgb[0],
        rgb[1],
        rgb[2]
    );
    Ok(())
}

fn near(pixel: &Rgb<u8>, color: [u8; 3]) -> bool {
    near_within(pixel, color, 8)
}

fn near_within(pixel: &Rgb<u8>, color: [u8; 3], tolerance: u8) -> bool {
    pixel
        .0
        .iter()
        .zip(color)
        .all(|(left, right)| left.abs_diff(right) <= tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_capture_validation_rejects_black_screens() {
        assert!(assert_real_pixels(&RgbImage::from_pixel(100, 100, Rgb([0, 0, 0]))).is_err());
    }

    #[test]
    fn bounds_validation_rejects_clipped_controls() {
        let image = RgbImage::new(100, 100);
        let state =
            serde_json::json!({"controls":[{"id":"button","x":90,"y":0,"width":20,"height":20}]});
        assert!(assert_visible_controls(&image, &state).is_err());
    }

    #[test]
    fn bounds_validation_accounts_for_display_scale() {
        let image = RgbImage::new(200, 200);
        let state = serde_json::json!({"window_width":100,"controls":[{"id":"button","x":50,"y":10,"width":50,"height":20}]});
        assert!(assert_visible_controls(&image, &state).is_ok());
    }

    #[test]
    fn bounds_validation_rejects_controls_painted_over_each_other() {
        let image = RgbImage::new(100, 100);
        let state = serde_json::json!({"controls":[{"id":"button-a","x":0,"y":0,"width":50,"height":20},{"id":"button-b","x":25,"y":0,"width":50,"height":20}]});
        assert!(assert_visible_controls(&image, &state).is_err());
    }

    #[test]
    fn icon_colour_is_read_from_the_icon_box_not_the_label() {
        let state = serde_json::json!({"controls":[{"id":"folder:src","x":0,"y":0,"width":100,"height":22}]});
        let mut image = RgbImage::new(100, 22);
        // A green label alone doesn't count as a green icon.
        for x in 56..66 {
            image.put_pixel(x, 10, Rgb([0, 200, 0]));
        }
        assert!(assert_icon_color(&image, &state, "folder:src", [0, 200, 0]).is_err());
        assert!(assert_label_color(&image, &state, "folder:src", [0, 200, 0]).is_ok());
        // Icon box at depth 0: 8 + 14 + 4 = 26 to 40.
        for x in 30..34 {
            image.put_pixel(x, 10, Rgb([0, 200, 0]));
        }
        assert!(assert_icon_color(&image, &state, "folder:src", [0, 200, 0]).is_ok());
        // One level deeper the box moves right by the indent.
        let deeper = serde_json::json!({"controls":[{"id":"file:src/main.rs","x":0,"y":0,"width":100,"height":22}]});
        assert!(assert_icon_color(&image, &deeper, "file:src/main.rs", [0, 200, 0]).is_err());
    }

    #[test]
    fn disabled_button_validation_requires_the_actual_dimmed_palette() {
        let mut image = RgbImage::from_pixel(100, 100, Rgb(DISABLED_BG));
        let state = serde_json::json!({"controls":[{"id":"agent-claude","x":0,"y":0,"width":100,"height":100}]});
        assert!(assert_disabled_agents(&image, &state).is_err());
        for x in 20..40 {
            image.put_pixel(x, 50, Rgb(DISABLED_TEXT));
        }
        assert!(assert_disabled_agents(&image, &state).is_ok());
    }

    #[test]
    fn regression_the_bare_panel_is_not_mistaken_for_a_disabled_fill() {
        // Dimmed text on the panel or window colour, with no disabled fill, must not pass.
        let state = serde_json::json!({"controls":[{"id":"agent-claude","x":0,"y":0,"width":100,"height":100}]});
        for ground in [PANEL, BACKGROUND] {
            let mut image = RgbImage::from_pixel(100, 100, Rgb(ground));
            for x in 20..40 {
                image.put_pixel(x, 50, Rgb(DISABLED_TEXT));
            }
            assert!(
                assert_disabled_agents(&image, &state).is_err(),
                "{ground:?}"
            );
        }
    }
}
