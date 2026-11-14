// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 14:56:17.250422
//+ Revised: 	2026/11/13 20:02:49.518408
// -----------------------------------------------------------------------------

//! Build script: on Windows, embeds the mascot as the executable's icon.
//!
//! GPUI's Windows backend registers its window class with icon resource 1 of the running
//! executable, which Windows shows in the title bar, the taskbar, Alt+Tab and Explorer. The icon
//! is generated here from `assets/branding/orin-mascot.png`, so the PNG stays the only source. Its
//! transparent margin is trimmed the same way as in the app, so the acorn fills the icon.

#[path = "src/desktop/mascot_frame.rs"]
mod mascot_frame;

use std::{env, error::Error, fs::File, io::BufWriter, path::PathBuf};

const MASCOT: &str = "assets/branding/orin-mascot.png";

/// Every size Windows picks from: small icons at 100–200 % scale, the taskbar, Alt+Tab and Explorer.
const ICON_SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={MASCOT}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/desktop/mascot_frame.rs");
    if env::var("CARGO_CFG_TARGET_OS")? != "windows" {
        return Ok(());
    }
    let icon = PathBuf::from(env::var("OUT_DIR")?).join("orin.ico");
    write_icon(&icon)?;
    let mut resource = winresource::WindowsResource::new();
    // GPUI loads the icon by resource ID 1.
    resource.set_icon_with_id(icon.to_str().ok_or("OUT_DIR is not valid UTF-8")?, "1");
    resource.compile()?;
    Ok(())
}

fn write_icon(path: &PathBuf) -> Result<(), Box<dyn Error>> {
    use image::{
        ExtendedColorType,
        codecs::ico::{IcoEncoder, IcoFrame},
        imageops::FilterType,
    };
    let source = mascot_frame::trimmed_square(&image::open(MASCOT)?.into_rgba8());
    let frames = ICON_SIZES
        .iter()
        .map(|&size| {
            let resized = image::imageops::resize(&source, size, size, FilterType::Lanczos3);
            IcoFrame::as_png(resized.as_raw(), size, size, ExtendedColorType::Rgba8)
        })
        .collect::<Result<Vec<_>, _>>()?;
    IcoEncoder::new(BufWriter::new(File::create(path)?)).encode_images(&frames)?;
    Ok(())
}
