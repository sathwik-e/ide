#![recursion_limit = "512"]
//! Orion IDE — a fast, native code editor in Rust.

mod ai;
mod assets;
mod browser;
mod editing;
mod explorer;
mod formatter;
mod git;
mod http_client;
mod icons;
mod live_server;
mod lsp;
mod opener;
mod palette;
mod runner;
mod search;
mod settings;
mod settings_view;
mod symbols;
mod tasks;
mod terminal;
mod themes;
mod util;
mod workspace;

use gpui_kit::component::{Theme, ThemeRegistry, TitleBar, TransparentRoot};
use gpui_kit::*;

use crate::settings::Settings;
use crate::workspace::Workspace;

pub const APP_NAME: &str = "Orion";

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
    themes::register_all(settings, cx);
    let registry = ThemeRegistry::global(cx).themes().clone();
    let config = registry
        .get(&SharedString::from(settings.theme.clone()))
        .or_else(|| registry.get(&SharedString::from("Orion")))
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
    // Log to a file as well: a Finder-launched app has nowhere to print.
    let mut logger = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    let _ = std::fs::create_dir_all(Settings::dir());
    if let Ok(file) = std::fs::File::create(Settings::dir().join("orion.log")) {
        logger.target(env_logger::Target::Pipe(Box::new(file)));
    }
    logger.init();

    // Files macOS asks us to open (Finder, Dock, `open -a`) arrive here,
    // possibly before the window exists; they're queued and delivered.
    let incoming = opener::Incoming::default();
    let (wake_tx, wake_rx) = smol::channel::unbounded::<()>();
    let app = gpui_kit::application().with_assets(assets::Assets);
    {
        let incoming = incoming.clone();
        app.on_open_urls(move |urls| {
            if let Ok(mut q) = incoming.0.lock() {
                q.extend(opener::paths_from_urls(&urls));
            }
            let _ = wake_tx.try_send(());
        });
    }
    app.on_reopen(|cx| opener::deliver(cx));
    app.run(move |cx| {
            gpui_kit::init(cx);
            cx.set_global(incoming.clone());
            cx.spawn(async move |cx| {
                while wake_rx.recv().await.is_ok() {
                    let _ = cx.update(|cx| opener::deliver(cx));
                }
            })
            .detach();

            if let Err(err) = cx.text_system().add_fonts(assets::font_bytes()) {
                log::warn!("failed to load bundled fonts: {err}");
            }

            let settings = Settings::load();
            apply_theme(&settings, cx);
            let glass = settings.glass;
            cx.set_global(settings);
            // The workspace paints its own (possibly translucent) backdrop.
            cx.set_global(TransparentRoot(true));

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
            let mut options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(720.), px(480.))),
                window_background: if glass {
                    WindowBackgroundAppearance::Blurred
                } else {
                    WindowBackgroundAppearance::Opaque
                },
                ..TitleBar::window_options()
            };
            if let Some(titlebar) = options.titlebar.as_mut() {
                titlebar.traffic_light_position = Some(point(px(16.), px(16.)));
            }
            let (handle, ws) = gpui_kit::open_window(options, cx, |window, cx| {
                window.set_window_title(APP_NAME);
                let ws = cx.new(|cx| Workspace::new(window, cx));
                let focus = ws.focus_handle(cx);
                window.defer(cx, move |window, cx| focus.focus(window, cx));
                ws
            })
            .expect("failed to open window");
            cx.set_global(opener::OpenTarget {
                window: handle,
                workspace: ws.downgrade(),
            });
        });
}
