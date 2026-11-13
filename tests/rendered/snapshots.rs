// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:13:23.068665
//+ Revised: 	2026/11/12 09:16:51.984319
// -----------------------------------------------------------------------------

use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result, ensure};
use image::{Rgb, RgbImage};

const MAX_CHANNEL_DIFFERENCE: u8 = 8;
const MAX_CHANGED_FRACTION: f64 = 0.002;

pub fn assert_snapshot(name: &str, actual: &RgbImage) -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/rendered");
    let baseline = root.join("baselines/windows").join(format!("{name}.png"));
    if env::var("ORIN_UPDATE_BASELINES").as_deref() == Ok("1") {
        fs::create_dir_all(baseline.parent().context("baseline has no parent")?)?;
        actual.save(&baseline)?;
        return Ok(());
    }
    let expected = image::open(&baseline)
        .with_context(|| {
            format!(
                "missing or invalid reviewed baseline {}",
                baseline.display()
            )
        })?
        .into_rgb8();
    let artifacts = root.join("artifacts/windows");
    fs::create_dir_all(&artifacts)?;
    actual.save(artifacts.join(format!("{name}-actual.png")))?;
    ensure!(
        actual.dimensions() == expected.dimensions(),
        "snapshot dimensions changed for {name}"
    );
    let (changed, diff) = difference(&expected, actual);
    let fraction = changed as f64 / (actual.width() as f64 * actual.height() as f64);
    if fraction > MAX_CHANGED_FRACTION {
        diff.save(artifacts.join(format!("{name}-diff.png")))?;
    }
    ensure!(
        fraction <= MAX_CHANGED_FRACTION,
        "{name}: {changed} pixels ({:.3}%) changed; inspect actual/diff artifacts",
        fraction * 100.0
    );
    Ok(())
}

fn difference(expected: &RgbImage, actual: &RgbImage) -> (usize, RgbImage) {
    let mut changed = 0;
    let diff = RgbImage::from_fn(actual.width(), actual.height(), |x, y| {
        let before = expected.get_pixel(x, y);
        let after = actual.get_pixel(x, y);
        if before
            .0
            .iter()
            .zip(after.0)
            .any(|(left, right)| left.abs_diff(right) > MAX_CHANNEL_DIFFERENCE)
        {
            changed += 1;
            Rgb([255, 0, 80])
        } else {
            Rgb([after[0] / 4, after[1] / 4, after[2] / 4])
        }
    });
    (changed, diff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_diff_ignores_small_channel_variation() {
        let expected = RgbImage::from_pixel(2, 2, Rgb([20, 30, 40]));
        let actual = RgbImage::from_pixel(2, 2, Rgb([28, 22, 48]));
        assert_eq!(difference(&expected, &actual).0, 0);
    }

    #[test]
    fn snapshot_diff_marks_changed_pixels() {
        let expected = RgbImage::from_pixel(2, 2, Rgb([20, 30, 40]));
        let mut actual = expected.clone();
        actual.put_pixel(1, 0, Rgb([30, 30, 40]));
        let (changed, diff) = difference(&expected, &actual);
        assert_eq!(changed, 1);
        assert_eq!(diff.get_pixel(1, 0), &Rgb([255, 0, 80]));
    }
}
