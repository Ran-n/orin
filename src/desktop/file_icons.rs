// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 22:43:37.001063
//+ Revised: 	2026/11/13 07:53:54.067408
// -----------------------------------------------------------------------------

//! File-type and folder-role icons for the Files column and the Code header.
//!
//! Every glyph is a monochrome Lucide icon tinted with an existing palette colour, so the four
//! themes stay coherent with their code colours. Rules are data tables: one place to add a type.

use super::{
    assets::Icon,
    theme::{self, Palette},
};

/// What kind of file a name denotes, by well-known name first and then by extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
    Readme,
    License,
    Changelog,
    Contributing,
    People,
    Git,
    Lock,
    Build,
    Code,
    Web,
    Script,
    Document,
    Config,
    Data,
    Image,
    Archive,
    Secret,
    Plain,
}

/// Well-known stems: they match the whole name, the part before the first `.`, or a `-` variant
/// (`LICENSE-MIT`).
const STEM_RULES: &[(&str, FileKind)] = &[
    ("readme", FileKind::Readme),
    ("license", FileKind::License),
    ("licence", FileKind::License),
    ("copying", FileKind::License),
    ("changelog", FileKind::Changelog),
    ("changes", FileKind::Changelog),
    ("history", FileKind::Changelog),
    ("contributing", FileKind::Contributing),
    ("contributors", FileKind::People),
    ("authors", FileKind::People),
    ("codeowners", FileKind::People),
    ("makefile", FileKind::Build),
    ("justfile", FileKind::Build),
    ("dockerfile", FileKind::Build),
    ("containerfile", FileKind::Build),
];

/// Names that only match exactly.
const NAME_RULES: &[(&str, FileKind)] = &[
    (".gitignore", FileKind::Git),
    (".gitattributes", FileKind::Git),
    (".gitmodules", FileKind::Git),
    (".gitkeep", FileKind::Git),
    (".mailmap", FileKind::Git),
    ("package-lock.json", FileKind::Lock),
    ("pnpm-lock.yaml", FileKind::Lock),
    ("npm-shrinkwrap.json", FileKind::Lock),
];

/// Matched on the last extension (`archive.tar.gz` → `gz`); a dotfile's whole name after the dot
/// counts as its extension (`.env` → `env`).
const EXTENSION_RULES: &[(&[&str], FileKind)] = &[
    (&["lock"], FileKind::Lock),
    (
        &[
            "rs", "py", "js", "mjs", "cjs", "ts", "tsx", "jsx", "go", "c", "h", "cpp", "cc", "hpp",
            "cs", "java", "kt", "swift", "rb", "php", "lua", "zig", "hs", "dart", "scala", "sql",
        ],
        FileKind::Code,
    ),
    (
        &[
            "html", "htm", "css", "scss", "sass", "less", "vue", "svelte",
        ],
        FileKind::Web,
    ),
    (
        &["sh", "bash", "zsh", "fish", "ps1", "psm1", "bat", "cmd"],
        FileKind::Script,
    ),
    (
        &["md", "markdown", "mdx", "rst", "adoc", "txt", "pdf"],
        FileKind::Document,
    ),
    (
        &[
            "toml",
            "yaml",
            "yml",
            "ini",
            "cfg",
            "conf",
            "env",
            "editorconfig",
            "properties",
        ],
        FileKind::Config,
    ),
    (
        &["json", "jsonc", "json5", "xml", "csv", "tsv"],
        FileKind::Data,
    ),
    (
        &[
            "png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp", "tiff", "avif",
        ],
        FileKind::Image,
    ),
    (
        &["zip", "tar", "gz", "tgz", "xz", "bz2", "7z", "rar", "zst"],
        FileKind::Archive,
    ),
    (
        &["pem", "key", "crt", "cer", "p12", "pfx", "asc", "gpg"],
        FileKind::Secret,
    ),
];

impl FileKind {
    #[cfg(test)]
    pub const ALL: [FileKind; 18] = [
        FileKind::Readme,
        FileKind::License,
        FileKind::Changelog,
        FileKind::Contributing,
        FileKind::People,
        FileKind::Git,
        FileKind::Lock,
        FileKind::Build,
        FileKind::Code,
        FileKind::Web,
        FileKind::Script,
        FileKind::Document,
        FileKind::Config,
        FileKind::Data,
        FileKind::Image,
        FileKind::Archive,
        FileKind::Secret,
        FileKind::Plain,
    ];

    pub fn icon(self) -> Icon {
        match self {
            FileKind::Readme => Icon::Info,
            FileKind::License => Icon::License,
            FileKind::Changelog => Icon::History,
            FileKind::Contributing => Icon::Guide,
            FileKind::People => Icon::People,
            FileKind::Git => Icon::GitBranch,
            FileKind::Lock => Icon::LockFile,
            FileKind::Build => Icon::BuildFile,
            FileKind::Code | FileKind::Web => Icon::Code,
            FileKind::Script => Icon::ScriptFile,
            FileKind::Document => Icon::TextFile,
            FileKind::Config => Icon::ConfigFile,
            FileKind::Data => Icon::DataFile,
            FileKind::Image => Icon::ImageFile,
            FileKind::Archive => Icon::ArchiveFile,
            FileKind::Secret => Icon::KeyFile,
            FileKind::Plain => Icon::File,
        }
    }

    pub fn color(self, palette: &Palette) -> u32 {
        let syntax = &palette.syntax;
        match self {
            FileKind::Readme | FileKind::Document => syntax.link,
            FileKind::License | FileKind::People => palette.error,
            // Gruvbox's grey comment colour is too faint on the light panel, so neither uses it.
            FileKind::Changelog | FileKind::Git => syntax.string,
            FileKind::Script => syntax.function,
            FileKind::Contributing | FileKind::Lock | FileKind::Secret => palette.warning,
            FileKind::Build | FileKind::Config => syntax.type_name,
            FileKind::Code => syntax.keyword,
            // Web shares Code's glyph, so it must never share its colour too.
            FileKind::Web if syntax.tag == syntax.keyword => syntax.string,
            FileKind::Web => syntax.tag,
            FileKind::Data => syntax.function,
            FileKind::Image => syntax.number,
            FileKind::Archive => palette.walnut,
            FileKind::Plain => palette.muted,
        }
    }
}

/// The kind of the file called `name` (its own name, not a path). Pure and allocation-free,
/// ignoring ASCII case: it runs for every visible row on every frame.
pub fn kind_for(name: &str) -> FileKind {
    if let Some(&(_, kind)) = NAME_RULES
        .iter()
        .find(|(rule, _)| rule.eq_ignore_ascii_case(name))
    {
        return kind;
    }
    let by_extension = name.rsplit_once('.').and_then(|(_, extension)| {
        EXTENSION_RULES
            .iter()
            .find(|(extensions, _)| {
                extensions
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(extension))
            })
            .map(|&(_, kind)| kind)
    });
    let stem = name.split('.').next().unwrap_or_default();
    let by_stem = STEM_RULES
        .iter()
        .find(|(rule, _)| {
            // `rule` is ASCII, so its length is a character boundary in a matching stem.
            stem.eq_ignore_ascii_case(rule)
                || stem.get(..rule.len()).is_some_and(|head| {
                    head.eq_ignore_ascii_case(rule) && stem[rule.len()..].starts_with('-')
                })
        })
        .map(|&(_, kind)| kind);
    // Well-known names beat document extensions (`README.md`), but not real code: `history.rs`
    // is source, not a changelog.
    match (by_stem, by_extension) {
        (Some(kind), None | Some(FileKind::Document)) => kind,
        (_, Some(kind)) => kind,
        (None, None)
            if name
                .get(..4)
                .is_some_and(|head| head.eq_ignore_ascii_case(".env")) =>
        {
            FileKind::Config
        }
        (None, None) => FileKind::Plain,
    }
}

/// The glyph and active-theme colour for the file called `name`: the one entry point both views
/// use. The colour is looked up at render time, so theme changes apply on the next frame.
pub fn file_icon(name: &str) -> (Icon, u32) {
    let kind = kind_for(name);
    (kind.icon(), kind.color(theme::colors()))
}

/// The role of a folder, by its own name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FolderKind {
    Source,
    Tests,
    Docs,
    Git,
    Assets,
    Config,
    /// Generated build output; drawn dimmed when Git can't say what is ignored.
    Output,
    Plain,
}

const FOLDER_RULES: &[(&[&str], FolderKind)] = &[
    (&["src", "source", "lib", "app"], FolderKind::Source),
    (&["tests", "test", "spec", "__tests__"], FolderKind::Tests),
    (&["docs", "doc", "documentation"], FolderKind::Docs),
    (&[".git", ".github", ".gitlab", ".forgejo"], FolderKind::Git),
    (
        &["assets", "static", "public", "images", "icons", "resources"],
        FolderKind::Assets,
    ),
    (
        &[".vscode", ".idea", ".claude", ".cargo", "config", ".config"],
        FolderKind::Config,
    ),
    (
        &[
            "target",
            "build",
            "dist",
            "out",
            "node_modules",
            ".venv",
            "venv",
            "__pycache__",
        ],
        FolderKind::Output,
    ),
];

impl FolderKind {
    #[cfg(test)]
    pub const ALL: [FolderKind; 8] = [
        FolderKind::Source,
        FolderKind::Tests,
        FolderKind::Docs,
        FolderKind::Git,
        FolderKind::Assets,
        FolderKind::Config,
        FolderKind::Output,
        FolderKind::Plain,
    ];

    /// Well-known folders keep their glyph when open (Lucide has no open variants of them); the
    /// chevron shows the state.
    pub fn icon(self, open: bool) -> Icon {
        match self {
            FolderKind::Source => Icon::SourceFolder,
            FolderKind::Tests => Icon::TestsFolder,
            FolderKind::Docs => Icon::DocsFolder,
            FolderKind::Git => Icon::GitFolder,
            FolderKind::Assets => Icon::AssetsFolder,
            FolderKind::Config => Icon::ConfigFolder,
            FolderKind::Output => Icon::OutputFolder,
            FolderKind::Plain if open => Icon::OpenFolder,
            FolderKind::Plain => Icon::Folder,
        }
    }

    pub fn color(self, palette: &Palette) -> u32 {
        match self {
            FolderKind::Source => palette.accent,
            FolderKind::Tests => palette.syntax.type_name,
            FolderKind::Docs => palette.syntax.link,
            FolderKind::Git => palette.syntax.string,
            FolderKind::Assets => palette.syntax.number,
            FolderKind::Config => palette.muted,
            FolderKind::Output => palette.faint,
            FolderKind::Plain => palette.walnut,
        }
    }
}

/// The role of the folder called `name`, at any depth, ignoring case.
pub fn folder_kind_for(name: &str) -> FolderKind {
    FOLDER_RULES
        .iter()
        .find(|(names, _)| names.iter().any(|known| known.eq_ignore_ascii_case(name)))
        .map_or(FolderKind::Plain, |&(_, kind)| kind)
}

/// The glyph and active-theme colour for the folder called `name`.
pub fn folder_icon(name: &str, open: bool) -> (Icon, u32) {
    let kind = folder_kind_for(name);
    (kind.icon(open), kind.color(theme::colors()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Theme;

    #[test]
    fn unit_well_known_names_beat_extensions() {
        for (name, kind) in [
            ("README.md", FileKind::Readme),
            ("LICENSE", FileKind::License),
            ("LICENSE-MIT", FileKind::License),
            ("LICENSE.txt", FileKind::License),
            ("CHANGELOG.md", FileKind::Changelog),
            ("CONTRIBUTING.md", FileKind::Contributing),
            ("CONTRIBUTORS", FileKind::People),
            ("Cargo.lock", FileKind::Lock),
            ("package-lock.json", FileKind::Lock),
            (".gitignore", FileKind::Git),
            ("Dockerfile", FileKind::Build),
            ("Dockerfile.dev", FileKind::Build),
        ] {
            assert_eq!(kind_for(name), kind, "{name}");
        }
    }

    #[test]
    fn unit_code_extensions_beat_well_known_stems() {
        assert_eq!(kind_for("history.rs"), FileKind::Code);
        assert_eq!(kind_for("authors.json"), FileKind::Data);
    }

    #[test]
    fn unit_extensions_choose_the_category() {
        for (name, kind) in [
            ("main.rs", FileKind::Code),
            ("index.html", FileKind::Web),
            ("run.ps1", FileKind::Script),
            ("notes.txt", FileKind::Document),
            ("rust-toolchain.toml", FileKind::Config),
            ("data.json", FileKind::Data),
            ("logo.svg", FileKind::Image),
            ("x.tar.gz", FileKind::Archive),
            ("id.pem", FileKind::Secret),
        ] {
            assert_eq!(kind_for(name), kind, "{name}");
        }
    }

    #[test]
    fn unit_matching_ignores_case() {
        assert_eq!(kind_for("README.MD"), FileKind::Readme);
        assert_eq!(kind_for("readme"), FileKind::Readme);
        assert_eq!(kind_for("Photo.PNG"), FileKind::Image);
        assert_eq!(kind_for("MAKEFILE"), FileKind::Build);
    }

    #[test]
    fn unit_dotfiles_and_unknown_names() {
        assert_eq!(kind_for(".env"), FileKind::Config);
        assert_eq!(kind_for(".env.local"), FileKind::Config);
        assert_eq!(kind_for(".editorconfig"), FileKind::Config);
        for name in ["notes", "a.unknownext", "", "."] {
            assert_eq!(kind_for(name), FileKind::Plain, "{name:?}");
        }
    }

    #[test]
    fn unit_every_kind_has_a_glyph_in_the_asset_table() {
        for kind in FileKind::ALL {
            assert!(Icon::ALL.contains(&kind.icon()), "{kind:?}");
        }
        for kind in FolderKind::ALL {
            for open in [false, true] {
                assert!(Icon::ALL.contains(&kind.icon(open)), "{kind:?}");
            }
        }
    }

    #[test]
    fn unit_kinds_are_distinguishable_in_every_theme() {
        for theme in Theme::ALL {
            let palette = theme::palette(theme);
            let looks: Vec<(Icon, u32)> = FileKind::ALL
                .iter()
                .map(|kind| (kind.icon(), kind.color(palette)))
                .collect();
            for (index, look) in looks.iter().enumerate() {
                assert!(
                    !looks[index + 1..].contains(look),
                    "{theme:?}: {:?} looks like another kind",
                    FileKind::ALL[index]
                );
            }
        }
    }

    #[test]
    fn unit_well_known_folders() {
        for (name, kind) in [
            ("src", FolderKind::Source),
            ("SRC", FolderKind::Source),
            ("Tests", FolderKind::Tests),
            ("tests", FolderKind::Tests),
            ("docs", FolderKind::Docs),
            (".github", FolderKind::Git),
            ("assets", FolderKind::Assets),
            (".cargo", FolderKind::Config),
            ("target", FolderKind::Output),
            ("node_modules", FolderKind::Output),
            ("misc", FolderKind::Plain),
        ] {
            assert_eq!(folder_kind_for(name), kind, "{name}");
        }
    }

    #[test]
    fn unit_plain_folder_opens() {
        assert_eq!(FolderKind::Plain.icon(false), Icon::Folder);
        assert_eq!(FolderKind::Plain.icon(true), Icon::OpenFolder);
        assert_eq!(folder_icon("misc", true).0, Icon::OpenFolder);
    }

    #[test]
    fn unit_well_known_folders_keep_their_glyph_when_open() {
        for kind in FolderKind::ALL {
            if kind != FolderKind::Plain {
                assert_eq!(kind.icon(true), kind.icon(false), "{kind:?}");
            }
        }
    }
}
