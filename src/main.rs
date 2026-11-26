//! IDE — a fast, native code editor written in Rust on GPUI.

mod ai;
mod assets;
mod browser;
mod explorer;
mod formatter;
mod http_client;
mod icons;
mod live_server;
mod palette;
mod runner;
mod search;
mod settings;
mod terminal;
mod util;
mod workspace;

use gpui_kit::component::{Theme, ThemeRegistry, TitleBar};
use gpui_kit::*;

use crate::settings::Settings;
use crate::workspace::Workspace;

/// Apply one of the bundled themes by name ("Midnight" or "Daylight").
pub fn apply_theme(name: &str, cx: &mut App) {
    let config = ThemeRegistry::global(cx)
        .themes()
        .get(&SharedString::from(name.to_string()))
        .cloned()
        .or_else(|| {
            ThemeRegistry::global(cx)
                .themes()
                .get(&SharedString::from("Midnight"))
                .cloned()
        });
    if let Some(config) = config {
        Theme::update(cx, |theme| theme.apply_config(&config));
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);

            if let Err(err) = cx.text_system().add_fonts(assets::font_bytes()) {
                log::warn!("failed to load bundled fonts: {err}");
            }
            if let Err(err) =
                ThemeRegistry::global_mut(cx).load_themes_from_str(&assets::theme_json())
            {
                log::warn!("failed to load themes: {err}");
            }

            let settings = Settings::load();
            apply_theme(&settings.theme, cx);
            cx.set_global(settings);

            workspace::bind_keys(cx);
            workspace::set_menus(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);

            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(720.), px(480.))),
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                let ws = cx.new(|cx| Workspace::new(window, cx));
                let focus = ws.focus_handle(cx);
                window.defer(cx, move |window, cx| focus.focus(window, cx));
                ws
            })
            .expect("failed to open window");
        });
}
