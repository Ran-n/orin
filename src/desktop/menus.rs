// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 10:31:30.392689
//+ Revised: 	2026/11/13 17:43:24.012921
// -----------------------------------------------------------------------------

//! Drop-down menus anchored under their button: the account menu and agent session options.
//!
//! A menu closes when a choice is made, when the pointer goes down anywhere outside it, or on
//! Escape.

use super::{
    CloseMenu,
    agent_actions::provider_id,
    assets::Icon,
    controls::{ControlBounds, hint, icon, measured},
    theme,
    workspace::Workspace,
};
use crate::{
    agents::{Provider, accounts::SignIn},
    settings::SettingsSection,
};
use gpui::{
    AnyElement, Context, Corner, FontWeight, MouseButton, MouseDownEvent, SharedString, anchored,
    deferred, div, point, prelude::*, px, rgb,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Menu {
    Account,
    Editor,
    /// Choices for one option (such as the model) of the agent session at `session`.
    AgentOption {
        session: usize,
        option: String,
    },
}

/// An open menu and the button it hangs from.
#[derive(Clone, Debug)]
pub struct OpenMenu {
    pub menu: Menu,
    pub anchor: ControlBounds,
}

/// What choosing a menu entry does.
type MenuAction = Box<dyn Fn(&mut Workspace, &mut gpui::Window, &mut Context<Workspace>)>;

enum Entry {
    Item {
        id: String,
        label: SharedString,
        detail: Option<SharedString>,
        checked: Option<bool>,
        dot: Option<u32>,
        enabled: bool,
        tooltip: Option<SharedString>,
        action: MenuAction,
    },
    Label(SharedString),
    Separator,
    Custom(AnyElement),
}

impl Workspace {
    /// Open `menu` under the control `anchor_id`, or close it if it is already open.
    pub fn toggle_menu(
        &mut self,
        menu: Menu,
        anchor_id: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.menu.as_ref().is_some_and(|open| open.menu == menu) {
            self.close_menu(window, cx);
            return;
        }
        let Some(anchor) = self.controls.borrow().get(anchor_id).cloned() else {
            tracing::warn!(target: "orin::desktop", anchor_id, "menu button was not on screen");
            return;
        };
        if menu == Menu::Account && self.accounts.is_none() {
            self.refresh_accounts(cx);
        }
        self.menu = Some(OpenMenu { menu, anchor });
        window.focus(&self.menu_focus);
        cx.notify();
    }

    pub fn close_menu(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            window.focus(&self.focus);
            cx.notify();
        }
    }

    /// The open menu, drawn above everything else.
    pub fn menu_overlay(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let open = self.menu.as_ref()?;
        let (entries, corner, position, id) = match &open.menu {
            Menu::Editor => (
                self.editor_entries(cx),
                Corner::TopRight,
                point(
                    px(open.anchor.x + open.anchor.width),
                    px(open.anchor.y + open.anchor.height + 4.),
                ),
                "editor-menu".to_owned(),
            ),
            Menu::Account => (
                self.account_entries(),
                Corner::BottomLeft,
                point(
                    px(open.anchor.x + open.anchor.width + 4.),
                    px(open.anchor.y + open.anchor.height),
                ),
                "account-menu".to_owned(),
            ),
            Menu::AgentOption { session, option } => (
                self.option_entries(*session, option, cx)?,
                Corner::TopLeft,
                point(
                    px(open.anchor.x),
                    px(open.anchor.y + open.anchor.height + 4.),
                ),
                format!("agent-option-menu-{option}"),
            ),
        };
        let anchor = open.anchor.clone();
        let colors = theme::colors();
        let panel = theme::column()
            .id(SharedString::from(id.clone()))
            .relative()
            .min_w(theme::scaled(240.))
            .max_w(theme::scaled(380.))
            .max_h(theme::scaled(460.))
            .overflow_y_scroll()
            .p_1()
            .rounded(theme::scaled(6.))
            .border_1()
            .border_color(rgb(colors.border))
            .bg(rgb(colors.raised))
            .shadow_lg()
            .text_size(theme::scaled(theme::SMALL_TEXT))
            .key_context("Menu")
            .track_focus(&self.menu_focus)
            .on_action(cx.listener(|view, _: &CloseMenu, window, cx| view.close_menu(window, cx)))
            .on_mouse_down_out(
                cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                    // The menu's own button toggles it; anything else outside closes it.
                    let (x, y) = (f32::from(event.position.x), f32::from(event.position.y));
                    let on_anchor = x >= anchor.x
                        && x <= anchor.x + anchor.width
                        && y >= anchor.y
                        && y <= anchor.y + anchor.height;
                    if !on_anchor {
                        view.close_menu(window, cx);
                    }
                }),
            )
            .children(entries.into_iter().map(|entry| self.menu_entry(entry, cx)))
            .child(measured(id, true, &self.controls));
        Some(
            deferred(
                anchored()
                    .position(position)
                    .anchor(corner)
                    .snap_to_window_with_margin(px(8.))
                    .child(panel),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    fn menu_entry(&self, entry: Entry, cx: &Context<Self>) -> AnyElement {
        let colors = theme::colors();
        match entry {
            Entry::Separator => div()
                .my_1()
                .h(px(1.))
                .bg(rgb(colors.border))
                .into_any_element(),
            Entry::Label(text) => div()
                .px_2()
                .pt_1()
                .pb(px(2.))
                .text_size(theme::scaled(theme::TINY_TEXT))
                .text_color(rgb(colors.muted))
                .child(text.to_uppercase())
                .into_any_element(),
            Entry::Custom(element) => element,
            Entry::Item {
                id,
                label,
                detail,
                checked,
                dot,
                enabled,
                tooltip,
                action,
            } => {
                let foreground = if enabled {
                    colors.text
                } else {
                    colors.disabled_text
                };
                theme::row()
                    .id(SharedString::from(id.clone()))
                    .relative()
                    .min_h(theme::scaled(26.))
                    .px_2()
                    .gap_2()
                    .rounded(theme::scaled(theme::RADIUS))
                    .text_color(rgb(foreground))
                    .when_some(checked, |row, checked| {
                        row.child(
                            div()
                                .w(theme::scaled(theme::ICON_SIZE))
                                .flex_shrink_0()
                                .children(checked.then(|| icon(Icon::Check, colors.accent))),
                        )
                    })
                    .when_some(dot, |row, color| {
                        row.child(
                            div()
                                .size(theme::scaled(7.))
                                .rounded_full()
                                .flex_shrink_0()
                                .bg(rgb(color)),
                        )
                    })
                    .child(theme::clipped(label))
                    .children(detail.map(|detail| {
                        div()
                            .flex_shrink_0()
                            .max_w(theme::scaled(180.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(rgb(colors.muted))
                            .child(detail)
                    }))
                    .when_some(tooltip, |row, text| row.tooltip(hint(text)))
                    .child(measured(id, enabled, &self.controls))
                    .when(enabled, |row| {
                        row.cursor_pointer()
                            .hover(|style| style.bg(rgb(theme::colors().hover)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, window, cx| {
                                    cx.stop_propagation();
                                    view.close_menu(window, cx);
                                    action(view, window, cx);
                                }),
                            )
                    })
                    .into_any_element()
            }
        }
    }

    /// Like VS Code's accounts menu. Orin has no account service, so signing in is shown but
    /// unavailable; the agents' own sign-ins are listed below it.
    fn editor_entries(&self, cx: &Context<Self>) -> Vec<Entry> {
        let editor = self.editor.as_ref().map(|editor| editor.read(cx));
        let editing = !self.previewing_markdown()
            && self.settings.viewer_mode == crate::settings::ViewerMode::File;
        [
            (
                "editor-save",
                "Save",
                "Ctrl+S",
                editing && editor.is_some_and(|editor| editor.dirty() && !editor.saving),
            ),
            (
                "editor-undo",
                "Undo",
                "Ctrl+Z",
                editing && editor.is_some_and(|editor| editor.buffer().can_undo()),
            ),
            (
                "editor-redo",
                "Redo",
                "Ctrl+Y",
                editing && editor.is_some_and(|editor| editor.buffer().can_redo()),
            ),
            (
                "editor-reload",
                "Reload from disk",
                "",
                self.settings.selected_file.is_some()
                    && !editor.is_some_and(|editor| editor.saving),
            ),
        ]
        .into_iter()
        .map(|(id, label, shortcut, enabled)| Entry::Item {
            id: id.into(),
            label: label.into(),
            detail: (!shortcut.is_empty()).then(|| shortcut.into()),
            checked: None,
            dot: None,
            enabled: enabled && !self.closing,
            tooltip: None,
            action: Box::new(move |view, window, cx| {
                match id {
                    "editor-save" => view.save_editor(cx),
                    "editor-reload" => view.reload_editor(cx),
                    _ => {
                        if let Some(editor) = &view.editor {
                            editor.update(cx, |editor, cx| {
                                if id == "editor-undo" {
                                    editor.undo(cx)
                                } else {
                                    editor.redo(cx)
                                }
                            });
                        }
                    }
                }
                if let Some(editor) = &view.editor {
                    window.focus(&editor.read(cx).focus);
                }
            }),
        })
        .collect()
    }

    fn account_entries(&self) -> Vec<Entry> {
        let colors = theme::colors();
        let profile = theme::row()
            .gap_3()
            .px_2()
            .py_2()
            .child(
                div()
                    .size(theme::scaled(32.))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(rgb(colors.active))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(Icon::Account, colors.soft_text)),
            )
            .child(
                theme::column()
                    .gap(px(2.))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(colors.text))
                            .child("Not signed in"),
                    )
                    .child(
                        div()
                            .text_color(rgb(colors.muted))
                            .child("Orin accounts are coming later"),
                    ),
            )
            .into_any_element();
        let mut entries = vec![
            Entry::Custom(profile),
            Entry::Separator,
            Entry::Item {
                id: "account-menu-sync".into(),
                label: "Sign in to sync settings…".into(),
                detail: Some("Coming later".into()),
                checked: None,
                dot: None,
                enabled: false,
                tooltip: Some(
                    "Settings Sync needs an Orin account service, which doesn't exist yet".into(),
                ),
                action: Box::new(|_, _, _| {}),
            },
            Entry::Separator,
            Entry::Label("Agents".into()),
        ];
        for provider in Provider::ALL {
            let (dot, detail) = self.account_summary(provider);
            entries.push(Entry::Item {
                id: format!("account-menu-agent-{}", provider_id(provider)),
                label: provider.label().into(),
                detail: Some(detail.into()),
                checked: None,
                dot: Some(dot),
                enabled: !self.closing,
                tooltip: None,
                action: Box::new(|view, window, cx| {
                    view.open_settings(SettingsSection::Accounts, window, cx)
                }),
            });
        }
        entries.extend([
            Entry::Separator,
            Entry::Item {
                id: "account-menu-manage".into(),
                label: "Manage agent accounts…".into(),
                detail: None,
                checked: None,
                dot: None,
                enabled: !self.closing,
                tooltip: None,
                action: Box::new(|view, window, cx| {
                    view.open_settings(SettingsSection::Accounts, window, cx)
                }),
            },
        ]);
        entries
    }

    /// A status colour and a few words for `provider`'s sign-in.
    pub fn account_summary(&self, provider: Provider) -> (u32, String) {
        let colors = theme::colors();
        let account = self
            .accounts
            .as_ref()
            .and_then(|accounts| accounts.iter().find(|account| account.provider == provider));
        match account.map(|account| &account.sign_in) {
            None if self.accounts_checking => (colors.faint, "Checking…".into()),
            None => (colors.faint, "Not checked".into()),
            Some(SignIn::SignedIn(detail)) => (
                colors.accent,
                detail.clone().unwrap_or_else(|| "Signed in".into()),
            ),
            Some(SignIn::SignedOut) => (colors.warning, "Not signed in".into()),
            Some(SignIn::NoCli) => (colors.faint, "Not installed".into()),
            Some(SignIn::Unknown(_)) => (colors.error, "Status unknown".into()),
        }
    }

    fn option_entries(
        &self,
        session: usize,
        option_id: &str,
        cx: &Context<Self>,
    ) -> Option<Vec<Entry>> {
        let tab = self.agents.get(session)?;
        let snapshot = &tab.view.read(cx).snapshot;
        let option = snapshot
            .config_options
            .iter()
            .find(|option| option.id == option_id)?;
        let busy = snapshot.is_busy();
        let mut entries = vec![Entry::Label(option.name.clone().into())];
        entries.extend(option.choices.iter().map(|choice| {
            let value = choice.value.clone();
            let option = option.id.clone();
            Entry::Item {
                id: format!("agent-option-{}-{}", option, choice.value),
                label: choice.name.clone().into(),
                detail: None,
                checked: Some(choice.value == snapshot_current(snapshot, &option)),
                dot: None,
                enabled: !busy && !self.closing,
                tooltip: choice.description.clone().map(Into::into),
                action: Box::new(move |view, _, cx| {
                    view.set_agent_option(session, &option, &value, cx)
                }),
            }
        }));
        Some(entries)
    }
}

fn snapshot_current(snapshot: &crate::agents::AgentSnapshot, option: &str) -> String {
    snapshot
        .config_options
        .iter()
        .find(|candidate| candidate.id == option)
        .map(|candidate| candidate.current.clone())
        .unwrap_or_default()
}
