// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 13:59:38.171437
//+ Revised: 	2026/11/12 13:59:38.171437
// -----------------------------------------------------------------------------

//! The activity bar, like VS Code's: one icon per column down the left edge, with the accounts
//! and Settings buttons at the bottom.
//!
//! A column's icon is marked with an accent bar while the column is shown; clicking it hides or
//! shows the column (and leaves a full-window page for the columns).

use super::{
    assets::Icon,
    columns::column_icon,
    controls::{hint, measured},
    menus::Menu,
    theme,
    workspace::Workspace,
};
use crate::settings::{Column, Page, SettingsSection};
use gpui::{AnyElement, Context, MouseButton, SharedString, div, prelude::*, px, rgb, svg};

/// VS Code's activity bar is 48 px wide with 24 px icons.
pub const ACTIVITY_BAR_WIDTH: f32 = 48.0;
const ITEM_HEIGHT: f32 = 48.0;
const GLYPH: f32 = 22.0;

/// What one activity-bar button shows.
struct Item {
    id: String,
    glyph: Icon,
    tooltip: SharedString,
    active: bool,
    enabled: bool,
}

impl Workspace {
    pub fn activity_bar(&self, cx: &Context<Self>) -> AnyElement {
        let columns_shown = self.settings.page.is_none();
        let order = self.settings.columns.order;
        let column_items = order.into_iter().map(|column| {
            let shown = !self.settings.columns.get(column).collapsed;
            self.item(
                Item {
                    id: format!("activity-{}", column.key()),
                    glyph: column_icon(column),
                    tooltip: if shown && columns_shown {
                        format!("Hide {}", column.title()).into()
                    } else {
                        format!("Show {}", column.title()).into()
                    },
                    active: shown && columns_shown,
                    enabled: !self.closing,
                },
                cx,
                move |view, window, cx| view.activate_column(column, window, cx),
            )
        });
        let account_open = self
            .menu
            .as_ref()
            .is_some_and(|open| open.menu == Menu::Account);
        let settings_open = self.settings.settings_section().is_some();
        theme::column()
            .id("activity-bar")
            .w(theme::scaled(ACTIVITY_BAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .bg(rgb(theme::colors().panel))
            .border_r_1()
            .border_color(rgb(theme::colors().border))
            .children(column_items)
            .child(div().flex_1())
            .child(self.item(
                Item {
                    id: "open-accounts".into(),
                    glyph: Icon::Account,
                    tooltip: "Accounts".into(),
                    active: account_open,
                    enabled: !self.closing,
                },
                cx,
                |view, window, cx| view.toggle_menu(Menu::Account, "open-accounts", window, cx),
            ))
            .child(self.item(
                Item {
                    id: "open-settings".into(),
                    glyph: Icon::Settings,
                    tooltip: "Settings (Ctrl+,)".into(),
                    active: settings_open,
                    enabled: !self.closing,
                },
                cx,
                |view, window, cx| view.open_settings(SettingsSection::Appearance, window, cx),
            ))
            .into_any_element()
    }

    /// Show a column, or hide it when it's already shown; from a page, return to the columns.
    pub fn activate_column(
        &mut self,
        column: Column,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.close_menu(window, cx);
        match self.settings.page {
            Some(Page::Welcome) if self.project.is_none() => {
                // Without a project the columns have nothing to show; Welcome stays.
                tracing::debug!(target: "orin::desktop", "columns need an open project");
            }
            Some(_) => {
                self.settings.page = None;
                self.expand_column(column);
                self.changed(cx);
            }
            None => self.toggle_column(column, cx),
        }
    }

    fn item(
        &self,
        spec: Item,
        cx: &Context<Self>,
        action: impl Fn(&mut Self, &mut gpui::Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let Item {
            id,
            glyph,
            tooltip,
            active,
            enabled,
        } = spec;
        let colors = theme::colors();
        let color = if !enabled {
            colors.disabled_text
        } else if active {
            colors.text
        } else {
            colors.faint
        };
        div()
            .id(SharedString::from(id.clone()))
            .relative()
            .w_full()
            .h(theme::scaled(ITEM_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .tooltip(hint(tooltip))
            // The accent bar on the left edge marks what is shown, as in VS Code.
            .when(active, |item| {
                item.child(
                    // Inset so marks on neighbouring icons stay apart.
                    div()
                        .absolute()
                        .left_0()
                        .top(theme::scaled(10.))
                        .bottom(theme::scaled(10.))
                        .w(px(2.))
                        .bg(rgb(colors.accent)),
                )
            })
            .child(
                svg()
                    .path(glyph.path())
                    .size(theme::scaled(GLYPH))
                    .text_color(rgb(color)),
            )
            .child(measured(id, enabled, &self.controls))
            .when(enabled, |item| {
                item.cursor_pointer()
                    .hover(|style| style.bg(rgb(theme::colors().hover)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, window, cx| {
                            cx.stop_propagation();
                            action(view, window, cx)
                        }),
                    )
            })
            .into_any_element()
    }
}
