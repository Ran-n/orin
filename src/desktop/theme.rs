// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:18:28.632476
//+ Revised: 	2026/11/12 22:43:39.108043
// -----------------------------------------------------------------------------

//! Orin's compact palettes, sizes, and typography.
//!
//! Gruvbox Dark (medium contrast) is the default palette. Orin's own palettes come from the
//! mascot: walnut and oak-leaf green as accents on near-neutral greys.
//!
//! The active appearance is process-wide, like the system theme: the workspace applies the saved
//! choice before every frame, and every view reads it through [`colors`], [`scaled`], and the
//! font functions.

use crate::settings::{Appearance, Theme};
use gpui::{Div, ParentElement, Pixels, SharedString, Styled, div, px};
use std::sync::{
    RwLock,
    atomic::{AtomicU8, AtomicU32, Ordering},
};

/// Every colour the interface uses, as `0xRRGGBB` (or `0xRRGGBBAA` for the overlays).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    // Surfaces, from the window background up.
    pub background: u32,
    pub panel: u32,
    pub raised: u32,
    pub hover: u32,
    pub active: u32,
    pub border: u32,
    // Text.
    pub text: u32,
    pub soft_text: u32,
    pub muted: u32,
    pub faint: u32,
    // Accents.
    pub accent: u32,
    pub accent_hover: u32,
    pub on_accent: u32,
    pub walnut: u32,
    pub error: u32,
    pub error_surface: u32,
    pub added_surface: u32,
    pub removed_surface: u32,
    pub warning: u32,
    // Disabled controls are dimmed in both background and label.
    pub disabled_bg: u32,
    pub disabled_text: u32,
    // Translucent overlays (RGBA).
    pub selection: u32,
    pub cursor: u32,
    /// Code colours, by kind of token.
    pub syntax: SyntaxColors,
    /// Files column colours for Git changes, as VS Code decorates them.
    pub git: GitColors,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GitColors {
    pub modified: u32,
    /// Added and untracked files.
    pub added: u32,
    pub deleted: u32,
    pub conflict: u32,
    /// Names of paths Git ignores, greyed like VS Code's `ignoredResourceForeground`. Not a
    /// change, so [`GitColors::color`] never returns it.
    pub ignored: u32,
}

impl GitColors {
    pub fn color(&self, status: crate::project::FileStatus) -> u32 {
        use crate::project::FileStatus;
        match status {
            FileStatus::Modified | FileStatus::Renamed => self.modified,
            FileStatus::Added | FileStatus::Untracked => self.added,
            FileStatus::Deleted => self.deleted,
            FileStatus::Conflict => self.conflict,
        }
    }
}

/// Code token colours, in the spirit of the theme each palette comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyntaxColors {
    pub comment: u32,
    pub string: u32,
    pub keyword: u32,
    pub number: u32,
    pub constant: u32,
    pub function: u32,
    pub type_name: u32,
    pub variable: u32,
    pub operator: u32,
    pub tag: u32,
    pub attribute: u32,
    pub heading: u32,
    pub link: u32,
}

impl SyntaxColors {
    pub fn color(&self, token: super::syntax::Token) -> u32 {
        use super::syntax::Token;
        match token {
            Token::Comment => self.comment,
            Token::String => self.string,
            Token::Keyword => self.keyword,
            Token::Number => self.number,
            Token::Constant => self.constant,
            Token::Function => self.function,
            Token::Type => self.type_name,
            Token::Variable => self.variable,
            Token::Operator => self.operator,
            Token::Tag => self.tag,
            Token::Attribute => self.attribute,
            Token::Heading => self.heading,
            Token::Link => self.link,
        }
    }
}

pub const DARK: Palette = Palette {
    background: 0x1a1d1c,
    panel: 0x1f2321,
    raised: 0x282d2a,
    hover: 0x2f3531,
    active: 0x353c37,
    border: 0x2e3430,
    text: 0xdde2da,
    soft_text: 0xc7cdc4,
    muted: 0x8b958a,
    faint: 0x5f685e,
    accent: 0xa9b87a,
    accent_hover: 0xbac88d,
    on_accent: 0x161a17,
    walnut: 0xc29870,
    error: 0xe5a08c,
    error_surface: 0x3a2a25,
    added_surface: 0x23301f,
    removed_surface: 0x35231f,
    warning: 0xd9b45f,
    disabled_bg: 0x232725,
    disabled_text: 0x5a6259,
    selection: 0xb1bc8035,
    cursor: 0xb1bc8060,
    syntax: SyntaxColors {
        comment: 0x6a9955,
        string: 0xce9178,
        keyword: 0x569cd6,
        number: 0xb5cea8,
        constant: 0x569cd6,
        function: 0xdcdcaa,
        type_name: 0x4ec9b0,
        variable: 0x9cdcfe,
        operator: 0xd4d4d4,
        tag: 0x569cd6,
        attribute: 0x9cdcfe,
        heading: 0x569cd6,
        link: 0x3794ff,
    },
    git: GitColors {
        modified: 0xe2c08d,
        added: 0x73c991,
        deleted: 0xc74e39,
        conflict: 0xe4676b,
        ignored: 0x8c8c8c,
    },
};

pub const LIGHT: Palette = Palette {
    background: 0xf7f8f5,
    panel: 0xeef0eb,
    raised: 0xffffff,
    hover: 0xe2e6de,
    active: 0xd8ddd3,
    border: 0xd5dad1,
    text: 0x1d221e,
    soft_text: 0x323a34,
    muted: 0x5c675d,
    faint: 0x949d91,
    accent: 0x5b7a2c,
    accent_hover: 0x4d6a22,
    on_accent: 0xffffff,
    walnut: 0x8a5a2b,
    error: 0xae3b25,
    error_surface: 0xf7e0d9,
    added_surface: 0xdfedd2,
    removed_surface: 0xf6ddd6,
    warning: 0x8f6400,
    disabled_bg: 0xe7e9e4,
    disabled_text: 0xa6aea3,
    selection: 0x5b7a2c33,
    cursor: 0x5b7a2c66,
    syntax: SyntaxColors {
        comment: 0x008000,
        string: 0xa31515,
        keyword: 0x0000ff,
        number: 0x098658,
        constant: 0x0000ff,
        function: 0x795e26,
        type_name: 0x267f99,
        variable: 0x001080,
        operator: 0x000000,
        tag: 0x800000,
        attribute: 0xe50000,
        heading: 0x800000,
        link: 0x0451a5,
    },
    git: GitColors {
        modified: 0x895503,
        added: 0x007100,
        deleted: 0xad0707,
        conflict: 0xad0707,
        ignored: 0x868688,
    },
};

/// Gruvbox dark, medium contrast (bg0 `#282828`), by Pavel Pertsev (MIT).
pub const GRUVBOX_DARK: Palette = Palette {
    background: 0x282828,
    panel: 0x32302f,
    raised: 0x3c3836,
    hover: 0x45403d,
    active: 0x504945,
    border: 0x45403d,
    text: 0xebdbb2,
    soft_text: 0xd5c4a1,
    muted: 0xa89984,
    faint: 0x7c6f64,
    accent: 0xb8bb26,
    accent_hover: 0xc8cb3e,
    on_accent: 0x282828,
    walnut: 0xfabd2f,
    error: 0xfb4934,
    error_surface: 0x44251f,
    added_surface: 0x343a1c,
    removed_surface: 0x44251f,
    warning: 0xfe8019,
    disabled_bg: 0x2e2c2b,
    disabled_text: 0x665c54,
    selection: 0x83a59840,
    cursor: 0xebdbb280,
    syntax: SyntaxColors {
        comment: 0x928374,
        string: 0xb8bb26,
        keyword: 0xfb4934,
        number: 0xd3869b,
        constant: 0xd3869b,
        function: 0x8ec07c,
        type_name: 0xfabd2f,
        variable: 0x83a598,
        operator: 0xfe8019,
        tag: 0x83a598,
        attribute: 0xfabd2f,
        heading: 0xfe8019,
        link: 0x83a598,
    },
    git: GitColors {
        modified: 0xfabd2f,
        added: 0xb8bb26,
        deleted: 0xfb4934,
        conflict: 0xd3869b,
        ignored: 0x928374,
    },
};

/// Gruvbox light, medium contrast (bg0 `#fbf1c7`).
pub const GRUVBOX_LIGHT: Palette = Palette {
    background: 0xfbf1c7,
    panel: 0xf2e5bc,
    raised: 0xf9f5d7,
    hover: 0xebdbb2,
    active: 0xd5c4a1,
    border: 0xd5c4a1,
    text: 0x3c3836,
    soft_text: 0x504945,
    muted: 0x7c6f64,
    faint: 0xa89984,
    accent: 0x79740e,
    accent_hover: 0x66620c,
    on_accent: 0xfbf1c7,
    walnut: 0xb57614,
    error: 0x9d0006,
    error_surface: 0xf5d7c4,
    added_surface: 0xe4e4b8,
    removed_surface: 0xf5d7c4,
    warning: 0xaf3a03,
    disabled_bg: 0xefe3bc,
    disabled_text: 0xbdae93,
    selection: 0x07667830,
    cursor: 0x3c383666,
    syntax: SyntaxColors {
        comment: 0x928374,
        string: 0x79740e,
        keyword: 0x9d0006,
        number: 0x8f3f71,
        constant: 0x8f3f71,
        function: 0x427b58,
        type_name: 0xb57614,
        variable: 0x076678,
        operator: 0xaf3a03,
        tag: 0x076678,
        attribute: 0xb57614,
        heading: 0xaf3a03,
        link: 0x076678,
    },
    git: GitColors {
        modified: 0xa16a10,
        added: 0x79740e,
        deleted: 0x9d0006,
        conflict: 0x8f3f71,
        ignored: 0x8a7b6c,
    },
};

pub fn palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::GruvboxDark => &GRUVBOX_DARK,
        Theme::GruvboxLight => &GRUVBOX_LIGHT,
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
    }
}

static THEME: AtomicU8 = AtomicU8::new(0);
static SCALE_BITS: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0
static FONTS: RwLock<(Option<SharedString>, Option<SharedString>)> = RwLock::new((None, None));

/// Make `appearance` the one every view draws with from the next frame on.
pub fn apply(appearance: &Appearance) {
    let index = Theme::ALL
        .iter()
        .position(|theme| *theme == appearance.theme)
        .unwrap_or_default();
    THEME.store(index as u8, Ordering::Relaxed);
    SCALE_BITS.store(appearance.scale().to_bits(), Ordering::Relaxed);
    let chosen = (
        appearance.interface_font.clone().map(SharedString::from),
        appearance.code_font.clone().map(SharedString::from),
    );
    let mut fonts = FONTS.write().unwrap_or_else(|poison| poison.into_inner());
    if *fonts != chosen {
        *fonts = chosen;
    }
}

/// The active theme.
pub fn active_theme() -> Theme {
    Theme::ALL
        .get(usize::from(THEME.load(Ordering::Relaxed)))
        .copied()
        .unwrap_or_default()
}

/// The palette of the active theme.
pub fn colors() -> &'static Palette {
    palette(active_theme())
}

/// The active text-size scale, 1.0 at the default size.
pub fn scale() -> f32 {
    f32::from_bits(SCALE_BITS.load(Ordering::Relaxed))
}

/// A design-size measurement (text, row, control, icon) at the active text size, in logical pixels.
pub fn metric(base: f32) -> f32 {
    base * scale()
}

/// [`metric`] as GPUI pixels.
pub fn scaled(base: f32) -> Pixels {
    px(metric(base))
}

// Design sizes in logical pixels at the default text size; draw them through `scaled`.
pub const TEXT_SIZE: f32 = 13.0;
pub const SMALL_TEXT: f32 = 12.0;
pub const TINY_TEXT: f32 = 11.0;
pub const HEADING_TEXT: f32 = 18.0;
pub const TITLE_BAR_HEIGHT: f32 = 34.0;
pub const HEADER_HEIGHT: f32 = 32.0;
pub const STATUS_BAR_HEIGHT: f32 = 22.0;
pub const CONTROL_HEIGHT: f32 = 24.0;
pub const ICON_SIZE: f32 = 14.0;
pub const ROW_HEIGHT: f32 = 22.0;
pub const RAIL_WIDTH: f32 = 32.0;
pub const RADIUS: f32 = 4.0;
/// GPUI's spacing helpers (`p_2`, `gap_1`, …) are in rems of this size.
pub const REM: f32 = 16.0;

// Code and terminal geometry.
pub const CODE_TEXT: f32 = 13.0;
pub const CODE_LINE: f32 = 19.0;
pub const CODE_PADDING: f32 = 12.0;
pub const GUTTER: f32 = 52.0;
pub const LINE_NUMBER_TEXT: f32 = 12.0;
pub const CARET_HEIGHT: f32 = 18.0;
pub const TERMINAL_LINE: f32 = 18.0;
pub const TERMINAL_CELL: f32 = 7.8;

/// Divider hit area between columns; layout geometry, not scaled with text.
pub const DIVIDER_WIDTH: f32 = 5.0;

pub fn column() -> Div {
    div().flex().flex_col().min_h_0().min_w_0()
}

pub fn row() -> Div {
    div().flex().items_center().min_w_0()
}

/// Single-line text that fills the remaining width and is clipped at its edge.
///
/// GPUI 0.2.2 only adds "…" when a text's width is known while it is measured, which is not the
/// case inside these flex rows, so long names are clipped rather than ellipsised; show the full
/// value in a tooltip where it matters.
pub fn clipped(text: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .child(text.into())
}

/// The chosen interface font, or the system's.
pub fn interface_font() -> SharedString {
    FONTS
        .read()
        .unwrap_or_else(|poison| poison.into_inner())
        .0
        .clone()
        .unwrap_or_else(|| system_interface_font().into())
}

/// The chosen code font, or the system's monospace font.
pub fn code_font() -> SharedString {
    FONTS
        .read()
        .unwrap_or_else(|poison| poison.into_inner())
        .1
        .clone()
        .unwrap_or_else(|| system_code_font().into())
}

pub fn system_interface_font() -> &'static str {
    if cfg!(target_os = "windows") {
        "Segoe UI"
    } else if cfg!(target_os = "macos") {
        ".AppleSystemUIFont"
    } else {
        "DejaVu Sans"
    }
}

pub fn system_code_font() -> &'static str {
    if cfg!(target_os = "windows") {
        "Consolas"
    } else if cfg!(target_os = "macos") {
        "Menlo"
    } else {
        "DejaVu Sans Mono"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of `0xRRGGBB`.
    fn luminance(color: u32) -> f64 {
        let channel = |shift: u32| {
            let value = f64::from((color >> shift) & 0xff) / 255.0;
            if value <= 0.039_28 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    fn contrast(foreground: u32, background: u32) -> f64 {
        let (a, b) = (luminance(foreground), luminance(background));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn unit_every_theme_keeps_text_readable() {
        for theme in Theme::ALL {
            let palette = palette(theme);
            for surface in [palette.background, palette.panel, palette.raised] {
                assert!(contrast(palette.text, surface) >= 7.0, "{theme:?} text");
                assert!(contrast(palette.muted, surface) >= 3.0, "{theme:?} muted");
            }
            assert!(
                contrast(palette.on_accent, palette.accent) >= 3.0,
                "{theme:?} primary buttons"
            );
        }
    }

    #[test]
    fn unit_code_colours_stay_readable_on_the_editor_background() {
        use crate::desktop::syntax::Token;
        let tokens = [
            Token::Comment,
            Token::String,
            Token::Keyword,
            Token::Number,
            Token::Constant,
            Token::Function,
            Token::Type,
            Token::Variable,
            Token::Operator,
            Token::Tag,
            Token::Attribute,
            Token::Heading,
            Token::Link,
        ];
        for theme in Theme::ALL {
            let palette = palette(theme);
            for token in tokens {
                let color = palette.syntax.color(token);
                assert!(
                    contrast(color, palette.background) >= 3.0,
                    "{theme:?} {token:?} {color:06x}"
                );
            }
        }
    }

    #[test]
    fn unit_git_and_file_type_colours_stay_readable_in_every_theme() {
        use crate::project::FileStatus;
        for theme in Theme::ALL {
            let palette = palette(theme);
            for status in [
                FileStatus::Modified,
                FileStatus::Added,
                FileStatus::Untracked,
                FileStatus::Deleted,
                FileStatus::Renamed,
                FileStatus::Conflict,
            ] {
                let color = palette.git.color(status);
                assert!(
                    contrast(color, palette.panel) >= 3.0,
                    "{theme:?} {status:?} {color:06x}"
                );
                // On the selected row the reds dip, as VS Code's own do; still clearly legible.
                assert!(
                    contrast(color, palette.active) >= 2.0,
                    "{theme:?} {status:?} {color:06x} selected"
                );
            }
            let ignored = palette.git.ignored;
            assert!(
                contrast(ignored, palette.panel) >= 3.0,
                "{theme:?} ignored {ignored:06x}"
            );
            assert!(
                contrast(ignored, palette.active) >= 2.0,
                "{theme:?} ignored {ignored:06x} selected"
            );
            // Greyed means dimmer than an ordinary name, not one more colour.
            assert!(
                contrast(ignored, palette.panel) < contrast(palette.soft_text, palette.panel),
                "{theme:?} ignored is not dimmed"
            );
        }
    }

    #[test]
    fn unit_file_icon_colours_stay_readable_in_the_explorer() {
        use crate::desktop::file_icons::{FileKind, FolderKind};
        for theme in Theme::ALL {
            let palette = palette(theme);
            for kind in FileKind::ALL {
                let color = kind.color(palette);
                for surface in [palette.panel, palette.background] {
                    assert!(
                        contrast(color, surface) >= 3.0,
                        "{theme:?} {kind:?} {color:06x} on {surface:06x}"
                    );
                }
            }
            for kind in FolderKind::ALL {
                let color = kind.color(palette);
                if kind == FolderKind::Output {
                    // Deliberately dim, but never invisible.
                    assert_ne!(color, palette.panel, "{theme:?}");
                    assert!(
                        contrast(color, palette.panel) < contrast(palette.soft_text, palette.panel),
                        "{theme:?}"
                    );
                } else {
                    assert!(
                        contrast(color, palette.panel) >= 3.0,
                        "{theme:?} {kind:?} {color:06x}"
                    );
                }
            }
        }
    }

    #[test]
    fn unit_disabled_controls_look_dimmed_in_every_theme() {
        for theme in Theme::ALL {
            let palette = palette(theme);
            assert!(
                contrast(palette.disabled_text, palette.disabled_bg)
                    < contrast(palette.text, palette.raised),
                "{theme:?}"
            );
            assert_ne!(palette.disabled_bg, palette.raised, "{theme:?}");
        }
    }

    #[test]
    fn unit_dark_and_light_themes_match_their_names() {
        for theme in Theme::ALL {
            let dark = luminance(palette(theme).background) < 0.2;
            assert_eq!(dark, theme.is_dark(), "{theme:?}");
        }
        assert_eq!(Theme::default(), Theme::GruvboxDark);
        assert_eq!(GRUVBOX_DARK.background, 0x282828);
    }
}
