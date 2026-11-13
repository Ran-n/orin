// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 08:39:25.563142
//+ Revised: 	2026/11/13 07:53:54.133091
// -----------------------------------------------------------------------------

//! Markdown parsed into a small block model for the Code column's preview.
//!
//! The model is free of GPUI so it can be tested on its own; `markdown_view` draws it.
//! Raw HTML is left out of the preview, as it can't be shown faithfully without a browser engine.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::{
    ops::Range,
    path::{Component, Path, PathBuf},
};

/// Inline styling over a byte range of an [`Inline`]'s text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Strong,
    Emphasis,
    Code,
    Strike,
    /// Index into [`Inline::links`].
    Link(usize),
}

/// One run of text with its marks, such as a paragraph, heading, or table cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inline {
    pub text: String,
    pub marks: Vec<(Range<usize>, Mark)>,
    pub links: Vec<String>,
}

impl Inline {
    /// Split the text where any mark starts or ends, so each piece has one set of marks.
    pub fn segments(&self) -> Vec<(Range<usize>, Vec<Mark>)> {
        let mut edges: Vec<usize> = self
            .marks
            .iter()
            .flat_map(|(range, _)| [range.start, range.end])
            .chain([0, self.text.len()])
            .collect();
        edges.sort_unstable();
        edges.dedup();
        edges
            .windows(2)
            .filter_map(|pair| {
                let range = pair[0]..pair[1];
                let marks: Vec<Mark> = self
                    .marks
                    .iter()
                    .filter(|(mark, _)| mark.start <= range.start && range.end <= mark.end)
                    .map(|(_, mark)| *mark)
                    .collect();
                (!marks.is_empty()).then_some((range, marks))
            })
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    /// `Some` for a task-list item, telling whether it is ticked.
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading(u8, Inline),
    Paragraph(Inline),
    /// A code block; `language` is the first word of a fence's info string, if any.
    Code {
        language: Option<String>,
        code: String,
    },
    Quote(Vec<Block>),
    /// `start` is the first number of an ordered list, `None` for bullets.
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    /// The first row is the header.
    Table(Vec<Vec<Inline>>),
    Rule,
}

enum Frame {
    Blocks(Vec<Block>),
    Quote(Vec<Block>),
    List(Option<u64>, Vec<Item>),
    Item(Item),
    Table(Vec<Vec<Inline>>, Vec<Inline>),
}

enum InlineKind {
    Paragraph,
    /// Text placed directly in a tight list item, without a paragraph around it.
    Loose,
    Heading(u8),
    Cell,
}

/// The inline run being built, with the marks opened in it but not yet closed.
struct OpenInline {
    kind: InlineKind,
    inline: Inline,
    open: Vec<(usize, Mark)>,
}

impl OpenInline {
    fn new(kind: InlineKind) -> Self {
        Self {
            kind,
            inline: Inline::default(),
            open: Vec::new(),
        }
    }

    fn open(&mut self, mark: Mark) {
        self.open.push((self.inline.text.len(), mark));
    }
}

struct Builder {
    frames: Vec<Frame>,
    inline: Option<OpenInline>,
    code: Option<(Option<String>, String)>,
}

impl Builder {
    fn push_block(&mut self, block: Block) {
        match self.frames.last_mut() {
            Some(Frame::Blocks(blocks) | Frame::Quote(blocks)) => blocks.push(block),
            Some(Frame::Item(item)) => item.blocks.push(block),
            Some(Frame::List(..) | Frame::Table(..)) | None => {
                tracing::debug!(target: "orin::markdown", "block outside a container was dropped");
            }
        }
    }

    fn begin_inline(&mut self, kind: InlineKind) {
        self.finish_inline();
        self.inline = Some(OpenInline::new(kind));
    }

    fn inline(&mut self) -> &mut OpenInline {
        self.inline
            .get_or_insert_with(|| OpenInline::new(InlineKind::Loose))
    }

    fn finish_inline(&mut self) {
        let Some(OpenInline { kind, inline, .. }) = self.inline.take() else {
            return;
        };
        match kind {
            InlineKind::Paragraph | InlineKind::Loose => self.push_block(Block::Paragraph(inline)),
            InlineKind::Heading(level) => self.push_block(Block::Heading(level, inline)),
            InlineKind::Cell => {
                if let Some(Frame::Table(_, row)) = self.frames.last_mut() {
                    row.push(inline);
                }
            }
        }
    }

    fn text(&mut self, text: &str) {
        if let Some((_, code)) = &mut self.code {
            code.push_str(text);
        } else {
            self.inline().inline.text.push_str(text);
        }
    }

    fn open_mark(&mut self, mark: Mark) {
        self.inline().open(mark);
    }

    fn close_mark(&mut self) {
        let OpenInline { inline, open, .. } = self.inline();
        if let Some((start, mark)) = open.pop()
            && start < inline.text.len()
        {
            inline.marks.push((start..inline.text.len(), mark));
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.begin_inline(InlineKind::Paragraph),
            Tag::Heading { level, .. } => self.begin_inline(InlineKind::Heading(level as u8)),
            Tag::BlockQuote(_) => {
                self.finish_inline();
                self.frames.push(Frame::Quote(Vec::new()));
            }
            Tag::CodeBlock(kind) => {
                self.finish_inline();
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().map(str::to_owned)
                    }
                    CodeBlockKind::Indented => None,
                };
                self.code = Some((language, String::new()));
            }
            Tag::List(start) => {
                self.finish_inline();
                self.frames.push(Frame::List(start, Vec::new()));
            }
            Tag::Item => self.frames.push(Frame::Item(Item::default())),
            Tag::Table(_) => {
                self.finish_inline();
                self.frames.push(Frame::Table(Vec::new(), Vec::new()));
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(Frame::Table(_, row)) = self.frames.last_mut() {
                    row.clear();
                }
            }
            Tag::TableCell => self.begin_inline(InlineKind::Cell),
            Tag::Emphasis => self.open_mark(Mark::Emphasis),
            Tag::Strong => self.open_mark(Mark::Strong),
            Tag::Strikethrough => self.open_mark(Mark::Strike),
            // An image shows its description, the closest a text preview can come.
            Tag::Image { .. } => self.open_mark(Mark::Emphasis),
            Tag::Link { dest_url, .. } => {
                let current = self.inline();
                current.inline.links.push(dest_url.into_string());
                current.open(Mark::Link(current.inline.links.len() - 1));
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::TableCell => self.finish_inline(),
            TagEnd::BlockQuote(_) => {
                self.finish_inline();
                if let Some(Frame::Quote(blocks)) = self.frames.pop() {
                    self.push_block(Block::Quote(blocks));
                }
            }
            TagEnd::CodeBlock => {
                if let Some((language, code)) = self.code.take() {
                    self.push_block(Block::Code {
                        language,
                        code: code.trim_end_matches('\n').to_owned(),
                    });
                }
            }
            TagEnd::List(_) => {
                self.finish_inline();
                if let Some(Frame::List(start, items)) = self.frames.pop() {
                    self.push_block(Block::List { start, items });
                }
            }
            TagEnd::Item => {
                self.finish_inline();
                if let Some(Frame::Item(item)) = self.frames.pop()
                    && let Some(Frame::List(_, items)) = self.frames.last_mut()
                {
                    items.push(item);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(Frame::Table(rows, row)) = self.frames.last_mut() {
                    rows.push(std::mem::take(row));
                }
            }
            TagEnd::Table => {
                if let Some(Frame::Table(rows, _)) = self.frames.pop() {
                    self.push_block(Block::Table(rows));
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Image => {
                self.close_mark()
            }
            TagEnd::Link => self.close_mark(),
            _ => {}
        }
    }
}

pub fn parse(source: &str) -> Vec<Block> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut builder = Builder {
        frames: vec![Frame::Blocks(Vec::new())],
        inline: None,
        code: None,
    };
    for event in Parser::new_ext(source, options) {
        match event {
            Event::Start(tag) => builder.start(tag),
            Event::End(tag) => builder.end(tag),
            Event::Text(text) => builder.text(&text),
            Event::Code(code) => {
                builder.open_mark(Mark::Code);
                builder.text(&code);
                builder.close_mark();
            }
            Event::SoftBreak => builder.text(" "),
            Event::HardBreak => builder.text("\n"),
            Event::Rule => {
                builder.finish_inline();
                builder.push_block(Block::Rule);
            }
            Event::TaskListMarker(checked) => {
                if let Some(Frame::Item(item)) = builder.frames.last_mut() {
                    item.checked = Some(checked);
                }
            }
            Event::FootnoteReference(name) => builder.text(&format!("[{name}]")),
            Event::Html(_) | Event::InlineHtml(_) => {}
            Event::InlineMath(text) | Event::DisplayMath(text) => builder.text(&text),
        }
    }
    builder.finish_inline();
    match builder.frames.into_iter().next() {
        Some(Frame::Blocks(blocks)) => blocks,
        _ => Vec::new(),
    }
}

/// The readable text of the blocks, one block per line; used by the UI test driver.
pub fn plain_text(blocks: &[Block]) -> String {
    let mut lines = Vec::new();
    collect_text(blocks, &mut lines);
    lines.join("\n")
}

fn collect_text(blocks: &[Block], lines: &mut Vec<String>) {
    for block in blocks {
        match block {
            Block::Heading(_, inline) | Block::Paragraph(inline) => lines.push(inline.text.clone()),
            Block::Code { code, .. } => lines.push(code.clone()),
            Block::Quote(blocks) => collect_text(blocks, lines),
            Block::List { items, .. } => {
                for item in items {
                    collect_text(&item.blocks, lines);
                }
            }
            Block::Table(rows) => lines.extend(rows.iter().map(|row| {
                row.iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            })),
            Block::Rule => {}
        }
    }
}

/// Where a link in a previewed file leads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    /// A web or mail address, opened by the system.
    External(String),
    /// A project file, relative to the project root, opened in Orin.
    File(PathBuf),
    /// An anchor in the same file or a path outside the project: nothing to open.
    None,
}

/// Resolve `destination` from a link inside `current` (a project-relative file path).
pub fn link_target(current: &Path, destination: &str) -> LinkTarget {
    let lower = destination.to_ascii_lowercase();
    if ["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        return LinkTarget::External(destination.to_owned());
    }
    let path = destination.split(['#', '?']).next().unwrap_or_default();
    if path.is_empty() || lower.contains("://") || lower.starts_with("javascript:") {
        return LinkTarget::None;
    }
    // Links to names with spaces are written `my%20notes.md`, as GitHub renders them.
    let Some(path) = percent_decode(path) else {
        return LinkTarget::None;
    };
    let path = path.as_str();
    let joined = if let Some(rooted) = path.strip_prefix('/') {
        PathBuf::from(rooted)
    } else {
        current.parent().unwrap_or(Path::new("")).join(path)
    };
    let mut resolved = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return LinkTarget::None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return LinkTarget::None,
        }
    }
    if resolved.as_os_str().is_empty() {
        LinkTarget::None
    } else {
        LinkTarget::File(resolved)
    }
}

/// `%XX` escapes decoded to the bytes they stand for; `None` for a broken escape or bytes that
/// aren't UTF-8. A decoded `/` is a separator, as browsers treat it.
fn percent_decode(text: &str) -> Option<String> {
    if !text.contains('%') {
        return Some(text.to_owned());
    }
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// The last parsed source and its blocks. The preview repaints on every frame (a blinking caret,
/// a scroll), and parsing a long file each time is the costliest part of drawing it.
#[derive(Default)]
pub struct ParseCache {
    last: std::cell::RefCell<Option<(u64, std::rc::Rc<[Block]>)>>,
}

impl ParseCache {
    /// The blocks of `source`, parsed again only when the text changed.
    pub fn blocks(&self, source: &str) -> std::rc::Rc<[Block]> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hasher);
        let key = hasher.finish();
        let mut last = self.last.borrow_mut();
        if let Some((cached, blocks)) = last.as_ref()
            && *cached == key
        {
            return blocks.clone();
        }
        let blocks: std::rc::Rc<[Block]> = parse(source).into();
        *last = Some((key, blocks.clone()));
        blocks
    }
}

/// True for the file extensions the preview understands.
pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["md", "markdown", "mdown", "mkd"]
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_the_parse_cache_reuses_blocks_until_the_text_changes() {
        let cache = ParseCache::default();
        let first = cache.blocks("# Title");
        assert!(std::rc::Rc::ptr_eq(&first, &cache.blocks("# Title")));
        let changed = cache.blocks("# Other");
        assert!(!std::rc::Rc::ptr_eq(&first, &changed));
        assert_eq!(&*changed, parse("# Other").as_slice());
    }

    #[test]
    fn regression_percent_escaped_links_open_the_named_file() {
        let current = Path::new("docs/guide.md");
        assert_eq!(
            link_target(current, "my%20notes.md"),
            LinkTarget::File(PathBuf::from("docs/my notes.md"))
        );
        assert_eq!(
            link_target(current, "caf%C3%A9.md#top"),
            LinkTarget::File(PathBuf::from("docs/caf\u{e9}.md"))
        );
        assert_eq!(link_target(current, "bad%2.md"), LinkTarget::None);
        assert_eq!(link_target(current, "bad%FF.md"), LinkTarget::None);
        // An escaped parent step is still a parent step, and can't leave the project.
        assert_eq!(
            link_target(current, "%2E%2E/%2E%2E/secret"),
            LinkTarget::None
        );
    }

    fn paragraph(blocks: &[Block], index: usize) -> &Inline {
        match &blocks[index] {
            Block::Paragraph(inline) => inline,
            other => panic!("expected a paragraph, found {other:?}"),
        }
    }

    #[test]
    fn unit_headings_paragraphs_and_rules() {
        let blocks = parse("# Title\n\nSome text\nwrapped.\n\n---\n\n### Small");
        assert_eq!(blocks.len(), 4);
        assert!(matches!(&blocks[0], Block::Heading(1, inline) if inline.text == "Title"));
        assert_eq!(paragraph(&blocks, 1).text, "Some text wrapped.");
        assert_eq!(blocks[2], Block::Rule);
        assert!(matches!(&blocks[3], Block::Heading(3, inline) if inline.text == "Small"));
    }

    #[test]
    fn unit_inline_marks_cover_their_text() {
        let blocks = parse("A **bold** and *soft* `code` ~~gone~~ [site](https://example.com).");
        let inline = paragraph(&blocks, 0);
        assert_eq!(inline.text, "A bold and soft code gone site.");
        let marked = |mark: Mark| {
            inline
                .marks
                .iter()
                .find(|(_, candidate)| *candidate == mark)
                .map(|(range, _)| &inline.text[range.clone()])
        };
        assert_eq!(marked(Mark::Strong), Some("bold"));
        assert_eq!(marked(Mark::Emphasis), Some("soft"));
        assert_eq!(marked(Mark::Code), Some("code"));
        assert_eq!(marked(Mark::Strike), Some("gone"));
        assert_eq!(marked(Mark::Link(0)), Some("site"));
        assert_eq!(inline.links, ["https://example.com"]);
    }

    #[test]
    fn unit_nested_marks_split_into_flat_segments() {
        let blocks = parse("**bold *both* bold**");
        let segments = paragraph(&blocks, 0).segments();
        assert_eq!(
            segments,
            vec![
                (0..5, vec![Mark::Strong]),
                (5..9, vec![Mark::Emphasis, Mark::Strong]),
                (9..14, vec![Mark::Strong]),
            ]
        );
    }

    #[test]
    fn unit_tight_and_task_lists_keep_their_items() {
        let blocks = parse("- one\n- [x] done\n- [ ] todo\n  - nested\n\n3. three\n4. four");
        let Block::List { start: None, items } = &blocks[0] else {
            panic!("expected a bullet list: {blocks:?}");
        };
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].checked, None);
        assert_eq!(items[1].checked, Some(true));
        assert_eq!(items[2].checked, Some(false));
        assert_eq!(paragraph(&items[0].blocks, 0).text, "one");
        assert!(matches!(&items[2].blocks[1], Block::List { items, .. } if items.len() == 1));
        assert!(matches!(&blocks[1], Block::List { start: Some(3), items } if items.len() == 2));
    }

    #[test]
    fn unit_code_blocks_quotes_and_tables() {
        let source =
            "```rust\nfn main() {}\n```\n\n> quoted **text**\n\n| A | B |\n| - | - |\n| 1 | 2 |\n";
        let blocks = parse(source);
        assert_eq!(
            blocks[0],
            Block::Code {
                language: Some("rust".into()),
                code: "fn main() {}".into()
            }
        );
        let Block::Quote(quoted) = &blocks[1] else {
            panic!("expected a quote: {blocks:?}");
        };
        assert_eq!(paragraph(quoted, 0).text, "quoted text");
        let Block::Table(rows) = &blocks[2] else {
            panic!("expected a table: {blocks:?}");
        };
        let texts: Vec<Vec<&str>> = rows
            .iter()
            .map(|row| row.iter().map(|cell| cell.text.as_str()).collect())
            .collect();
        assert_eq!(texts, vec![vec!["A", "B"], vec!["1", "2"]]);
    }

    #[test]
    fn unit_html_is_left_out_and_images_show_their_description() {
        let blocks = parse("<div align=\"center\">\n\n![Orin logo](logo.png) text<br>\n\n</div>\n");
        assert_eq!(blocks.len(), 1);
        let inline = paragraph(&blocks, 0);
        assert_eq!(inline.text, "Orin logo text");
        assert_eq!(inline.marks, vec![(0..9, Mark::Emphasis)]);
    }

    #[test]
    fn unit_plain_text_reads_every_block() {
        let blocks = parse("# Fixture\n\n- item\n\n| A |\n| - |\n| 1 |\n\n```\ncode\n```");
        assert_eq!(plain_text(&blocks), "Fixture\nitem\nA\n1\ncode");
        assert!(parse("").is_empty());
    }

    #[test]
    fn unit_links_resolve_inside_the_project_only() {
        let current = Path::new("docs/guide.md");
        assert_eq!(
            link_target(current, "https://example.com/x"),
            LinkTarget::External("https://example.com/x".into())
        );
        assert_eq!(
            link_target(current, "../README.md#install"),
            LinkTarget::File("README.md".into())
        );
        assert_eq!(
            link_target(current, "./api.md"),
            LinkTarget::File("docs/api.md".into())
        );
        assert_eq!(
            link_target(current, "/src/main.rs"),
            LinkTarget::File("src/main.rs".into())
        );
        assert_eq!(link_target(current, "#heading"), LinkTarget::None);
        assert_eq!(link_target(current, "../../outside.md"), LinkTarget::None);
        assert_eq!(
            link_target(current, "javascript:alert(1)"),
            LinkTarget::None
        );
        assert_eq!(link_target(current, "ftp://host/file"), LinkTarget::None);
    }

    #[test]
    fn unit_markdown_extensions_are_recognised() {
        for name in ["README.md", "notes.MARKDOWN", "a.mdown", "b.mkd"] {
            assert!(is_markdown(Path::new(name)), "{name}");
        }
        for name in ["main.rs", "md", "README"] {
            assert!(!is_markdown(Path::new(name)), "{name}");
        }
    }
}
