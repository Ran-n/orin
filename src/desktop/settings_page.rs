// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 09:16:12.029537
//+ Revised: 	2026/11/14 01:02:31.660382
// -----------------------------------------------------------------------------

//! The Settings page: appearance (theme, fonts, text size), agent accounts, layout, and updates.
//!
//! It takes the place of the columns while open, like a settings tab, and every choice applies
//! and is saved at once; there is nothing to confirm.

use super::{
    CloseSettings,
    agent_actions::provider_id,
    assets::Icon,
    controls::{ControlSpec, Emphasis, hint, icon, measured},
    events::Event,
    theme,
    workspace::Workspace,
};
use crate::{
    agents::{
        Provider,
        accounts::{self, Account, SignIn},
    },
    settings::{Appearance, Column, Columns, Page, SettingsSection, Theme},
};
use gpui::{AnyElement, Context, FontWeight, MouseButton, div, prelude::*, px, rgb};

/// Which font a picker chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontSlot {
    Interface,
    Code,
}

impl FontSlot {
    fn key(self) -> &'static str {
        match self {
            Self::Interface => "interface",
            Self::Code => "code",
        }
    }

    fn system_font(self) -> &'static str {
        match self {
            Self::Interface => theme::system_interface_font(),
            Self::Code => theme::system_code_font(),
        }
    }
}

const NAVIGATION_WIDTH: f32 = 190.0;
const MAX_CONTENT_WIDTH: f32 = 760.0;

/// The most tool folders that can be saved.
const MAX_TOOL_DIRECTORIES: usize = 32;
const FONT_SAMPLE: &str = "The quick brown fox jumps over the lazy dog · 0O 1lI {}[]";

impl Workspace {
    pub fn refresh_tools(&mut self, cx: &mut Context<Self>) {
        self.account_environment.path = crate::process::launch::tool_path(
            &self.settings.tool_directories,
            !self.update.test_driver,
        );
        let path = self.account_environment.path.clone();
        let sender = self.sender.clone();
        self.submit(move || {
            let providers = super::providers::probe_all_in(path.as_deref());
            if sender.send(Event::Providers { path, providers }).is_err() {
                tracing::debug!(target: "orin::accounts", "Workspace closed during tool discovery");
            }
        });
        self.accounts = None;
        self.refresh_accounts(cx);
    }

    pub fn choose_tool_directory(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose the folder containing your agent CLI or Node.js (npx)".into()),
        });
        cx.spawn(async move |view, cx| {
            let result = prompt.await;
            if let Err(error) = view.update(cx, |view, cx| match result {
                Ok(Ok(Some(paths))) => {
                    if let Some(path) = paths.into_iter().next() {
                        if !view.settings.tool_directories.contains(&path)
                            && view.settings.tool_directories.len() < MAX_TOOL_DIRECTORIES
                        {
                            view.settings.tool_directories.push(path);
                            view.changed(cx);
                        }
                        view.refresh_tools(cx);
                    }
                }
                Ok(Ok(None)) => {}
                result => {
                    tracing::warn!(?result, "Tool folder picker failed");
                    view.error = Some("The folder picker could not open. Try again.".into());
                    cx.notify();
                }
            }) {
                tracing::debug!(%error, "Workspace closed during tool selection");
            }
        })
        .detach();
    }

    pub fn open_settings(
        &mut self,
        section: SettingsSection,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.page = Some(Page::Settings(section));
        self.font_picker = None;
        window.focus(&self.settings_focus);
        if section == SettingsSection::Accounts && self.accounts.is_none() {
            self.refresh_accounts(cx);
        }
        self.changed(cx);
    }

    pub fn close_settings(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if self.settings.page.take().is_some() {
            self.font_picker = None;
            window.focus(&self.focus);
            self.changed(cx);
        }
    }

    pub fn choose_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if self.settings.appearance.theme != theme {
            self.settings.appearance.theme = theme;
            self.changed(cx);
        }
    }

    pub fn toggle_font_picker(&mut self, slot: FontSlot, cx: &mut Context<Self>) {
        self.font_picker = (self.font_picker != Some(slot)).then_some(slot);
        if self.font_picker.is_some() && self.font_names.is_none() {
            let mut names = cx.text_system().all_font_names();
            names.retain(|name| installed_font(name));
            names.sort_by_key(|name| name.to_lowercase());
            names.dedup();
            tracing::debug!(target: "orin::settings", count = names.len(), "system fonts listed");
            self.font_names = Some(names);
        }
        cx.notify();
    }

    /// Use `name` for `slot`, or the system font when `None`.
    pub fn choose_font(&mut self, slot: FontSlot, name: Option<String>, cx: &mut Context<Self>) {
        let appearance = &mut self.settings.appearance;
        let target = match slot {
            FontSlot::Interface => &mut appearance.interface_font,
            FontSlot::Code => &mut appearance.code_font,
        };
        self.font_picker = None;
        if *target != name {
            *target = name;
            self.changed(cx);
        } else {
            cx.notify();
        }
    }

    pub fn step_text_size(&mut self, steps: f32, cx: &mut Context<Self>) {
        let before = self.settings.appearance.text_size;
        self.settings.appearance.step_text_size(steps);
        if self.settings.appearance.text_size != before {
            self.changed(cx);
        }
    }

    pub fn default_text_size(&mut self, cx: &mut Context<Self>) {
        if self.settings.appearance.text_size != Appearance::DEFAULT_TEXT_SIZE {
            self.settings.appearance.text_size = Appearance::DEFAULT_TEXT_SIZE;
            self.changed(cx);
        }
    }

    /// Ask every agent's CLI for its sign-in status, off the UI thread.
    pub fn refresh_accounts(&mut self, cx: &mut Context<Self>) {
        if self.accounts_checking || self.closing {
            return;
        }
        let environment = self.account_environment.clone();
        let sender = self.sender.clone();
        if self.submit(move || {
            let accounts = accounts::check_all(&environment);
            if sender.send(Event::Accounts { path: environment.path, accounts }).is_err() {
                tracing::debug!(target: "orin::accounts", "workspace closed during the account check");
            }
        }) {
            self.accounts_checking = true;
        }
        cx.notify();
    }

    pub fn accounts_checked(&mut self, accounts: Vec<Account>, cx: &mut Context<Self>) {
        self.accounts_checking = false;
        self.accounts = Some(accounts);
        cx.notify();
    }

    fn account(&self, provider: Provider) -> Option<&Account> {
        self.accounts
            .as_ref()?
            .iter()
            .find(|account| account.provider == provider)
    }

    /// Start `provider`'s own sign-in in the Terminal column, where the user answers its prompts.
    pub fn sign_in(
        &mut self,
        provider: Provider,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let Some(cli) = self
            .account(provider)
            .and_then(|account| account.cli.clone())
        else {
            return;
        };
        if self.project.is_none() || self.closing {
            return;
        }
        let Some(command) = accounts::sign_in_command_line(provider, &cli) else {
            tracing::warn!(target: "orin::accounts", provider = provider.label(), "sign-in path can't be typed safely");
            self.error = Some(format!(
                "The {} command's path contains %, which the terminal would change. \
                 Run its sign-in yourself in the Terminal column.",
                provider.cli()
            ));
            cx.notify();
            return;
        };
        let line = format!("{command}\r");
        tracing::info!(target: "orin::accounts", provider = provider.label(), "sign-in started in the terminal");
        self.close_settings(window, cx);
        self.expand_column(Column::Terminal);
        // Once signed in, the account has to be checked again.
        self.accounts = None;
        match &self.shell {
            Some(tab) if tab.view.read(cx).running() => {
                tab.view
                    .update(cx, |view, cx| view.send(line.as_bytes(), cx));
                window.focus(&tab.view.read(cx).focus);
            }
            _ => {
                self.shell_input = Some(line);
                self.launch_shell(cx);
            }
        }
        self.changed(cx);
    }

    /// The page for a window `total` logical pixels wide.
    pub fn settings_page(
        &self,
        section: SettingsSection,
        total: f32,
        cx: &Context<Self>,
    ) -> AnyElement {
        let colors = theme::colors();
        // GPUI wraps text only inside a width known in pixels, so the content width is computed:
        // what the navigation leaves, up to a readable line length.
        let content_width = (total - theme::metric(NAVIGATION_WIDTH))
            .min(theme::metric(MAX_CONTENT_WIDTH))
            .max(0.);
        let navigation = theme::column()
            .w(theme::scaled(NAVIGATION_WIDTH))
            .flex_shrink_0()
            .h_full()
            .p_2()
            .gap_1()
            .bg(rgb(colors.panel))
            .border_r_1()
            .border_color(rgb(colors.border))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(theme::scaled(theme::SMALL_TEXT))
                    .text_color(rgb(colors.muted))
                    .child("Settings"),
            )
            .children(SettingsSection::ALL.map(|entry| {
                ControlSpec::new(
                    format!("settings-section-{}", entry.key()),
                    &self.controls,
                    cx,
                )
                .enabled(!self.closing)
                .full_width()
                .emphasis(if entry == section {
                    Emphasis::Selected
                } else {
                    Emphasis::Ghost
                })
                .icon_text(
                    section_icon(entry),
                    entry.title(),
                    move |view, window, cx| view.open_settings(entry, window, cx),
                )
            }));
        let body = match section {
            SettingsSection::Appearance => self.appearance_settings(cx),
            SettingsSection::Accounts => self.account_settings(cx),
            SettingsSection::Layout => self.layout_settings(cx),
            SettingsSection::Updates => self.update_settings(cx),
        };
        let content = div()
            .id("settings-content")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .bg(rgb(colors.background))
            .child(
                theme::column()
                    .w(px(content_width))
                    .px_6()
                    .py_4()
                    .gap_5()
                    .child(
                        theme::row()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(theme::scaled(theme::HEADING_TEXT))
                                    .text_color(rgb(colors.text))
                                    .child(section.title()),
                            )
                            .child(
                                ControlSpec::new("settings-close", &self.controls, cx)
                                    .tooltip("Close settings (Esc)")
                                    .icon(Icon::Close, |view, window, cx| {
                                        view.close_settings(window, cx)
                                    }),
                            ),
                    )
                    .children(body),
            );
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .key_context("Settings")
            .track_focus(&self.settings_focus)
            .on_action(
                cx.listener(|view, _: &CloseSettings, window, cx| view.close_settings(window, cx)),
            )
            .child(navigation)
            .child(content)
            .into_any_element()
    }

    fn appearance_settings(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let appearance = &self.settings.appearance;
        let size = appearance.text_size;
        vec![
            setting(
                "Theme",
                "Colours for the whole window. Gruvbox Dark is the default.",
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(Theme::ALL.map(|choice| self.theme_card(choice, cx)))
                    .into_any_element(),
            ),
            setting(
                "Interface font",
                "Used for menus, buttons, conversations, and the Markdown preview.",
                self.font_picker_control(FontSlot::Interface, cx),
            ),
            setting(
                "Code font",
                "Used for the editor, the terminal, diffs, and code blocks.",
                self.font_picker_control(FontSlot::Code, cx),
            ),
            setting(
                "Text size",
                "Everything grows with it. Ctrl+- and Ctrl+= change it anywhere; Ctrl+0 resets it.",
                theme::row()
                    .gap_1()
                    .child(
                        ControlSpec::new("text-smaller", &self.controls, cx)
                            .enabled(!self.closing && size > Appearance::MIN_TEXT_SIZE)
                            .emphasis(Emphasis::Normal)
                            .tooltip("Smaller text (Ctrl+-)")
                            .icon_text(Icon::SmallerText, "Smaller", |view, _, cx| {
                                view.step_text_size(-1., cx)
                            }),
                    )
                    .child(
                        div()
                            .min_w(theme::scaled(56.))
                            .flex()
                            .justify_center()
                            .text_color(rgb(theme::colors().text))
                            .child(format!("{size:.0} px")),
                    )
                    .child(
                        ControlSpec::new("text-larger", &self.controls, cx)
                            .enabled(!self.closing && size < Appearance::MAX_TEXT_SIZE)
                            .tooltip("Larger text (Ctrl+=)")
                            .icon_text(Icon::LargerText, "Larger", |view, _, cx| {
                                view.step_text_size(1., cx)
                            }),
                    )
                    .child(div().w(theme::scaled(8.)))
                    .child(
                        ControlSpec::new("text-default", &self.controls, cx)
                            .enabled(!self.closing && size != Appearance::DEFAULT_TEXT_SIZE)
                            .tooltip("Default text size (Ctrl+0)")
                            .text("Default (13 px)", |view, _, cx| view.default_text_size(cx)),
                    )
                    .into_any_element(),
            ),
        ]
    }

    fn theme_card(&self, choice: Theme, cx: &Context<Self>) -> AnyElement {
        let current = theme::colors();
        let palette = theme::palette(choice);
        let selected = self.settings.appearance.theme == choice;
        let id = format!("theme-{}", choice.key());
        let swatch = |color: u32, width: f32| {
            div()
                .h(theme::scaled(6.))
                .w(theme::scaled(width))
                .rounded(px(2.))
                .bg(rgb(color))
        };
        div()
            .id(gpui::SharedString::from(id.clone()))
            .relative()
            .w(theme::scaled(150.))
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .rounded(theme::scaled(theme::RADIUS + 2.))
            .border_2()
            .border_color(rgb(if selected {
                current.accent
            } else {
                current.border
            }))
            .bg(rgb(current.panel))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(theme::colors().hover)))
            .child(
                theme::column()
                    .h(theme::scaled(62.))
                    .p_2()
                    .gap_1()
                    .rounded(theme::scaled(theme::RADIUS))
                    .bg(rgb(palette.background))
                    .border_1()
                    .border_color(rgb(palette.border))
                    .child(swatch(palette.text, 70.))
                    .child(swatch(palette.muted, 96.))
                    .child(
                        theme::row()
                            .gap_1()
                            .child(swatch(palette.accent, 30.))
                            .child(swatch(palette.walnut, 22.))
                            .child(swatch(palette.error, 14.)),
                    )
                    .child(div().flex_1())
                    .child(swatch(palette.panel, 110.)),
            )
            .child(
                theme::row()
                    .gap_1()
                    .text_size(theme::scaled(theme::SMALL_TEXT))
                    .text_color(rgb(if selected {
                        current.text
                    } else {
                        current.soft_text
                    }))
                    .child(theme::clipped(choice.label()))
                    .when(selected, |row| row.child(icon(Icon::Check, current.accent))),
            )
            .child(measured(id, !self.closing, &self.controls))
            .when(!self.closing, |card| {
                card.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, _, cx| {
                        cx.stop_propagation();
                        view.choose_theme(choice, cx)
                    }),
                )
            })
            .into_any_element()
    }

    fn font_picker_control(&self, slot: FontSlot, cx: &Context<Self>) -> AnyElement {
        let colors = theme::colors();
        let chosen = match slot {
            FontSlot::Interface => self.settings.appearance.interface_font.clone(),
            FontSlot::Code => self.settings.appearance.code_font.clone(),
        };
        let shown = chosen
            .clone()
            .unwrap_or_else(|| format!("System default ({})", slot.system_font()));
        let open = self.font_picker == Some(slot);
        let sample_font = chosen
            .clone()
            .unwrap_or_else(|| slot.system_font().to_owned());
        let picker = ControlSpec::new(format!("font-{}-toggle", slot.key()), &self.controls, cx)
            .enabled(!self.closing)
            .emphasis(if open {
                Emphasis::Selected
            } else {
                Emphasis::Normal
            })
            .tooltip(format!("Choose the {} font", slot.key()))
            .text_icon(shown, Icon::ChevronDown, move |view, _, cx| {
                view.toggle_font_picker(slot, cx)
            });
        let list = open.then(|| {
            let names = self.font_names.as_deref().unwrap_or_default();
            let missing = chosen
                .as_ref()
                .filter(|name| !names.contains(name))
                .map(|name| format!("{name} (not installed)"));
            let option = |id: String, label: String, selected: bool, value: Option<String>| {
                div()
                    .id(gpui::SharedString::from(id.clone()))
                    .relative()
                    .h(theme::scaled(theme::ROW_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .cursor_pointer()
                    .when(selected, |row| row.bg(rgb(colors.active)))
                    .hover(|style| style.bg(rgb(theme::colors().hover)))
                    .child(
                        div()
                            .w(theme::scaled(theme::ICON_SIZE))
                            .flex_shrink_0()
                            .children(selected.then(|| icon(Icon::Check, colors.accent))),
                    )
                    .child(theme::clipped(label))
                    .child(measured(id, true, &self.controls))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            view.choose_font(slot, value.clone(), cx)
                        }),
                    )
            };
            div()
                .id(gpui::SharedString::from(format!(
                    "font-{}-list",
                    slot.key()
                )))
                .max_h(theme::scaled(260.))
                .overflow_y_scroll()
                .py_1()
                .rounded(theme::scaled(theme::RADIUS))
                .border_1()
                .border_color(rgb(colors.border))
                .bg(rgb(colors.raised))
                .child(option(
                    format!("font-{}-default", slot.key()),
                    format!("System default ({})", slot.system_font()),
                    chosen.is_none(),
                    None,
                ))
                .children(
                    missing.map(|label| div().px_2().text_color(rgb(colors.warning)).child(label)),
                )
                .children(names.iter().map(|name| {
                    option(
                        format!("font-{}-option:{name}", slot.key()),
                        name.clone(),
                        chosen.as_deref() == Some(name.as_str()),
                        Some(name.clone()),
                    )
                }))
        });
        theme::column()
            .gap_2()
            .child(div().flex().child(picker))
            .children(list)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded(theme::scaled(theme::RADIUS))
                    .bg(rgb(colors.panel))
                    .font_family(sample_font)
                    .text_color(rgb(colors.soft_text))
                    .child(FONT_SAMPLE),
            )
            .into_any_element()
    }

    fn account_settings(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let colors = theme::colors();
        // A definite width lets the paragraph measure its wrapped height before layout.
        let intro = div()
            .w_full()
            .whitespace_normal()
            .text_color(rgb(colors.muted))
            .child(
                "Orin has no account of its own. Each agent signs in with its own tool; this page \
                 shows what each agent reports and can start its sign-in in the Terminal column.",
            );
        let refresh = ControlSpec::new("account-refresh", &self.controls, cx)
            .enabled(!self.accounts_checking && !self.closing)
            .tooltip("Ask each agent for its sign-in status again")
            .icon_text(
                Icon::Refresh,
                if self.accounts_checking {
                    "Checking…"
                } else {
                    "Check again"
                },
                |view, _, cx| view.refresh_tools(cx),
            );
        let cards = Provider::ALL.map(|provider| self.account_card(provider, cx));
        let mut elements = vec![
            intro.into_any_element(),
            div().flex().child(refresh).into_any_element(),
        ];
        elements.extend(cards);
        elements.push(setting(
            "Tool folders",
            "Orin runs the agents' command-line tools, not their VS Code extensions, and needs \
             Node.js to run their adapters. If a tool is installed outside the usual places, add \
             the folder that contains claude, codex, gemini, or npx.",
            self.tool_folders(cx),
        ));
        elements
    }

    /// The saved tool folders, each removable, then the button that adds one.
    fn tool_folders(&self, cx: &Context<Self>) -> AnyElement {
        let colors = theme::colors();
        let folders = self
            .settings
            .tool_directories
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let remove = path.clone();
                theme::row()
                    .h(theme::scaled(30.))
                    .gap_2()
                    .pl_3()
                    .pr_1()
                    .rounded(theme::scaled(theme::RADIUS))
                    .border_1()
                    .border_color(rgb(colors.border))
                    .bg(rgb(colors.panel))
                    .child(icon(Icon::Folder, colors.walnut))
                    .child(theme::clipped(
                        crate::process::launch::native_path(path)
                            .display()
                            .to_string(),
                    ))
                    .child(
                        ControlSpec::new(
                            format!("agent-remove-tool-folder-{index}"),
                            &self.controls,
                            cx,
                        )
                        .enabled(!self.closing)
                        .tooltip("Remove this tool folder")
                        .icon(Icon::Close, move |view, _, cx| {
                            view.settings
                                .tool_directories
                                .retain(|entry| entry != &remove);
                            view.changed(cx);
                            view.refresh_tools(cx);
                        }),
                    )
            });
        let full = self.settings.tool_directories.len() >= MAX_TOOL_DIRECTORIES;
        theme::column()
            .gap_2()
            .children(folders)
            .child(
                div().flex().child(
                    ControlSpec::new("agent-add-tool-folder", &self.controls, cx)
                        .enabled(!self.closing && !full)
                        .tooltip(if full {
                            "Remove a folder before adding another"
                        } else {
                            "Add a folder to search for agent tools"
                        })
                        .icon_text(Icon::Plus, "Add tool folder…", |view, _, cx| {
                            view.choose_tool_directory(cx)
                        }),
                ),
            )
            .into_any_element()
    }

    fn account_card(&self, provider: Provider, cx: &Context<Self>) -> AnyElement {
        let colors = theme::colors();
        let account = self.account(provider);
        let (dot, status, detail) = match account.map(|account| &account.sign_in) {
            None if self.accounts_checking => (colors.faint, "Checking…".to_owned(), None),
            None => (colors.faint, "Not checked yet".to_owned(), None),
            Some(SignIn::SignedIn(detail)) => {
                (colors.accent, "Signed in".to_owned(), detail.clone())
            }
            Some(SignIn::SignedOut) => (colors.warning, "Not signed in".to_owned(), None),
            Some(SignIn::NoCli) => (
                colors.faint,
                format!("The `{}` command is not installed", provider.cli()),
                None,
            ),
            Some(SignIn::Unknown(reason)) => (
                colors.error,
                "Status unknown".to_owned(),
                Some(reason.clone()),
            ),
        };
        let signed_out = matches!(
            account.map(|account| &account.sign_in),
            Some(SignIn::SignedOut)
        );
        let signed_in = matches!(
            account.map(|account| &account.sign_in),
            Some(SignIn::SignedIn(_))
        );
        let installed = account.is_some_and(|account| account.cli.is_some());
        let tooltip = match account.and_then(|account| account.cli.as_deref()) {
            _ if self.project.is_none() => {
                "Open a project first: the sign-in runs in the Terminal column".to_owned()
            }
            Some(cli) => match accounts::sign_in_command_line(provider, cli) {
                Some(line) => format!("Run `{line}` in the Terminal column"),
                None => "Its path contains %, so run its sign-in yourself in the Terminal column"
                    .to_owned(),
            },
            None => format!("Install the `{}` command to sign in", provider.cli()),
        };
        let id = format!("account-card-{}", provider_id(provider));
        theme::row()
            .id(gpui::SharedString::from(id.clone()))
            .relative()
            .gap_3()
            .px_3()
            .py_2()
            .rounded(theme::scaled(theme::RADIUS + 2.))
            .border_1()
            .border_color(rgb(colors.border))
            .bg(rgb(colors.panel))
            .child(icon(super::controls::agent_icon(provider), colors.text))
            .child(
                // Single lines that clip rather than wrap, so a card never grows taller than two lines.
                theme::column()
                    .flex_1()
                    .gap(px(2.))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(colors.text))
                            .child(provider.label()),
                    )
                    .child(
                        theme::row()
                            .gap_2()
                            .text_size(theme::scaled(theme::SMALL_TEXT))
                            .text_color(rgb(colors.soft_text))
                            .child(
                                div()
                                    .size(theme::scaled(7.))
                                    .rounded_full()
                                    .flex_shrink_0()
                                    .bg(rgb(dot)),
                            )
                            .child(div().flex_shrink_0().child(status))
                            .children(detail.map(|detail| {
                                theme::clipped(detail).text_color(rgb(colors.muted))
                            })),
                    ),
            )
            .children((account.is_some() && !installed).then(|| {
                ControlSpec::new(
                    format!("agent-install-{}", provider_id(provider)),
                    &self.controls,
                    cx,
                )
                .tooltip(format!(
                    "Open the {} installation guide in your browser",
                    provider.label()
                ))
                .icon_text(Icon::Guide, "Install guide", move |_, _, cx| {
                    cx.open_url(provider.setup_url())
                })
            }))
            .child(
                div()
                    .id(gpui::SharedString::from(format!("{id}-sign-in-hint")))
                    .tooltip(hint(tooltip))
                    .child(
                        ControlSpec::new(
                            format!("account-sign-in-{}", provider_id(provider)),
                            &self.controls,
                            cx,
                        )
                        .enabled(installed && self.project.is_some() && !self.closing)
                        .emphasis(if signed_out {
                            Emphasis::Primary
                        } else {
                            Emphasis::Normal
                        })
                        .text(
                            if signed_in {
                                "Sign in again"
                            } else {
                                "Sign in"
                            },
                            move |view, window, cx| view.sign_in(provider, window, cx),
                        ),
                    ),
            )
            .child(measured(id, true, &self.controls))
            .into_any_element()
    }

    fn layout_settings(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        vec![setting(
            "Columns",
            "Drag a column's title to move it, drag a divider to resize, and use « to hide a column. \
             Reset puts Files, Code, Agents, and Terminal back in order at their default widths.",
            div()
                .flex()
                .child(
                    ControlSpec::new("reset-layout", &self.controls, cx)
                        .enabled(!self.closing && self.settings.columns != Columns::default())
                        .tooltip("Reset column layout")
                        .icon_text(Icon::ResetLayout, "Reset column layout", |view, _, cx| {
                            view.reset_layout(cx)
                        }),
                )
                .into_any_element(),
        )]
    }

    fn update_settings(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let colors = theme::colors();
        let target = crate::update::target::release_target();
        let built = target.map_or("built from source".to_owned(), |target| {
            format!("for {target}")
        });
        let automatic = self.settings.updates.check_automatically;
        let available = matches!(
            self.update.status,
            Some(crate::update::check::UpdateStatus::Available { .. })
        );
        let busy = self.update.downloading.is_some() || self.update.installed.is_some();
        let mut actions = theme::row().gap_2().child(
            ControlSpec::new("update-check-now", &self.controls, cx)
                .enabled(target.is_some() && !self.update.checking && !busy && !self.closing)
                .tooltip("Ask for the newest release now")
                .icon_text(
                    Icon::Refresh,
                    if self.update.checking {
                        "Checking…"
                    } else {
                        "Check now"
                    },
                    |view, _, cx| view.check_for_updates(true, cx),
                ),
        );
        if available || self.update.downloading.is_some() {
            actions = actions.child(self.install_button(cx));
        }
        if let Some(skipped) = &self.settings.updates.skipped_version {
            actions = actions.child(
                ControlSpec::new("update-unskip", &self.controls, cx)
                    .enabled(target.is_some() && !self.update.checking && !busy && !self.closing)
                    .tooltip("Offer this version again and check now")
                    .text(format!("Stop skipping {skipped}"), |view, _, cx| {
                        view.unskip_update(cx)
                    }),
            );
        }
        vec![
            setting(
                "Version",
                "The version of Orin you are running, and which build it is.",
                div()
                    .text_color(rgb(colors.text))
                    .child(format!("Orin {} · {built}", env!("CARGO_PKG_VERSION")))
                    .into_any_element(),
            ),
            setting(
                "Check for updates automatically",
                "Once a day, Orin asks GitHub (or GitLab, if GitHub can't be reached) for its newest \
                 release. Nothing about you or your projects is sent. Off means Orin only checks when \
                 you press Check now.",
                div()
                    .flex()
                    .child(
                        ControlSpec::new("update-auto", &self.controls, cx)
                            .enabled(target.is_some() && !self.closing)
                            .emphasis(if automatic && target.is_some() {
                                Emphasis::Selected
                            } else {
                                Emphasis::Normal
                            })
                            .tooltip("Check for updates automatically")
                            .icon_text(
                                if automatic {
                                    Icon::Checked
                                } else {
                                    Icon::Unchecked
                                },
                                "Check for updates automatically",
                                |view, _, cx| view.toggle_update_checks(cx),
                            ),
                    )
                    .into_any_element(),
            ),
            setting(
                "Status",
                "The result of the last check.",
                theme::column()
                    .gap_2()
                    .child(
                        div()
                            .id("update-status")
                            .w_full()
                            .whitespace_normal()
                            .text_color(rgb(colors.soft_text))
                            .child(self.update_status_text()),
                    )
                    .child(actions)
                    .into_any_element(),
            ),
        ]
    }

    /// **Update and restart**, shared by the banner and Settings › Updates; **Cancel** while
    /// downloading.
    pub fn install_button(&self, cx: &Context<Self>) -> AnyElement {
        if self.update.downloading.is_some() {
            let cancelling = self
                .update
                .cancel
                .load(std::sync::atomic::Ordering::Acquire);
            return ControlSpec::new("update-cancel", &self.controls, cx)
                .enabled(!cancelling && !self.closing)
                .tooltip("Stop the download; nothing is installed")
                .text("Cancel", |view, _, cx| view.cancel_update(cx));
        }
        let busy = self.update.installed.is_some();
        ControlSpec::new("update-install", &self.controls, cx)
            .enabled(!busy && !self.closing)
            .emphasis(Emphasis::Primary)
            .tooltip("Download, verify, and restart into the new version")
            .text("Update and restart", |view, _, cx| view.install_update(cx))
    }
}

/// GPUI 0.2.2 lists its own fallback families whether or not they are installed, and a missing
/// family can't be drawn, so only those known to ship with this operating system are offered.
fn installed_font(name: &str) -> bool {
    const GPUI_FALLBACKS: [&str; 8] = [
        "Helvetica",
        "Segoe UI",
        "Ubuntu",
        "Adwaita Sans",
        "Cantarell",
        "Noto Sans",
        "DejaVu Sans",
        "Arial",
    ];
    let shipped: &[&str] = if cfg!(target_os = "windows") {
        &["Segoe UI", "Arial"]
    } else if cfg!(target_os = "macos") {
        &["Helvetica", "Arial"]
    } else {
        &[]
    };
    !name.trim().is_empty()
        && !name.starts_with('.')
        && (!GPUI_FALLBACKS.contains(&name) || shipped.contains(&name))
}

fn section_icon(section: SettingsSection) -> Icon {
    match section {
        SettingsSection::Appearance => Icon::Preview,
        SettingsSection::Accounts => Icon::Account,
        SettingsSection::Layout => Icon::ResetLayout,
        SettingsSection::Updates => Icon::Refresh,
    }
}

/// One setting: a name, a sentence about it, and its control below.
fn setting(name: &'static str, description: &'static str, control: AnyElement) -> AnyElement {
    let colors = theme::colors();
    theme::column()
        .gap_2()
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(colors.text))
                .child(name),
        )
        .child(
            div()
                .w_full()
                .whitespace_normal()
                .text_size(theme::scaled(theme::SMALL_TEXT))
                .text_color(rgb(colors.muted))
                .child(description),
        )
        .child(control)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_font_list_leaves_out_hidden_and_unshipped_fallbacks() {
        assert!(!installed_font(".SystemUIFont"));
        assert!(!installed_font(".ZedMono"));
        assert!(!installed_font("Adwaita Sans"));
        assert!(!installed_font("  "));
        assert!(installed_font("Consolas"));
        assert_eq!(installed_font("Segoe UI"), cfg!(target_os = "windows"));
    }
}
