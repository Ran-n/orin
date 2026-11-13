// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:18:28.790673
//+ Revised: 	2026/11/12 09:16:22.818514
// -----------------------------------------------------------------------------

//! Shared compact controls, tooltips, and measured hit rectangles for native UI verification.

use super::{assets::Icon, theme};
use gpui::{
    AnyElement, AnyView, App, Context, IntoElement, MouseButton, Render, SharedString, Svg, Window,
    canvas, div, prelude::*, px, rgb, svg,
};
use serde::Serialize;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Clone, Debug, Serialize)]
pub struct ControlBounds {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub enabled: bool,
}

pub type Controls = Rc<RefCell<BTreeMap<String, ControlBounds>>>;

/// Records the visible rectangle of the enclosing element under `id`, for the UI test driver.
pub fn measured(id: impl Into<String>, enabled: bool, controls: &Controls) -> impl IntoElement {
    let id = id.into();
    let controls = controls.clone();
    canvas(
        move |bounds, window, _| {
            let visible = bounds.intersect(&window.content_mask().bounds);
            if visible.size.width <= px(1.) || visible.size.height <= px(1.) {
                return;
            }
            controls.borrow_mut().insert(
                id.clone(),
                ControlBounds {
                    id,
                    x: visible.origin.x.into(),
                    y: visible.origin.y.into(),
                    width: visible.size.width.into(),
                    height: visible.size.height.into(),
                    enabled,
                },
            );
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

pub fn icon(icon: Icon, color: u32) -> Svg {
    svg()
        .path(icon.path())
        .size(theme::scaled(theme::ICON_SIZE))
        .flex_shrink_0()
        .text_color(rgb(color))
}

/// A hover hint naming an icon-only control and its shortcut.
struct Tooltip(SharedString);

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded(theme::scaled(theme::RADIUS))
            .bg(rgb(theme::colors().active))
            .border_1()
            .border_color(rgb(theme::colors().border))
            .text_size(theme::scaled(theme::SMALL_TEXT))
            .text_color(rgb(theme::colors().text))
            .font_family(theme::interface_font())
            .child(self.0.clone())
    }
}

/// A tooltip builder for any stateful element.
pub fn hint(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tooltip(text.clone())).into()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Emphasis {
    /// Neutral control on a panel.
    Normal,
    /// The one main action in its area.
    Primary,
    /// The current choice in a group (a selected tab or mode).
    Selected,
    /// An icon-only toolbar control: no fill until hovered.
    Ghost,
}

impl Emphasis {
    /// Background (none for a ghost), foreground, and hover background.
    fn colors(self, enabled: bool) -> (Option<u32>, u32, u32) {
        match (enabled, self) {
            // A disabled ghost stays unfilled; its dimmed glyph marks it unavailable.
            (false, Self::Ghost) => (
                None,
                theme::colors().disabled_text,
                theme::colors().disabled_bg,
            ),
            (false, _) => (
                Some(theme::colors().disabled_bg),
                theme::colors().disabled_text,
                theme::colors().disabled_bg,
            ),
            (true, Self::Primary) => (
                Some(theme::colors().accent),
                theme::colors().on_accent,
                theme::colors().accent_hover,
            ),
            (true, Self::Selected) => (
                Some(theme::colors().active),
                theme::colors().text,
                theme::colors().active,
            ),
            (true, Self::Normal) => (
                Some(theme::colors().raised),
                theme::colors().text,
                theme::colors().hover,
            ),
            (true, Self::Ghost) => (None, theme::colors().soft_text, theme::colors().hover),
        }
    }
}

/// Everything a control needs besides its content.
pub struct ControlSpec<'a, 'b, T: 'static> {
    pub id: String,
    pub enabled: bool,
    pub emphasis: Emphasis,
    pub tooltip: Option<SharedString>,
    /// Fill the available width with content at the start, as in a list of sections.
    pub full_width: bool,
    pub controls: &'a Controls,
    pub cx: &'a Context<'b, T>,
}

impl<'a, 'b, T: 'static> ControlSpec<'a, 'b, T> {
    pub fn new(id: impl Into<String>, controls: &'a Controls, cx: &'a Context<'b, T>) -> Self {
        Self {
            id: id.into(),
            enabled: true,
            emphasis: Emphasis::Normal,
            tooltip: None,
            full_width: false,
            controls,
            cx,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn emphasis(mut self, emphasis: Emphasis) -> Self {
        self.emphasis = emphasis;
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Render `content` as a clickable control that runs `action` when enabled.
    fn build(
        self,
        content: AnyElement,
        horizontal_padding: f32,
        action: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> AnyElement {
        let (background, foreground, hover) = self.emphasis.colors(self.enabled);
        let id = self.id;
        div()
            .id(SharedString::from(id.clone()))
            .relative()
            .flex()
            .items_center()
            .when(self.full_width, |control| control.w_full().justify_start())
            .when(!self.full_width, |control| control.justify_center())
            .gap_1()
            .h(theme::scaled(theme::CONTROL_HEIGHT))
            .min_w(theme::scaled(theme::CONTROL_HEIGHT))
            .px(px(horizontal_padding))
            .flex_shrink_0()
            .rounded(theme::scaled(theme::RADIUS))
            .when_some(background, |element, background| {
                element.bg(rgb(background))
            })
            .text_color(rgb(foreground))
            .text_size(theme::scaled(theme::SMALL_TEXT))
            .whitespace_nowrap()
            .child(content)
            .child(measured(id, self.enabled, self.controls))
            .when_some(self.tooltip, |element, text| element.tooltip(hint(text)))
            .when(self.enabled, |element| {
                element
                    .cursor_pointer()
                    .hover(move |style| style.bg(rgb(hover)))
                    .on_mouse_down(
                        MouseButton::Left,
                        self.cx.listener(move |view, _, window, cx| {
                            cx.stop_propagation();
                            action(view, window, cx)
                        }),
                    )
            })
            .into_any_element()
    }

    pub fn text(
        self,
        label: impl Into<SharedString>,
        action: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> AnyElement {
        self.build(div().child(label.into()).into_any_element(), 8., action)
    }

    /// An icon-only control; a normal one is drawn as a ghost so toolbars stay quiet.
    pub fn icon(
        mut self,
        glyph: Icon,
        action: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> AnyElement {
        if self.emphasis == Emphasis::Normal {
            self.emphasis = Emphasis::Ghost;
        }
        let color = self.emphasis.colors(self.enabled).1;
        self.build(icon(glyph, color).into_any_element(), 0., action)
    }

    pub fn icon_text(
        self,
        glyph: Icon,
        label: impl Into<SharedString>,
        action: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> AnyElement {
        let color = self.emphasis.colors(self.enabled).1;
        let content = div()
            .flex()
            .items_center()
            .gap_1()
            .child(icon(glyph, color))
            .child(label.into())
            .into_any_element();
        self.build(content, 7., action)
    }
}

/// A labelled button; kept as a short form for the many plain text actions.
pub fn button<T: 'static>(
    id: impl Into<String>,
    label: impl Into<String>,
    enabled: bool,
    primary: bool,
    controls: &Controls,
    cx: &Context<T>,
    action: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
) -> AnyElement {
    let emphasis = if primary {
        Emphasis::Primary
    } else {
        Emphasis::Normal
    };
    ControlSpec::new(id, controls, cx)
        .enabled(enabled)
        .emphasis(emphasis)
        .text(label.into(), action)
}
