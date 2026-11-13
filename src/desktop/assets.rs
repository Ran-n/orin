// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:18:28.933335
//+ Revised: 	2026/11/12 22:43:39.034540
// -----------------------------------------------------------------------------

//! Embedded images: the approved mascot and the interface icons, so a built app needs no asset folder.

use gpui::{AssetSource, ImageSource, RenderImage, SharedString};
use image::{Frame, RgbaImage, imageops::FilterType};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, OnceLock},
};

const MASCOT_PNG: &[u8] = include_bytes!("../../assets/branding/orin-mascot.png");

/// Interface icons from Lucide (ISC license, see `assets/icons/LICENSE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Account,
    Agents,
    ArchiveFile,
    AssetsFolder,
    BuildFile,
    Check,
    Checked,
    ChevronDown,
    ChevronRight,
    Close,
    Code,
    Collapse,
    ConfigFile,
    ConfigFolder,
    DataFile,
    DocsFolder,
    EditSource,
    Error,
    Expand,
    File,
    Files,
    Folder,
    GitBranch,
    GitFolder,
    Guide,
    History,
    ImageFile,
    Info,
    KeyFile,
    LargerText,
    License,
    LockFile,
    OpenFolder,
    OutputFolder,
    People,
    Plus,
    Preview,
    Redo,
    Refresh,
    Reload,
    ResetLayout,
    Save,
    ScriptFile,
    Search,
    Send,
    Settings,
    SmallerText,
    SourceFolder,
    Start,
    Stop,
    Terminal,
    TestsFolder,
    TextFile,
    Unchecked,
    Undo,
}

macro_rules! icons {
    ($($icon:ident => $file:literal),* $(,)?) => {
        impl Icon {
            pub const ALL: &[Icon] = &[$(Icon::$icon),*];

            /// The asset path GPUI's `svg()` element loads through [`Assets`].
            pub fn path(self) -> &'static str {
                match self { $(Icon::$icon => concat!("icons/", $file)),* }
            }

            fn bytes(self) -> &'static [u8] {
                match self { $(Icon::$icon => include_bytes!(concat!("../../assets/icons/", $file))),* }
            }
        }
    };
}

icons! {
    Account => "circle-user-round.svg",
    Agents => "bot.svg",
    ArchiveFile => "file-archive.svg",
    AssetsFolder => "folder-heart.svg",
    BuildFile => "hammer.svg",
    Check => "check.svg",
    Checked => "square-check.svg",
    ChevronDown => "chevron-down.svg",
    ChevronRight => "chevron-right.svg",
    Close => "x.svg",
    Code => "file-code.svg",
    Collapse => "chevrons-left.svg",
    ConfigFile => "file-cog.svg",
    ConfigFolder => "folder-cog.svg",
    DataFile => "file-json.svg",
    DocsFolder => "folder-pen.svg",
    EditSource => "pencil.svg",
    Error => "circle-alert.svg",
    Expand => "chevrons-right.svg",
    File => "file.svg",
    Files => "folder-tree.svg",
    Folder => "folder.svg",
    GitBranch => "git-branch.svg",
    GitFolder => "folder-git-2.svg",
    Guide => "book-open.svg",
    History => "history.svg",
    ImageFile => "file-image.svg",
    Info => "info.svg",
    KeyFile => "file-key.svg",
    LargerText => "a-arrow-up.svg",
    License => "scale.svg",
    LockFile => "file-lock.svg",
    OpenFolder => "folder-open.svg",
    OutputFolder => "folder-archive.svg",
    People => "users.svg",
    Plus => "plus.svg",
    Preview => "eye.svg",
    Redo => "redo-2.svg",
    Refresh => "refresh-cw.svg",
    Reload => "rotate-ccw.svg",
    ResetLayout => "columns-3.svg",
    Save => "save.svg",
    ScriptFile => "file-terminal.svg",
    Search => "search.svg",
    Send => "send-horizontal.svg",
    Settings => "settings.svg",
    SmallerText => "a-arrow-down.svg",
    SourceFolder => "folder-code.svg",
    Start => "play.svg",
    Stop => "square.svg",
    Terminal => "square-terminal.svg",
    TestsFolder => "folder-check.svg",
    TextFile => "file-text.svg",
    Unchecked => "square.svg",
    Undo => "undo-2.svg",
}

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Icon::ALL
            .iter()
            .find(|icon| icon.path() == path)
            .map(|icon| Cow::Borrowed(icon.bytes())))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Icon::ALL
            .iter()
            .map(|icon| icon.path())
            .filter(|asset| asset.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

/// The mascot resampled to exactly `pixels` device pixels square.
///
/// The source is 1254 px. Letting the GPU shrink it to a few dozen pixels in one step skips most
/// source pixels and looks jagged, so it is resampled once with a Lanczos filter and cached per size.
pub fn mascot(pixels: u32) -> ImageSource {
    thread_local! {
        static CACHE: RefCell<HashMap<u32, Arc<RenderImage>>> = RefCell::new(HashMap::new());
    }
    let pixels = pixels.clamp(1, 1024);
    let image = CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry(pixels)
            .or_insert_with(|| Arc::new(RenderImage::new(vec![Frame::new(resampled(pixels))])))
            .clone()
    });
    ImageSource::Render(image)
}

/// The decoded source, shared by every size. The embedded PNG is verified by tests, so a decode
/// failure is a build defect; it is logged and drawn as a transparent square rather than crashing.
fn source() -> &'static RgbaImage {
    static SOURCE: OnceLock<RgbaImage> = OnceLock::new();
    SOURCE.get_or_init(|| match image::load_from_memory(MASCOT_PNG) {
        Ok(image) => image.into_rgba8(),
        Err(error) => {
            tracing::error!(target: "orin::desktop", %error, "Embedded mascot could not be decoded");
            RgbaImage::new(1, 1)
        }
    })
}

fn resampled(pixels: u32) -> RgbaImage {
    let mut image = image::imageops::resize(source(), pixels, pixels, FilterType::Lanczos3);
    // GPUI uploads image frames as BGRA.
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_served_as_svg_by_its_path() {
        for icon in Icon::ALL {
            let bytes = Assets.load(icon.path()).unwrap().expect("icon bytes");
            assert!(String::from_utf8_lossy(&bytes).contains("<svg"), "{icon:?}");
        }
        assert_eq!(Assets.list("icons/").unwrap().len(), Icon::ALL.len());
        assert!(Assets.load("orin-mascot.png").unwrap().is_none());
    }

    #[test]
    fn embedded_mascot_decodes_at_full_resolution() {
        assert_eq!(source().dimensions(), (1254, 1254));
    }

    #[test]
    fn mascot_is_resampled_to_the_exact_size_and_swapped_to_bgra() {
        let converted = resampled(36);
        assert_eq!(converted.dimensions(), (36, 36));
        let reference = image::imageops::resize(source(), 36, 36, FilterType::Lanczos3);
        for (bgra, rgba) in converted.pixels().zip(reference.pixels()) {
            assert_eq!([bgra[2], bgra[1], bgra[0], bgra[3]], rgba.0);
        }
    }

    #[test]
    fn regression_downscaled_mascot_is_filtered_not_point_sampled() {
        let lanczos = image::imageops::resize(source(), 36, 36, FilterType::Lanczos3);
        let nearest = image::imageops::resize(source(), 36, 36, FilterType::Nearest);
        let difference: u64 = lanczos
            .pixels()
            .zip(nearest.pixels())
            .map(|(a, b)| {
                a.0.iter()
                    .zip(b.0)
                    .map(|(x, y)| u64::from(x.abs_diff(y)))
                    .sum::<u64>()
            })
            .sum();
        assert!(difference > 10_000, "{difference}");
    }

    #[test]
    fn mascot_sizes_are_cached_and_bounded() {
        let ImageSource::Render(first) = mascot(40) else {
            panic!("render image expected")
        };
        let ImageSource::Render(second) = mascot(40) else {
            panic!("render image expected")
        };
        assert!(Arc::ptr_eq(&first, &second));
        assert!(matches!(mascot(0), ImageSource::Render(_)));
    }
}
