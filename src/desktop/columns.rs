// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 23:45:33.117347
//+ Revised: 	2026/11/12 13:59:43.052881
// -----------------------------------------------------------------------------

//! The four side-by-side workspace columns: width resolution, dividers, moving, and collapsing.
//!
//! One expanded column absorbs the remaining width (Agents when it is open, as the centre of the
//! product); the others keep their remembered widths. Dragging a divider resizes the neighbouring
//! fixed column, dragging a column's title moves it to another place in the row, and a collapsed
//! column shrinks to a narrow rail that expands again on click.

use super::{
    assets::Icon,
    controls::{ControlSpec, hint, icon, measured},
    theme,
    workspace::Workspace,
};
use crate::settings::{Column, Columns, MAX_COLUMN_WIDTH};
use gpui::{
    AnyElement, Context, CursorStyle, MouseButton, MouseDownEvent, MouseMoveEvent, div, prelude::*,
    px, rgb,
};

/// The resolved layout for one frame: a width per expanded column, `None` for a collapsed rail.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedColumns {
    pub widths: [Option<f32>; 4],
    /// The expanded column that absorbs leftover space, if any column is expanded.
    pub flex: Option<Column>,
}

impl ResolvedColumns {
    pub fn width(&self, column: Column) -> Option<f32> {
        self.widths[index(column)]
    }
}

fn index(column: Column) -> usize {
    Column::ALL
        .iter()
        .position(|candidate| *candidate == column)
        .expect("every column is listed in Column::ALL")
}

/// The column that fills the remaining width: Agents first, then the nearest open neighbour.
pub fn flex_column(columns: &Columns) -> Option<Column> {
    [
        Column::Agents,
        Column::Code,
        Column::Terminal,
        Column::Files,
    ]
    .into_iter()
    .find(|column| !columns.get(*column).collapsed)
}

/// Fit the remembered widths into `total` logical pixels of window width, with collapsed
/// columns drawn as rails `rail` pixels wide.
pub fn resolve(columns: &Columns, total: f32, rail: f32) -> ResolvedColumns {
    let flex = flex_column(columns);
    let rails = Column::ALL
        .iter()
        .filter(|column| columns.get(**column).collapsed)
        .count() as f32;
    let dividers = (Column::ALL.len() - 1) as f32;
    let space = (total - rails * rail - dividers * theme::DIVIDER_WIDTH).max(0.);
    let fixed: Vec<Column> = Column::ALL
        .into_iter()
        .filter(|column| !columns.get(*column).collapsed && Some(*column) != flex)
        .collect();
    let mut widths = [None; 4];
    for column in &fixed {
        widths[index(*column)] = Some(columns.get(*column).width);
    }
    let flex_min = flex.map_or(0., Column::min_width);
    let used: f32 = fixed.iter().map(|column| columns.get(*column).width).sum();
    let overflow = used + flex_min - space;
    if overflow > 0. {
        // Shrink fixed columns in proportion to their slack above the minimum.
        let slack: f32 = fixed
            .iter()
            .map(|column| columns.get(*column).width - column.min_width())
            .sum();
        if slack > 0. {
            let ratio = (overflow / slack).min(1.);
            for column in &fixed {
                let state = columns.get(*column);
                let shrunk = state.width - (state.width - column.min_width()) * ratio;
                widths[index(*column)] = Some(shrunk);
            }
        }
    }
    if let Some(flex) = flex {
        let fixed_total: f32 = widths.iter().flatten().sum();
        widths[index(flex)] = Some((space - fixed_total).max(0.));
    }
    ResolvedColumns { widths, flex }
}

/// Left and right edges of every column in display order, rails included.
pub fn spans(columns: &Columns, resolved: &ResolvedColumns, rail: f32) -> Vec<(Column, f32, f32)> {
    let mut left = 0.;
    columns
        .order
        .into_iter()
        .map(|column| {
            let right = left + resolved.width(column).unwrap_or(rail);
            let span = (column, left, right);
            left = right + theme::DIVIDER_WIDTH;
            span
        })
        .collect()
}

/// Pointer travel before pressing a column title turns into moving the column.
pub const MOVE_THRESHOLD: f32 = 4.0;

/// A column being moved by its title: where it started and where the pointer is now.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnMove {
    pub column: Column,
    start_x: f32,
    x: f32,
    /// Centres of the other columns when the move began, left to right.
    centres: Vec<f32>,
}

impl ColumnMove {
    pub fn begin(column: Column, spans: &[(Column, f32, f32)], x: f32) -> Self {
        Self {
            column,
            start_x: x,
            x,
            centres: spans
                .iter()
                .filter(|(other, _, _)| *other != column)
                .map(|(_, left, right)| (left + right) / 2.)
                .collect(),
        }
    }

    pub fn track(&mut self, x: f32) {
        self.x = x;
    }

    /// True once the pointer has travelled far enough to count as a move rather than a click.
    pub fn started(&self) -> bool {
        (self.x - self.start_x).abs() >= MOVE_THRESHOLD
    }

    /// Where the column lands: after every other column whose centre the pointer has passed.
    pub fn target(&self) -> usize {
        self.centres
            .iter()
            .filter(|centre| **centre < self.x)
            .count()
    }

    /// The columns with this move applied.
    pub fn apply(&self, columns: &Columns) -> Columns {
        let mut moved = *columns;
        if self.started() {
            moved.move_column(self.column, self.target());
        }
        moved
    }
}

/// A divider drag in progress: which column it resizes and how the pointer maps to its width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnDrag {
    pub column: Column,
    start_x: f32,
    start_width: f32,
    /// +1 when the resized column is left of the divider, -1 when it is right of it.
    direction: f32,
    max_width: f32,
}

impl ColumnDrag {
    /// The drag a divider between `left` and `right` starts, or `None` when neither side can resize.
    pub fn begin(
        left: Column,
        right: Column,
        resolved: &ResolvedColumns,
        start_x: f32,
    ) -> Option<Self> {
        let flex = resolved.flex?;
        let flex_slack = resolved.width(flex)? - flex.min_width();
        let (column, direction) = [(left, 1.), (right, -1.)]
            .into_iter()
            .find(|(column, _)| *column != flex && resolved.width(*column).is_some())?;
        let start_width = resolved.width(column)?;
        Some(Self {
            column,
            start_x,
            start_width,
            direction,
            max_width: (start_width + flex_slack.max(0.)).min(MAX_COLUMN_WIDTH),
        })
    }

    /// The resized column's width with the pointer at `x`, kept within its limits.
    pub fn width_at(&self, x: f32) -> f32 {
        let width = self.start_width + self.direction * (x - self.start_x);
        width.clamp(
            self.column.min_width(),
            self.max_width.max(self.column.min_width()),
        )
    }
}

/// Narrowest width at which a column header fits its name beside its actions.
/// Agents carries three launchers; the other headers always have room.
fn titled_width(column: Column) -> f32 {
    match column {
        // Room for the uppercase title beside the three agent launchers.
        Column::Agents => theme::metric(365.0),
        Column::Files | Column::Code | Column::Terminal => 0.0,
    }
}

pub fn column_icon(column: Column) -> Icon {
    match column {
        Column::Files => Icon::Files,
        Column::Agents => Icon::Agents,
        Column::Code => Icon::Code,
        Column::Terminal => Icon::Terminal,
    }
}

impl Workspace {
    /// The columns as drawn this frame: a column being moved is shown at its prospective place.
    pub fn shown_columns(&self) -> Columns {
        self.moving
            .as_ref()
            .map_or(self.settings.columns, |moving| {
                moving.apply(&self.settings.columns)
            })
    }

    /// Window width left for pages and columns beside the activity bar.
    pub fn page_width(window: &gpui::Window) -> f32 {
        (f32::from(window.viewport_size().width) - Self::page_left()).max(0.)
    }

    /// Where pages and columns start: just right of the activity bar.
    pub fn page_left() -> f32 {
        theme::metric(super::activity_bar::ACTIVITY_BAR_WIDTH)
    }

    pub fn resolved_columns(&self, total: f32) -> ResolvedColumns {
        resolve(
            &self.shown_columns(),
            total,
            theme::metric(theme::RAIL_WIDTH),
        )
    }

    pub fn reset_layout(&mut self, cx: &mut Context<Self>) {
        self.settings.columns = Columns::default();
        self.changed(cx);
    }

    fn begin_move(&mut self, column: Column, x: f32, total: f32) {
        let rail = theme::metric(theme::RAIL_WIDTH);
        let columns = self.settings.columns;
        let spans = spans(&columns, &resolve(&columns, total, rail), rail);
        self.moving = Some(ColumnMove::begin(column, &spans, x));
    }

    /// Finish a column move or divider drag when the button is released.
    pub fn end_pointer_drag(&mut self, cx: &mut Context<Self>) {
        if let Some(moving) = self.moving.take() {
            let moved = moving.apply(&self.settings.columns);
            if moved == self.settings.columns {
                cx.notify();
            } else {
                self.settings.columns = moved;
                self.changed(cx);
            }
        }
        if self.drag.take().is_some() {
            cx.notify();
        }
    }

    pub fn expand_column(&mut self, column: Column) {
        self.settings.columns.get_mut(column).collapsed = false;
    }

    pub fn toggle_column(&mut self, column: Column, cx: &mut Context<Self>) {
        let state = self.settings.columns.get_mut(column);
        state.collapsed = !state.collapsed;
        self.changed(cx);
    }

    pub fn drag_divider(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if let Some(moving) = &mut self.moving {
            if !event.dragging() {
                self.moving = None;
                cx.notify();
                return;
            }
            let before = (moving.started(), moving.target());
            moving.track(f32::from(event.position.x) - Self::page_left());
            if (moving.started(), moving.target()) != before {
                cx.notify();
            }
            return;
        }
        let Some(drag) = self.drag else { return };
        if !event.dragging() {
            self.drag = None;
            return;
        }
        let width = drag.width_at(event.position.x.into());
        let state = self.settings.columns.get_mut(drag.column);
        if (state.width - width).abs() >= 0.5 {
            state.width = width;
            self.changed(cx);
        }
    }

    /// Lay out the columns and their dividers for a window `total` logical pixels wide.
    pub fn columns_row(
        &self,
        total: f32,
        mut body: impl FnMut(&Self, Column, &Context<Self>) -> (Vec<AnyElement>, AnyElement),
        cx: &Context<Self>,
    ) -> AnyElement {
        let resolved = self.resolved_columns(total);
        let order = self.shown_columns().order;
        let mut row = div().flex().flex_1().min_h_0().w_full();
        for (position, column) in order.into_iter().enumerate() {
            if position > 0 {
                row = row.child(self.divider(order[position - 1], column, resolved, cx));
            }
            row = row.child(match resolved.width(column) {
                Some(width) => {
                    let (actions, content) = body(self, column, cx);
                    self.column_panel(column, width, actions, content, cx)
                }
                None => self.column_rail(column, cx),
            });
        }
        row.into_any_element()
    }

    fn column_panel(
        &self,
        column: Column,
        width: f32,
        actions: Vec<AnyElement>,
        content: AnyElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        let colors = theme::colors();
        let moving = self
            .moving
            .as_ref()
            .is_some_and(|moving| moving.column == column && moving.started());
        let handle_id = format!("move-{}", column.key());
        // The icon and title are the handle that moves the column; the actions stay clickable.
        let handle = theme::row()
            .id(gpui::SharedString::from(handle_id.clone()))
            .relative()
            .flex_1()
            .h_full()
            .pl_3()
            .gap_1()
            .cursor(if moving {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            })
            .tooltip(hint(format!("Drag to move {}", column.title())))
            .child(icon(
                column_icon(column),
                if moving { colors.accent } else { colors.muted },
            ))
            .child(
                // Below its titled width the header keeps only the icon rather than a cut-off word.
                theme::clipped(if width >= titled_width(column) {
                    column.title().to_uppercase()
                } else {
                    String::new()
                })
                .ml_1()
                .text_size(theme::scaled(theme::TINY_TEXT))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(if moving {
                    colors.text
                } else {
                    colors.soft_text
                })),
            )
            .child(measured(handle_id, !self.closing, &self.controls))
            .when(!self.closing, |handle| {
                handle.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                        let total = Self::page_width(window);
                        let x = f32::from(event.position.x) - Self::page_left();
                        view.begin_move(column, x, total);
                        cx.stop_propagation();
                    }),
                )
            });
        let header = theme::row()
            .h(theme::scaled(theme::HEADER_HEIGHT))
            .flex_shrink_0()
            .pr_1()
            .gap_1()
            .border_b_1()
            .border_color(rgb(if moving { colors.accent } else { colors.border }))
            .when(moving, |header| header.bg(rgb(colors.active)))
            .child(handle)
            .children(actions)
            .child(
                ControlSpec::new(format!("collapse-{}", column.key()), &self.controls, cx)
                    .enabled(!self.closing)
                    .tooltip(format!("Hide {}", column.title()))
                    .icon(Icon::Collapse, move |view, _, cx| {
                        view.toggle_column(column, cx)
                    }),
            );
        theme::column()
            .relative()
            .w(px(width))
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .bg(rgb(if column == Column::Files {
                theme::colors().panel
            } else {
                theme::colors().background
            }))
            .child(header)
            .child(div().flex().flex_col().flex_1().min_h_0().child(content))
            .child(measured(
                format!("column-{}", column.key()),
                true,
                &self.controls,
            ))
            .into_any_element()
    }

    fn column_rail(&self, column: Column, cx: &Context<Self>) -> AnyElement {
        theme::column()
            .relative()
            .w(theme::scaled(theme::RAIL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .items_center()
            .pt_1()
            .gap_2()
            .bg(rgb(theme::colors().panel))
            .child(
                ControlSpec::new(format!("expand-{}", column.key()), &self.controls, cx)
                    .enabled(!self.closing)
                    .tooltip(format!("Show {}", column.title()))
                    .icon(column_icon(column), move |view, _, cx| {
                        view.toggle_column(column, cx)
                    }),
            )
            .child(icon(Icon::Expand, theme::colors().faint))
            .child(measured(
                format!("rail-{}", column.key()),
                true,
                &self.controls,
            ))
            .into_any_element()
    }

    fn divider(
        &self,
        left: Column,
        right: Column,
        resolved: ResolvedColumns,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = format!("divider-{}-{}", left.key(), right.key());
        let resizable = ColumnDrag::begin(left, right, &resolved, 0.).is_some();
        let dragging = self
            .drag
            .is_some_and(|drag| drag.column == left || drag.column == right);
        div()
            .id(gpui::SharedString::from(id.clone()))
            .relative()
            .w(px(theme::DIVIDER_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .justify_center()
            .bg(rgb(theme::colors().background))
            .child(div().w(px(1.)).h_full().bg(rgb(if dragging {
                theme::colors().accent
            } else {
                theme::colors().border
            })))
            .child(measured(id, resizable, &self.controls))
            .when(resizable, |element| {
                element
                    .cursor(CursorStyle::ResizeLeftRight)
                    .hover(|style| style.bg(rgb(theme::colors().hover)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                            let total = Self::page_width(window);
                            view.drag = ColumnDrag::begin(
                                left,
                                right,
                                &view.resolved_columns(total),
                                event.position.x.into(),
                            );
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAIL: f32 = theme::RAIL_WIDTH;

    fn total(resolved: &ResolvedColumns, columns: &Columns) -> f32 {
        let rails = Column::ALL
            .iter()
            .filter(|column| columns.get(**column).collapsed)
            .count() as f32;
        resolved.widths.iter().flatten().sum::<f32>() + rails * RAIL + 3. * theme::DIVIDER_WIDTH
    }

    #[test]
    fn unit_default_layout_fills_the_window_and_agents_take_the_rest() {
        let columns = Columns::default();
        let resolved = resolve(&columns, 1440., RAIL);
        assert_eq!(resolved.flex, Some(Column::Agents));
        assert_eq!(resolved.width(Column::Files), Some(columns.files.width));
        assert_eq!(resolved.width(Column::Code), Some(columns.code.width));
        assert!((total(&resolved, &columns) - 1440.).abs() < 0.01);
    }

    #[test]
    fn unit_narrow_window_shrinks_fixed_columns_but_respects_minimums() {
        let columns = Columns::default();
        let resolved = resolve(&columns, 1000., RAIL);
        for column in Column::ALL {
            assert!(
                resolved.width(column).unwrap() >= column.min_width() - 0.01,
                "{column:?} {resolved:?}"
            );
        }
        assert!((total(&resolved, &columns) - 1000.).abs() < 0.01);
    }

    #[test]
    fn unit_collapsed_columns_become_rails_and_the_next_column_flexes() {
        let mut columns = Columns::default();
        columns.agents.collapsed = true;
        columns.terminal.collapsed = true;
        let resolved = resolve(&columns, 1200., RAIL);
        assert_eq!(resolved.width(Column::Agents), None);
        assert_eq!(resolved.width(Column::Terminal), None);
        assert_eq!(resolved.flex, Some(Column::Code));
        assert!((total(&resolved, &columns) - 1200.).abs() < 0.01);
    }

    #[test]
    fn unit_all_collapsed_leaves_only_rails() {
        let mut columns = Columns::default();
        for column in Column::ALL {
            columns.get_mut(column).collapsed = true;
        }
        let resolved = resolve(&columns, 1200., RAIL);
        assert_eq!(resolved.flex, None);
        assert!(resolved.widths.iter().all(Option::is_none));
    }

    #[test]
    fn unit_dragging_resizes_the_fixed_neighbour_within_limits() {
        let columns = Columns::default();
        let resolved = resolve(&columns, 1440., RAIL);
        // Agents flexes, so Files (left of the divider) is resized and grows to the right.
        let drag = ColumnDrag::begin(Column::Files, Column::Agents, &resolved, 220.).unwrap();
        assert_eq!(drag.column, Column::Files);
        assert_eq!(drag.width_at(260.), columns.files.width + 40.);
        assert_eq!(drag.width_at(-500.), Column::Files.min_width());
        let agents_slack = resolved.width(Column::Agents).unwrap() - Column::Agents.min_width();
        assert_eq!(drag.width_at(5_000.), columns.files.width + agents_slack);
        // Agents | Code: Code (right) is resized and grows when dragged left.
        let drag = ColumnDrag::begin(Column::Agents, Column::Code, &resolved, 700.).unwrap();
        assert_eq!(drag.column, Column::Code);
        assert_eq!(drag.width_at(650.), columns.code.width + 50.);
    }

    #[test]
    fn unit_divider_between_two_collapsed_columns_cannot_drag() {
        let mut columns = Columns::default();
        columns.code.collapsed = true;
        columns.terminal.collapsed = true;
        let resolved = resolve(&columns, 1200., RAIL);
        assert!(ColumnDrag::begin(Column::Code, Column::Terminal, &resolved, 0.).is_none());
    }

    fn default_spans() -> Vec<(Column, f32, f32)> {
        let columns = Columns::default();
        spans(&columns, &resolve(&columns, 1440., RAIL), RAIL)
    }

    fn centre(spans: &[(Column, f32, f32)], column: Column) -> f32 {
        let (_, left, right) = spans.iter().find(|span| span.0 == column).unwrap();
        (left + right) / 2.
    }

    #[test]
    fn unit_spans_follow_the_display_order_and_cover_the_window() {
        let mut columns = Columns::default();
        columns.move_column(Column::Terminal, 0);
        let resolved = resolve(&columns, 1440., RAIL);
        let spans = spans(&columns, &resolved, RAIL);
        let order: Vec<Column> = spans.iter().map(|span| span.0).collect();
        assert_eq!(order, columns.order.to_vec());
        assert_eq!(spans[0].1, 0.);
        assert!((spans[3].2 - 1440.).abs() < 0.01);
        for pair in spans.windows(2) {
            assert!((pair[1].1 - pair[0].2 - theme::DIVIDER_WIDTH).abs() < 0.01);
        }
    }

    #[test]
    fn unit_a_small_press_is_not_a_move() {
        let spans = default_spans();
        let mut moving = ColumnMove::begin(Column::Terminal, &spans, 1300.);
        moving.track(1300. - MOVE_THRESHOLD + 1.);
        assert!(!moving.started());
        assert_eq!(moving.apply(&Columns::default()), Columns::default());
    }

    #[test]
    fn unit_moving_a_column_lands_it_after_the_centres_it_passes() {
        let spans = default_spans();
        let mut moving = ColumnMove::begin(Column::Terminal, &spans, 1300.);
        moving.track(5.);
        assert_eq!(moving.target(), 0);
        assert_eq!(
            moving.apply(&Columns::default()).order,
            [
                Column::Terminal,
                Column::Files,
                Column::Code,
                Column::Agents
            ]
        );
        moving.track(centre(&spans, Column::Code) + 1.);
        assert_eq!(
            moving.apply(&Columns::default()).order,
            [
                Column::Files,
                Column::Code,
                Column::Terminal,
                Column::Agents
            ]
        );
        let mut moving = ColumnMove::begin(Column::Files, &spans, 20.);
        moving.track(5_000.);
        assert_eq!(
            moving.apply(&Columns::default()).order,
            [
                Column::Code,
                Column::Agents,
                Column::Terminal,
                Column::Files
            ]
        );
    }

    #[test]
    fn unit_moving_keeps_widths_and_collapsed_state() {
        let mut columns = Columns::default();
        columns.code.width = 333.;
        columns.files.collapsed = true;
        let spans = spans(&columns, &resolve(&columns, 1440., RAIL), RAIL);
        let mut moving = ColumnMove::begin(Column::Code, &spans, 900.);
        moving.track(0.);
        let moved = moving.apply(&columns);
        assert_eq!(moved.order[0], Column::Code);
        assert_eq!(moved.code.width, 333.);
        assert!(moved.files.collapsed);
    }

    #[test]
    fn unit_resolution_is_independent_of_order() {
        let mut columns = Columns::default();
        let before = resolve(&columns, 1280., RAIL);
        columns.move_column(Column::Agents, 3);
        assert_eq!(resolve(&columns, 1280., RAIL), before);
    }
}
