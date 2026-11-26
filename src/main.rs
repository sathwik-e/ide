//! Eclipse — everything from the cosmos. A fast, native code editor in Rust.

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
mod settings_view;
mod terminal;
mod util;
mod workspace;

use gpui_kit::component::{Theme, ThemeRegistry, TitleBar};
use gpui_kit::*;

use crate::settings::Settings;
use crate::workspace::Workspace;

pub const APP_NAME: &str = "Eclipse";
pub const TAGLINE: &str = "everything from the cosmos";

/// Accent swatches offered in Settings: (name, hex).
pub const ACCENTS: &[(&str, &str)] = &[
    ("Corona", "#f2b36d"),
    ("Nebula", "#b48cff"),
    ("Aurora", "#4fd8b8"),
    ("Comet", "#7c9cff"),
    ("Mars", "#ff6b6b"),
    ("Pulsar", "#ff7ac6"),
];

fn parse_hex(hex: &str) -> Option<Hsla> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    u32::from_str_radix(h, 16).ok().map(|v| rgb(v).into())
}

/// Apply the theme, accent and editor font from settings.
pub fn apply_theme(settings: &Settings, cx: &mut App) {
    let registry = ThemeRegistry::global(cx).themes().clone();
    let config = registry
        .get(&SharedString::from(settings.theme.clone()))
        .or_else(|| registry.get(&SharedString::from("Eclipse")))
        .cloned();
    let accent = parse_hex(&settings.accent);
    let font: SharedString = settings.editor_font.clone().into();
    Theme::update(cx, move |theme| {
        if let Some(config) = config {
            theme.apply_config(&config);
        }
        theme.mono_font_family = font;
        if let Some(a) = accent {
            let dark = theme.is_dark();
            let hover = if dark { a.blend(white().opacity(0.12)) } else { a.blend(black().opacity(0.08)) };
            theme.primary = a;
            theme.primary_hover = hover;
            theme.primary_active = a.blend(black().opacity(0.1));
            theme.button_primary = a;
            theme.button_primary_hover = hover;
            theme.button_primary_active = a.blend(black().opacity(0.1));
            theme.ring = a;
            theme.caret = a;
            theme.link = a;
            theme.link_hover = hover;
            theme.link_active = a;
            theme.list_active = a.opacity(0.16);
            theme.list_highlight_foreground = a;
            theme.selection = a.opacity(0.28);
            theme.sidebar_primary = a;
            theme.progress_bar = a;
            theme.slider_bar = a;
            theme.drag_border = a;
        }
    });
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
            apply_theme(&settings, cx);
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
                window.set_window_title(APP_NAME);
                let ws = cx.new(|cx| Workspace::new(window, cx));
                let focus = ws.focus_handle(cx);
                window.defer(cx, move |window, cx| focus.focus(window, cx));
                ws
            })
            .expect("failed to open window");
        });
}
