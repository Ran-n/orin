// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 08:39:25.884327
//+ Revised: 	2026/11/13 07:53:54.194715
// -----------------------------------------------------------------------------

//! Draws the Markdown preview in the Code column, live from the editor's text.
//!
//! The look follows VS Code's built-in Markdown preview (its `markdown.css`): 14 px text on a
//! 22 px line with 26 px side padding, browser heading sizes with normal-weight h1–h3 and a rule
//! under h1, code blocks with 16 px padding on a darker ground, quotes with a 5 px left bar,
//! tables ruled between rows, and bullets that change with depth. Block margins collapse as they
//! do in a browser.

use super::{
    assets::Icon,
    controls::{icon, measured},
    markdown::{self, Block, Inline, LinkTarget, Mark},
    syntax::{Highlighter, Token},
    theme,
    workspace::Workspace,
};
use gpui::{
    AnyElement, Context, FontStyle, FontWeight, HighlightStyle, InteractiveText,
    StrikethroughStyle, StyledText, WeakEntity, div, prelude::*, px, relative, rgb,
};
use std::path::{Path, PathBuf};

/// The preview's body text size and line height, as in VS Code.
const BODY_TEXT: f32 = 14.0;
const BODY_LINE: f32 = 22.0;
const SIDE_PADDING: f32 = 26.0;
/// A paragraph's margin: 1em.
const PARAGRAPH_MARGIN: f32 = BODY_TEXT;
const CODE_PADDING: f32 = 16.0;
const CODE_RADIUS: f32 = 3.0;
/// VS Code sets code lines at 1.357em.
const CODE_LINE: f32 = 19.0;
const QUOTE_BAR: f32 = 5.0;
/// Where list text starts: the browser's 40 px list indent.
const LIST_INDENT: f32 = 40.0;

/// What blocks need while drawing: the file links are relative to, the workspace, and how deep
/// in lists they sit.
#[derive(Clone)]
struct RenderContext<'a> {
    file: &'a Path,
    view: WeakEntity<Workspace>,
    list_depth: usize,
}

/// Preview colours, derived from the theme the way VS Code derives them from its own.
struct PreviewColors {
    text: u32,
    rule: u32,
    code_block: u32,
    quote_ground: u32,
    quote_bar: u32,
    inline_code: u32,
    inline_code_ground: u32,
    link: u32,
    muted: u32,
    accent: u32,
}

/// `color` laid over `base` at `alpha` (0–1), as a solid colour.
fn over(base: u32, color: u32, alpha: f32) -> u32 {
    let channel = |value: u32, shift: u32| ((value >> shift) & 0xff) as f32;
    [16, 8, 0].iter().fold(0, |mixed, &shift| {
        let blended = channel(base, shift) * (1. - alpha) + channel(color, shift) * alpha;
        mixed | ((blended.round() as u32).min(255) << shift)
    })
}

fn preview_colors() -> PreviewColors {
    let colors = theme::colors();
    let dark = theme::active_theme().is_dark();
    let ink = if dark { 0xffffff } else { 0x000000 };
    PreviewColors {
        text: colors.text,
        // VS Code: rgba(255, 255, 255, 0.18) on dark themes, rgba(0, 0, 0, 0.18) on light ones.
        rule: over(colors.background, ink, 0.18),
        // textCodeBlock.background: rgba(10, 10, 10, 0.4) dark, rgba(220, 220, 220, 0.4) light.
        code_block: over(
            colors.background,
            if dark { 0x0a0a0a } else { 0xdcdcdc },
            0.4,
        ),
        // textBlockQuote.background: rgba(127, 127, 127, 0.1); its border is the accent at 50%.
        quote_ground: over(colors.background, 0x7f7f7f, 0.1),
        quote_bar: over(colors.background, colors.accent, 0.5),
        inline_code: colors.walnut,
        inline_code_ground: over(colors.background, ink, 0.1),
        link: colors.syntax.link,
        muted: colors.muted,
        accent: colors.accent,
    }
}

impl Workspace {
    /// The rendered form of `source`, the Markdown file at `file` (project-relative).
    pub fn markdown_preview(&self, file: &Path, source: &str, cx: &Context<Self>) -> AnyElement {
        let blocks = self.markdown_cache.blocks(source);
        let context = RenderContext {
            file,
            view: cx.weak_entity(),
            list_depth: 0,
        };
        let colors = preview_colors();
        div()
            .id("markdown-preview")
            .relative()
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(theme::colors().background))
            .child(
                theme::column()
                    .px(theme::scaled(SIDE_PADDING))
                    .pb(theme::scaled(PARAGRAPH_MARGIN))
                    .text_size(theme::scaled(BODY_TEXT))
                    .line_height(theme::scaled(BODY_LINE))
                    .text_color(rgb(colors.text))
                    .children(if blocks.is_empty() {
                        vec![
                            div()
                                .mt(theme::scaled(PARAGRAPH_MARGIN))
                                .text_color(rgb(colors.muted))
                                .child("This file is empty.")
                                .into_any_element(),
                        ]
                    } else {
                        render_blocks(&blocks, "md", &context, true)
                    }),
            )
            .child(measured("markdown-preview", true, &self.controls))
            .into_any_element()
    }
}

/// A heading's text size and margin in pixels at the base text size; browser defaults.
fn heading_metrics(level: u8) -> (f32, f32) {
    let (size, margin): (f32, f32) = match level {
        1 => (2.0, 0.67),
        2 => (1.5, 0.83),
        3 => (1.17, 1.0),
        4 => (1.0, 1.33),
        5 => (0.83, 1.67),
        _ => (0.67, 2.33),
    };
    (BODY_TEXT * size, BODY_TEXT * size * margin)
}

/// A block's (top, bottom) margins in pixels at the base text size.
fn margins(block: &Block) -> (f32, f32) {
    match block {
        Block::Heading(level, _) => {
            let (_, margin) = heading_metrics(*level);
            (margin, margin)
        }
        // A table has no margin of its own; its neighbours' margins separate it.
        Block::Table(_) => (0., 0.),
        // hr: 0.5em.
        Block::Rule => (BODY_TEXT / 2., BODY_TEXT / 2.),
        _ => (PARAGRAPH_MARGIN, PARAGRAPH_MARGIN),
    }
}

/// `outermost` blocks keep their first top and last bottom margin; nested ones (inside a quote
/// or list item) let them collapse out of their container, as CSS margins do.
fn render_blocks(
    blocks: &[Block],
    id: &str,
    context: &RenderContext<'_>,
    outermost: bool,
) -> Vec<AnyElement> {
    let in_list = context.list_depth > 0 && !outermost;
    let mut previous_bottom = None;
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let (top, bottom) =
                if in_list && matches!(block, Block::Paragraph(_) | Block::List { .. }) {
                    // Tight list items show their text without paragraph or nested-list margins.
                    (0., 0.)
                } else {
                    margins(block)
                };
            let space_above = match previous_bottom {
                Some(previous) => f32::max(previous, top),
                None if outermost => top,
                None => 0.,
            };
            previous_bottom = Some(bottom);
            let last = index + 1 == blocks.len();
            div()
                .mt(theme::scaled(space_above))
                .when(last && outermost, |element| {
                    element.mb(theme::scaled(bottom))
                })
                .child(render_block(block, &format!("{id}.{index}"), context))
                .into_any_element()
        })
        .collect()
}

fn render_block(block: &Block, id: &str, context: &RenderContext<'_>) -> AnyElement {
    let colors = preview_colors();
    match block {
        Block::Heading(level, inline) => {
            let (size, _) = heading_metrics(*level);
            div()
                .text_size(theme::scaled(size))
                .line_height(relative(if *level == 1 { 1.2 } else { 1.25 }))
                // VS Code draws h1–h3 at normal weight; h4–h6 keep the browser's bold.
                .when(*level > 3, |heading| heading.font_weight(FontWeight::BOLD))
                .when(*level == 1, |heading| {
                    heading
                        .pb(theme::scaled(size * 0.3))
                        .border_b_1()
                        .border_color(rgb(colors.rule))
                })
                .child(render_inline(inline, id, context))
                .into_any_element()
        }
        Block::Paragraph(inline) => div()
            .child(render_inline(inline, id, context))
            .into_any_element(),
        Block::Code { language, code } => div()
            .id(gpui::SharedString::from(format!("{id}-code")))
            .overflow_x_scroll()
            .p(theme::scaled(CODE_PADDING))
            .rounded(theme::scaled(CODE_RADIUS))
            .bg(rgb(colors.code_block))
            .font_family(theme::code_font())
            .text_size(theme::scaled(BODY_TEXT))
            .line_height(theme::scaled(CODE_LINE))
            .children(code_lines(language.as_deref(), code))
            .into_any_element(),
        Block::Quote(blocks) => theme::column()
            // margin: 0 7px 0 5px; padding: 0 16px 0 10px; a 5 px bar on the left.
            .ml(theme::scaled(5.))
            .mr(theme::scaled(7.))
            .pl(theme::scaled(10.))
            .pr(theme::scaled(16.))
            .border_l(theme::scaled(QUOTE_BAR))
            .border_color(rgb(colors.quote_bar))
            .bg(rgb(colors.quote_ground))
            .children(render_blocks(blocks, id, context, false))
            .into_any_element(),
        Block::List { start, items } => {
            let nested = RenderContext {
                list_depth: context.list_depth + 1,
                ..context.clone()
            };
            theme::column()
                .children(items.iter().enumerate().map(|(index, item)| {
                    let marker = match (item.checked, start) {
                        // Task boxes are drawn as icons so ticked and empty ones match on every font.
                        (Some(true), _) => icon(Icon::Checked, colors.accent).into_any_element(),
                        (Some(false), _) => icon(Icon::Unchecked, colors.muted).into_any_element(),
                        (None, Some(first)) => {
                            format!("{}.", first + index as u64).into_any_element()
                        }
                        (None, None) => bullet(context.list_depth).into_any_element(),
                    };
                    div()
                        .flex()
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .w(theme::scaled(LIST_INDENT))
                                .h(theme::scaled(BODY_LINE))
                                .items_center()
                                .justify_end()
                                .pr(theme::scaled(6.))
                                .child(marker),
                        )
                        .child(theme::column().flex_1().min_w_0().children(render_blocks(
                            &item.blocks,
                            &format!("{id}.{index}"),
                            &nested,
                            false,
                        )))
                }))
                .into_any_element()
        }
        Block::Table(rows) => theme::column()
            .children(rows.iter().enumerate().map(|(row_index, row)| {
                div()
                    .flex()
                    // th: bold, aligned to the start, ruled below; rows after the first are ruled above.
                    .when(row_index == 0, |header| {
                        header
                            .font_weight(FontWeight::BOLD)
                            .border_b_1()
                            .border_color(rgb(colors.rule))
                    })
                    .when(row_index > 1, |line| {
                        line.border_t_1().border_color(rgb(colors.rule))
                    })
                    .children(row.iter().enumerate().map(|(cell_index, cell)| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .px(theme::scaled(10.))
                            .py(theme::scaled(5.))
                            .child(render_inline(
                                cell,
                                &format!("{id}.{row_index}.{cell_index}"),
                                context,
                            ))
                    }))
            }))
            .into_any_element(),
        // hr: a 2 px rule.
        Block::Rule => div()
            .h(theme::scaled(2.))
            .w_full()
            .bg(rgb(colors.rule))
            .into_any_element(),
    }
}

/// Disc, circle, then square, as browsers mark nested bullet lists.
fn bullet(depth: usize) -> &'static str {
    match depth {
        0 => "•",
        1 => "◦",
        _ => "▪",
    }
}

/// The code's lines, coloured by the fence's language like the editor colours files.
fn code_lines(language: Option<&str>, code: &str) -> Vec<AnyElement> {
    let colors = theme::colors();
    let lines: Vec<&str> = code.split('\n').collect();
    let mut highlighter = language.and_then(Highlighter::for_language);
    let tokens: Vec<Vec<(std::ops::Range<usize>, Token)>> = highlighter
        .as_mut()
        .map(|highlighter| {
            highlighter
                .highlight(&lines, usize::MAX)
                .into_iter()
                .map(<[_]>::to_vec)
                .collect()
        })
        .unwrap_or_default();
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let highlights = tokens
                .get(index)
                .into_iter()
                .flatten()
                .map(|(range, token)| {
                    (
                        range.clone(),
                        HighlightStyle {
                            color: Some(rgb(colors.syntax.color(*token)).into()),
                            font_style: (*token == Token::Comment).then_some(FontStyle::Italic),
                            ..HighlightStyle::default()
                        },
                    )
                });
            div()
                .whitespace_nowrap()
                .min_h(theme::scaled(CODE_LINE))
                .child(StyledText::new((*line).to_owned()).with_highlights(highlights))
                .into_any_element()
        })
        .collect()
}

fn highlight(marks: &[Mark]) -> HighlightStyle {
    let colors = preview_colors();
    let mut style = HighlightStyle::default();
    for mark in marks {
        match mark {
            Mark::Strong => style.font_weight = Some(FontWeight::BOLD),
            Mark::Emphasis => style.font_style = Some(FontStyle::Italic),
            Mark::Strike => {
                style.strikethrough = Some(StrikethroughStyle {
                    thickness: px(1.),
                    color: None,
                });
            }
            Mark::Code => {
                style.background_color = Some(rgb(colors.inline_code_ground).into());
                style.color = Some(rgb(colors.inline_code).into());
            }
            // VS Code links are coloured, not underlined.
            Mark::Link(_) => style.color = Some(rgb(colors.link).into()),
        }
    }
    style
}

fn render_inline(inline: &Inline, id: &str, context: &RenderContext<'_>) -> AnyElement {
    let segments = inline.segments();
    let text = StyledText::new(inline.text.clone()).with_highlights(
        segments
            .iter()
            .map(|(range, marks)| (range.clone(), highlight(marks))),
    );
    let links: Vec<(std::ops::Range<usize>, String)> = inline
        .marks
        .iter()
        .filter_map(|(range, mark)| match mark {
            Mark::Link(index) => inline
                .links
                .get(*index)
                .map(|url| (range.clone(), url.clone())),
            _ => None,
        })
        .collect();
    if links.is_empty() {
        return text.into_any_element();
    }
    let file = context.file.to_path_buf();
    let view = context.view.clone();
    let ranges = links.iter().map(|(range, _)| range.clone()).collect();
    InteractiveText::new(gpui::SharedString::from(id.to_owned()), text)
        .on_click(ranges, move |index, _, cx| {
            let Some((_, destination)) = links.get(index) else {
                return;
            };
            open_link(&file, destination, &view, cx);
        })
        .into_any_element()
}

fn open_link(file: &Path, destination: &str, view: &WeakEntity<Workspace>, cx: &mut gpui::App) {
    match markdown::link_target(file, destination) {
        LinkTarget::External(url) => cx.open_url(&url),
        LinkTarget::File(path) => {
            let path: PathBuf = path;
            if let Err(error) = view.update(cx, |view, cx| view.select_file(path, cx)) {
                tracing::warn!(target: "orin::markdown", %error, "link clicked after the workspace closed");
            }
        }
        LinkTarget::None => {
            tracing::debug!(target: "orin::markdown", destination, "link has nothing to open");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_colours_are_laid_over_their_ground() {
        assert_eq!(over(0x000000, 0xffffff, 0.), 0x000000);
        assert_eq!(over(0x000000, 0xffffff, 1.), 0xffffff);
        assert_eq!(over(0x000000, 0xffffff, 0.5), 0x808080);
        assert_eq!(over(0x282828, 0x0a0a0a, 0.4), 0x1c1c1c);
    }

    #[test]
    fn unit_margins_follow_the_browser_defaults_vs_code_keeps() {
        assert_eq!(heading_metrics(1), (28., 28. * 0.67));
        assert_eq!(heading_metrics(2).0, 21.);
        assert_eq!(margins(&Block::Rule), (7., 7.));
        assert_eq!(margins(&Block::Table(Vec::new())), (0., 0.));
        assert_eq!(bullet(0), "•");
        assert_eq!(bullet(1), "◦");
        assert_eq!(bullet(5), "▪");
    }
}
