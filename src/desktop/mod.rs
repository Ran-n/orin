// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 15:18:29.082066
//+ Revised: 	2026/11/12 22:43:39.330230
// -----------------------------------------------------------------------------

//! GPUI desktop entry point; domain services have no dependency on this module.

mod activity_bar;
mod agent_actions;
mod agent_layout;
mod agent_view;
mod agent_workspace;
mod assets;
mod code_layout;
mod columns;
mod controls;
mod driver;
mod editor;
mod event_handling;
mod events;
mod explorer;
mod file_actions;
mod file_icons;
mod layout;
mod markdown;
mod markdown_view;
mod menus;
mod project_actions;
mod providers;
mod session_actions;
mod settings_page;
mod shutdown;
mod syntax;
mod terminal_input;
mod terminal_view;
mod theme;
mod update_actions;
mod watch;
mod welcome;
mod workspace;

use crate::{
    settings::{SettingsStore, WorkspaceSettings},
    startup::Options,
};
use gpui::{
    App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, actions,
    px, size,
};
use workspace::{Startup, Workspace};

actions!(
    orin,
    [
        Quit,
        OpenProject,
        RefreshProject,
        ToggleTerminal,
        LargerText,
        SmallerText,
        DefaultTextSize,
        OpenSettings,
        CloseSettings,
        CloseMenu,
        ShowWelcome,
        ResetLayout,
        ToggleMarkdownPreview,
    ]
);

/// Delete downloads a crashed run left behind, off the startup path.
fn clean_update_leftovers() {
    let Some(dir) = crate::update::install::download_dir() else {
        return;
    };
    let spawned = std::thread::Builder::new()
        .name("orin-update-cleanup".into())
        .spawn(move || match crate::update::install::clean_leftovers(&dir) {
            Ok(0) => {}
            Ok(removed) => tracing::info!(target: "orin::update", removed, "Leftover update downloads deleted"),
            Err(error) => tracing::warn!(target: "orin::update", %error, "Leftover update downloads could not be deleted"),
        });
    if let Err(error) = spawned {
        tracing::warn!(target: "orin::update", %error, "Update cleanup thread could not start");
    }
}

pub fn run(options: Options) -> Result<(), String> {
    let store = match options.settings.clone() {
        Some(path) => SettingsStore::new(path),
        None => SettingsStore::default_path()
            .map(SettingsStore::new)
            .map_err(|_| {
                "Orin could not locate your configuration folder. Use --settings FILE.".to_owned()
            })?,
    };
    let (loaded, providers) = std::thread::scope(|scope| {
        let settings = scope.spawn(|| store.load());
        let providers = providers::probe_all();
        (settings.join(), providers)
    });
    let (settings, startup_error, writable) = match loaded {
        Ok(Ok(settings)) => (settings, None, true),
        result => {
            tracing::error!(target:"orin::settings", ?result, "Saved workspace could not be restored");
            (WorkspaceSettings::default(), Some("Saved workspace settings could not be read. The original file is preserved; changes will not be saved until it is repaired.".into()), false)
        }
    };
    clean_update_leftovers();
    let failure = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result = failure.clone();
    Application::new().with_assets(assets::Assets).run(move |cx: &mut App| {
        let mut bounds = Bounds::centered(None, size(px(settings.width.clamp(1000.,7680.)), px(settings.height.clamp(680.,4320.))), cx);
        if let Some((x,y))=settings.position{
            let origin=gpui::point(px(x),px(y));
            if cx.displays().iter().any(|display|display.bounds().contains(&origin)){bounds.origin=origin;}
        }
        cx.bind_keys([
            gpui::KeyBinding::new("cmd-q", Quit, None),
            gpui::KeyBinding::new("ctrl-shift-q", Quit, None),
            gpui::KeyBinding::new("ctrl-shift-o", OpenProject, None),
            gpui::KeyBinding::new("cmd-shift-o", OpenProject, None),
            gpui::KeyBinding::new("ctrl-shift-r", RefreshProject, None),
            gpui::KeyBinding::new("ctrl-`", ToggleTerminal, None),
            gpui::KeyBinding::new("ctrl-=", LargerText, None),
            gpui::KeyBinding::new("ctrl-+", LargerText, None),
            gpui::KeyBinding::new("ctrl--", SmallerText, None),
            gpui::KeyBinding::new("ctrl-0", DefaultTextSize, None),
            gpui::KeyBinding::new("cmd-=", LargerText, None),
            gpui::KeyBinding::new("cmd-+", LargerText, None),
            gpui::KeyBinding::new("cmd--", SmallerText, None),
            gpui::KeyBinding::new("cmd-0", DefaultTextSize, None),
            gpui::KeyBinding::new("ctrl-shift-v", ToggleMarkdownPreview, None),
            gpui::KeyBinding::new("ctrl-,", OpenSettings, None),
            gpui::KeyBinding::new("cmd-,", OpenSettings, None),
            gpui::KeyBinding::new("escape", CloseSettings, Some("Settings")),
            gpui::KeyBinding::new("escape", CloseMenu, Some("Menu")),
            gpui::KeyBinding::new("cmd-shift-v", ToggleMarkdownPreview, None),
        ]);
        let should_focus = !options.test_driver;
        let opened = cx.open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(1000.), px(680.))),
            titlebar: Some(TitlebarOptions { title: Some("Orin".into()), ..Default::default() }),
            app_id: Some("com.breren.Orin".into()), focus: should_focus,
            ..Default::default()
        }, move |window, cx| {
            cx.new(|cx| {
                        Workspace::new(
                            Startup {
                                options,
                                store,
                                settings,
                                providers,
                                error: startup_error,
                                writable,
                            },
                            window,
                            cx,
                        )
                    })
        });
        if let Err(error) = opened {
            tracing::error!(target:"orin::desktop", %error, "Window creation failed");
            *result.lock().unwrap_or_else(|poison| poison.into_inner()) = Some("Orin could not open its window. Check your graphics driver and display session.".into());
            cx.quit();
        } else if should_focus { cx.activate(true); }
    });
    failure
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .take()
        .map_or(Ok(()), Err)
}
