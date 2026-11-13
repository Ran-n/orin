// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:39:10.329295
//+ Revised: 	2026/11/13 07:53:53.225744
// -----------------------------------------------------------------------------

//! Text shaping, line numbers, selection, and caret share the same native geometry.

use super::{super::theme, TextEditor};
use gpui::{
    Bounds, ElementInputHandler, Entity, Pixels, TextRun, canvas, fill, point, prelude::*, px, rgb,
    rgba, size,
};

/// Editor geometry at the active text size, shared by painting and pointer hit-testing.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub line: f32,
    pub top: f32,
    /// Left edge of the text: past the line numbers when a file is open.
    pub left: f32,
    pub number_left: f32,
}

impl Geometry {
    pub fn current(numbered: bool) -> Self {
        Self {
            line: theme::metric(theme::CODE_LINE),
            top: theme::metric(theme::CODE_PADDING),
            left: theme::metric(if numbered {
                theme::GUTTER
            } else {
                theme::CODE_PADDING
            }),
            number_left: theme::metric(theme::CODE_PADDING),
        }
    }
}

/// One line of the text: where it starts, its length without the line break, and its shaped
/// form when it was on screen at the last layout.
pub struct EditorLine {
    pub offset: usize,
    pub len: usize,
    pub shaped: Option<gpui::ShapedLine>,
}

pub fn text_canvas(entity: Entity<TextEditor>) -> impl IntoElement + gpui::Styled {
    canvas(
        move |bounds, window, cx| {
            entity.update(cx, |view, cx| {
                let text = view.buffer().text().to_owned();
                let mut offset = 0;
                let mut lines = vec![];
                let style = window.text_style();
                let sources: Vec<&str> = text
                    .split('\n')
                    .map(|line| line.trim_end_matches('\r'))
                    .collect();
                let geometry = Geometry::current(view.document().is_some());
                // Highlight only down to the last line on screen; the rest is drawn later.
                let shown = ((f32::from(window.viewport_size().height - bounds.top())
                    - geometry.top)
                    / geometry.line)
                    .max(0.) as usize;
                // Only rows inside the window are shaped: a long file costs the same per frame
                // as a short one. One extra row each side covers a partly visible line.
                let first =
                    ((f32::from(-bounds.top()) - geometry.top) / geometry.line).max(0.) as usize;
                let visible = first.saturating_sub(1)..=shown.saturating_add(1);
                let tokens = view
                    .highlighter
                    .as_mut()
                    .map(|highlighter| highlighter.highlight(&sources, shown))
                    .unwrap_or_default();
                for (index, (line, raw)) in sources.iter().zip(text.split('\n')).enumerate() {
                    let runs = code_runs(
                        line,
                        tokens.get(index).copied().unwrap_or_default(),
                        &style.font(),
                    );
                    let shaped = visible.contains(&index).then(|| {
                        window.text_system().shape_line(
                            (*line).to_owned().into(),
                            theme::scaled(theme::CODE_TEXT),
                            &runs,
                            None,
                        )
                    });
                    lines.push(EditorLine {
                        offset,
                        len: line.len(),
                        shaped,
                    });
                    offset += raw.len() + 1;
                }
                view.bounds = Some(bounds);
                view.lines = lines;
                let _ = cx;
            });
            entity
        },
        move |bounds, entity, window, cx| {
            let input = entity.clone();
            entity.update(cx, |view, cx| {
                let geometry = Geometry::current(view.document().is_some());
                let line_height = px(geometry.line);
                window.handle_input(&view.focus, ElementInputHandler::new(bounds, input), cx);
                let selection = view.buffer().selection();
                for (index, entry) in view.lines.iter().enumerate() {
                    let Some(line) = &entry.shaped else {
                        continue;
                    };
                    let offset = &entry.offset;
                    let origin = point(
                        bounds.left() + px(geometry.left),
                        bounds.top() + px(geometry.top + index as f32 * geometry.line),
                    );
                    if origin.y + line_height < px(0.) || origin.y > window.viewport_size().height {
                        continue;
                    }
                    paint_selection(
                        bounds,
                        origin,
                        line_height,
                        *offset,
                        line,
                        &selection,
                        window,
                    );
                    if let Err(error) = line.paint(origin, line_height, window, cx) {
                        tracing::warn!(%error,"Editor line could not be painted");
                    }
                    if view.document().is_some() {
                        paint_number(
                            index + 1,
                            point(bounds.left() + px(geometry.number_left), origin.y),
                            line_height,
                            window,
                            cx,
                        );
                    }
                    if selection.is_empty()
                        && view.focus.is_focused(window)
                        && (*offset..=*offset + line.text.len()).contains(&selection.end)
                    {
                        let x = origin.x + line.x_for_index(selection.end - *offset);
                        window.paint_quad(fill(
                            Bounds::new(
                                point(x, origin.y),
                                size(px(1.5), theme::scaled(theme::CARET_HEIGHT)),
                            ),
                            rgb(theme::colors().accent),
                        ));
                    }
                }
                if view.document().is_none() && view.buffer().text().is_empty() {
                    paint_placeholder(
                        point(
                            bounds.left() + px(geometry.left),
                            bounds.top() + px(geometry.top),
                        ),
                        line_height,
                        window,
                        cx,
                    );
                }
            });
        },
    )
}

/// Text runs for one line: plain text in the theme's text colour, tokens in their code colours,
/// and comments in italics, as in VS Code.
pub fn code_runs(
    line: &str,
    tokens: &[(std::ops::Range<usize>, super::super::syntax::Token)],
    font: &gpui::Font,
) -> Vec<TextRun> {
    let colors = theme::colors();
    let run = |len: usize, color: u32, italic: bool| {
        let mut font = font.clone();
        if italic {
            font.style = gpui::FontStyle::Italic;
        }
        TextRun {
            len,
            font,
            color: rgb(color).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    };
    let mut runs = Vec::new();
    let mut position = 0;
    for (range, token) in tokens {
        if range.start > position {
            runs.push(run(range.start - position, colors.text, false));
        }
        runs.push(run(
            range.len(),
            colors.syntax.color(*token),
            *token == super::super::syntax::Token::Comment,
        ));
        position = range.end;
    }
    if position < line.len() || runs.is_empty() {
        runs.push(run(line.len() - position, colors.text, false));
    }
    runs
}

fn paint_selection(
    bounds: Bounds<Pixels>,
    origin: gpui::Point<Pixels>,
    line_height: Pixels,
    offset: usize,
    line: &gpui::ShapedLine,
    selection: &std::ops::Range<usize>,
    window: &mut gpui::Window,
) {
    if selection.start < offset + line.text.len() && selection.end > offset {
        let start = selection.start.saturating_sub(offset).min(line.text.len());
        let end = selection.end.saturating_sub(offset).min(line.text.len());
        window.paint_quad(fill(
            Bounds::from_corners(
                point(origin.x + line.x_for_index(start), origin.y),
                point(
                    (origin.x + line.x_for_index(end)).min(bounds.right()),
                    origin.y + line_height,
                ),
            ),
            rgba(theme::colors().selection),
        ));
    }
}
fn paint_number(
    number: usize,
    origin: gpui::Point<Pixels>,
    line_height: Pixels,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    paint_label(number.to_string(), origin, line_height, window, cx);
}

fn paint_placeholder(
    origin: gpui::Point<Pixels>,
    line_height: Pixels,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    paint_label(
        "Give this agent an intention…".into(),
        origin,
        line_height,
        window,
        cx,
    );
}

fn paint_label(
    text: String,
    origin: gpui::Point<Pixels>,
    line_height: Pixels,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    let run = TextRun {
        len: text.len(),
        font: window.text_style().font(),
        color: rgb(theme::colors().muted).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    if let Err(error) = window
        .text_system()
        .shape_line(
            text.into(),
            theme::scaled(theme::LINE_NUMBER_TEXT),
            &[run],
            None,
        )
        .paint(origin, line_height, window, cx)
    {
        tracing::warn!(%error,"Editor label could not be painted");
    }
}
