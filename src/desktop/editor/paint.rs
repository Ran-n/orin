// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 17:39:10.329295
//+ Revised: 	2026/11/14 12:09:13.486754
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
    /// A file's line, shaped while it is on screen. Files are never wrapped.
    pub shaped: Option<gpui::ShapedLine>,
    /// A prompt's line, wrapped to the width of its box.
    pub wrapped: Option<gpui::WrappedLine>,
    /// The drawn row this line starts on, counting wrapped rows above it.
    pub row: usize,
    /// How many drawn rows the line takes: one unless it wraps.
    pub rows: usize,
}

impl EditorLine {
    /// The byte ranges of this line's drawn rows, relative to the line.
    pub fn row_ranges(&self) -> Vec<std::ops::Range<usize>> {
        let mut start = 0;
        // An unwrapped line has no boundaries: it is one row, its whole length.
        let mut ranges: Vec<_> = self
            .wrapped
            .iter()
            .flat_map(|wrapped| {
                wrapped
                    .wrap_boundaries()
                    .iter()
                    .map(move |boundary| (wrapped, boundary))
            })
            .map(|(wrapped, boundary)| {
                let end =
                    wrapped.unwrapped_layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix].index;
                let range = start..end;
                start = end;
                range
            })
            .collect();
        ranges.push(start..self.len);
        ranges
    }

    /// Where the caret at `index` of a wrapped line is drawn: its row within the line and its
    /// distance from the row's left edge. `None` for a line that isn't wrapped (a file's).
    ///
    /// Where the line breaks, `index` ends one row and starts the next; `row_start` picks the
    /// next row, otherwise the caret stays at the end of the earlier one.
    pub fn caret_place(&self, index: usize, row_start: bool) -> Option<(usize, Pixels)> {
        let layout = &self.wrapped.as_ref()?.unwrapped_layout;
        let ranges = self.row_ranges();
        let mut row = ranges.iter().position(|range| index <= range.end)?;
        if row_start && index == ranges[row].end && row + 1 < ranges.len() {
            row += 1;
        }
        Some((
            row,
            layout.x_for_index(index) - layout.x_for_index(ranges[row].start),
        ))
    }
}

/// The narrowest a prompt's text is wrapped to, however small its box.
const MIN_WRAP_WIDTH: f32 = 40.0;

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
                // A prompt wraps at the width of its box; a file scrolls sideways instead.
                let wrap_width = view
                    .document()
                    .is_none()
                    .then(|| (bounds.size.width - px(2. * geometry.left)).max(px(MIN_WRAP_WIDTH)));
                let mut row = 0;
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
                    let (shaped, wrapped) = match wrap_width {
                        Some(width) => (None, wrap_line(line, &runs, width, window)),
                        None => (
                            visible.contains(&index).then(|| {
                                window.text_system().shape_line(
                                    (*line).to_owned().into(),
                                    theme::scaled(theme::CODE_TEXT),
                                    &runs,
                                    None,
                                )
                            }),
                            None,
                        ),
                    };
                    let rows = wrapped
                        .as_ref()
                        .map_or(1, |wrapped| wrapped.wrap_boundaries().len() + 1);
                    lines.push(EditorLine {
                        offset,
                        len: line.len(),
                        shaped,
                        wrapped,
                        row,
                        rows,
                    });
                    row += rows;
                    offset += raw.len() + 1;
                }
                view.bounds = Some(bounds);
                view.lines = lines;
                // The caret's place is known only now; when it moved out of view, scroll to it
                // and draw again.
                if view.reveal_caret() {
                    cx.notify();
                }
                // The box around a prompt is sized by its drawn rows, known only now: draw
                // again when wrapping changed how many there are.
                if view.visual_rows != row {
                    view.visual_rows = row;
                    cx.notify();
                }
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
                    let origin = point(
                        bounds.left() + px(geometry.left),
                        bounds.top() + px(geometry.top + entry.row as f32 * geometry.line),
                    );
                    if origin.y + line_height * (entry.rows as f32) < px(0.)
                        || origin.y > window.viewport_size().height
                    {
                        continue;
                    }
                    if let Some(wrapped) = &entry.wrapped {
                        let caret = (selection.is_empty() && view.focus.is_focused(window))
                            .then_some((selection.end, view.row_start));
                        paint_wrapped(entry, wrapped, origin, &selection, caret, window, cx);
                        continue;
                    }
                    let Some(line) = &entry.shaped else {
                        continue;
                    };
                    let offset = &entry.offset;
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

/// `line` wrapped to `width`. A line that can't be shaped is logged and left undrawn rather
/// than taking the editor down.
fn wrap_line(
    line: &str,
    runs: &[TextRun],
    width: Pixels,
    window: &mut gpui::Window,
) -> Option<gpui::WrappedLine> {
    match window.text_system().shape_text(
        line.to_owned().into(),
        theme::scaled(theme::CODE_TEXT),
        runs,
        Some(width),
        None,
    ) {
        Ok(mut lines) if !lines.is_empty() => Some(lines.swap_remove(0)),
        Ok(_) => None,
        Err(error) => {
            tracing::warn!(%error, "Prompt line could not be wrapped");
            None
        }
    }
}

/// One wrapped prompt line: its selection row by row, its text, and the caret when it is here.
fn paint_wrapped(
    entry: &EditorLine,
    wrapped: &gpui::WrappedLine,
    origin: gpui::Point<Pixels>,
    selection: &std::ops::Range<usize>,
    // The caret's offset and whether it sits at the start of a row (see `caret_place`).
    caret: Option<(usize, bool)>,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    let line_height = px(Geometry::current(false).line);
    let layout = &wrapped.unwrapped_layout;
    for (row, range) in entry.row_ranges().into_iter().enumerate() {
        let start = selection
            .start
            .saturating_sub(entry.offset)
            .max(range.start);
        let end = selection.end.saturating_sub(entry.offset).min(range.end);
        if selection.start >= entry.offset + range.end || start >= end {
            continue;
        }
        let left = layout.x_for_index(range.start);
        let top = origin.y + line_height * row as f32;
        window.paint_quad(fill(
            Bounds::from_corners(
                point(origin.x + layout.x_for_index(start) - left, top),
                point(origin.x + layout.x_for_index(end) - left, top + line_height),
            ),
            rgba(theme::colors().selection),
        ));
    }
    if let Err(error) = wrapped.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
    {
        tracing::warn!(%error, "Prompt line could not be painted");
    }
    let place = caret
        .filter(|(caret, _)| (entry.offset..=entry.offset + entry.len).contains(caret))
        .and_then(|(caret, row_start)| entry.caret_place(caret - entry.offset, row_start));
    if let Some((row, x)) = place {
        window.paint_quad(fill(
            Bounds::new(
                point(origin.x + x, origin.y + line_height * row as f32),
                size(px(1.5), theme::scaled(theme::CARET_HEIGHT)),
            ),
            rgb(theme::colors().accent),
        ));
    }
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
    paint_label(
        number.to_string(),
        theme::LINE_NUMBER_TEXT,
        origin,
        line_height,
        window,
        cx,
    );
}

fn paint_placeholder(
    origin: gpui::Point<Pixels>,
    line_height: Pixels,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    paint_label(
        "Give this agent an intention…".into(),
        theme::CODE_TEXT,
        origin,
        line_height,
        window,
        cx,
    );
}

fn paint_label(
    text: String,
    size: f32,
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
        .shape_line(text.into(), theme::scaled(size), &[run], None)
        .paint(origin, line_height, window, cx)
    {
        tracing::warn!(%error,"Editor label could not be painted");
    }
}
