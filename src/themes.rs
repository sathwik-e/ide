//! Theme engine. Every theme, including the user's custom one, is derived
//! from a compact palette so all UI tokens and syntax colors stay consistent.

use gpui_kit::component::{ThemeConfig, ThemeRegistry};
use gpui_kit::App;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::settings::Settings;

/// A theme in palette form. Colors are "#rrggbb".
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Palette {
    pub name: String,
    pub dark: bool,
    pub background: String,
    pub sidebar: String,
    pub text: String,
    pub muted: String,
    pub accent: String,
    pub keyword: String,
    pub function: String,
    pub string: String,
    pub number: String,
    pub types: String,
    pub comment: String,
    pub tag: String,
    pub property: String,
    pub operator: String,
}

impl Default for Palette {
    fn default() -> Self {
        builtin()[0].clone().renamed("Custom")
    }
}

impl Palette {
    pub fn renamed(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }
}

#[allow(clippy::too_many_arguments)]
fn p(
    name: &str,
    dark: bool,
    [background, sidebar, text, muted, accent]: [&str; 5],
    [keyword, function, string, number, types, comment, tag, property, operator]: [&str; 9],
) -> Palette {
    Palette {
        name: name.into(),
        dark,
        background: background.into(),
        sidebar: sidebar.into(),
        text: text.into(),
        muted: muted.into(),
        accent: accent.into(),
        keyword: keyword.into(),
        function: function.into(),
        string: string.into(),
        number: number.into(),
        types: types.into(),
        comment: comment.into(),
        tag: tag.into(),
        property: property.into(),
        operator: operator.into(),
    }
}

/// Built-in themes, in display order.
pub fn builtin() -> Vec<Palette> {
    vec![
        p("Orion", true, ["#0e0f13", "#0a0b0e", "#d8d9df", "#6c7080", "#8ab4ff"],
          ["#c4a0ff", "#8ab4ff", "#a8d995", "#f2b36d", "#ffd27a", "#4f5464", "#ff8a8a", "#9cc7ff", "#89ddff"]),
        p("Nebula", true, ["#110f1c", "#0c0a15", "#dcd7ee", "#7a7396", "#b48cff"],
          ["#ff8bd0", "#9bb8ff", "#b5e6a3", "#ffb38a", "#e6c0ff", "#5a5378", "#ff8bd0", "#a9c8ff", "#8be9fd"]),
        p("Aurora", true, ["#0b1214", "#080e10", "#d3e4e4", "#65807f", "#4fd8b8"],
          ["#7dd8ff", "#7ee8c3", "#d6e88a", "#ffb27a", "#9fd5ff", "#4a6262", "#ff8f8f", "#a6e3d6", "#7dd8ff"]),
        p("Midnight", true, ["#0f1116", "#0b0d11", "#d5d8e0", "#6b7180", "#7c9cff"],
          ["#c49bff", "#82aaff", "#a8d995", "#f0a36e", "#ffcb6b", "#545a68", "#f2727f", "#9cc7ff", "#89ddff"]),
        p("Supernova", true, ["#140f0e", "#0f0b0a", "#eadcd6", "#8a7670", "#ff7a59"],
          ["#ff9f7a", "#ffd28a", "#c3e08f", "#ff7a59", "#ffc0a8", "#64544f", "#ff7a8a", "#f4c7a1", "#ffb38a"]),
        p("Andromeda", true, ["#0d1020", "#090b17", "#d6dcf5", "#6e76a0", "#5ee2ff"],
          ["#c792ea", "#5ee2ff", "#c3e88d", "#f78c6c", "#ffcb6b", "#545d85", "#ff6e99", "#82aaff", "#89ddff"]),
        p("Event Horizon", true, ["#000000", "#000000", "#e6e6e6", "#6e6e6e", "#ffffff"],
          ["#d0a2ff", "#9ccaff", "#b9e3a2", "#ffb072", "#ffd88a", "#4e4e4e", "#ff8f8f", "#a8d3ff", "#8fe3f5"]),
        p("Mars", true, ["#150d0c", "#100908", "#ead9d4", "#8c6f68", "#ff5e3a"],
          ["#ff8a65", "#ffb74d", "#c5e1a5", "#ff5e3a", "#ffcc80", "#6a4f49", "#ef5350", "#ffab91", "#ffd180"]),
        p("Saturn", true, ["#13110c", "#0e0c08", "#ece3cf", "#8d8268", "#e8c26e"],
          ["#e8c26e", "#a3c9a8", "#d9e3a1", "#f0a868", "#f3d9a4", "#635b49", "#e88d72", "#c9d6b3", "#b8d4c6"]),
        p("Neptune", true, ["#0a1220", "#071019", "#d1e1f5", "#62789a", "#3fa8ff"],
          ["#7fb8ff", "#5fd4ff", "#9fe0c0", "#ffa86a", "#a3c8ff", "#45597a", "#ff7f9f", "#8ec7ff", "#6fe0f0"]),
        p("Pulsar", true, ["#140c16", "#0f0910", "#efd9ee", "#8c6f8a", "#ff5fbf"],
          ["#ff7ad1", "#b88cff", "#b8f2a6", "#ffb26b", "#ffc2e8", "#634a61", "#ff6e8a", "#d9a8ff", "#8fe9ff"]),
        p("Titan", true, ["#141312", "#0f0e0d", "#e2ded8", "#7f7a73", "#d9a46c"],
          ["#d0a8e0", "#9fc0de", "#b8cf96", "#e3a06a", "#e6cc8e", "#5c5852", "#e48a7f", "#b9cbd8", "#9fd0cc"]),
        p("Galaxy", true, ["#0f0d1a", "#0a0913", "#dddaf2", "#726d98", "#8f7cff"],
          ["#b18cff", "#7aa7ff", "#a6e3b5", "#ff9e7a", "#f7c97e", "#4f4a73", "#ff7aa8", "#9bb6ff", "#7fdcff"]),
        p("Comet", true, ["#111315", "#0c0e10", "#d9dde1", "#707880", "#6ee7b7"],
          ["#b5a6ff", "#7dc4ff", "#b3e59a", "#f4b07a", "#f0d58c", "#525a61", "#f48b8b", "#9ad0e6", "#7ee6d1"]),
        p("Lunar", false, ["#fbfbfc", "#f4f5f7", "#1d2130", "#868b98", "#5b6cf0"],
          ["#8b3fd9", "#2f5bea", "#2f8a3a", "#c2541b", "#b5780b", "#9aa0ad", "#d0364b", "#1f6fb2", "#0f8fa8"]),
        p("Polaris", false, ["#f7f9fc", "#eef2f8", "#17202e", "#7d8899", "#1f7aff"],
          ["#7c3aed", "#0b63d6", "#1a7f37", "#c4511b", "#9a6700", "#8c95a3", "#cf222e", "#0969a8", "#0a7b8c"]),
        p("Cosmic Latte", false, ["#fbf7ef", "#f3ecdf", "#2b2620", "#8f8473", "#c27c2c"],
          ["#9a4fb5", "#2b6cb0", "#4c7a2a", "#c25e1b", "#a0670c", "#a39885", "#c2402e", "#356f9c", "#2f8577"]),
        p("Solstice", false, ["#fdf8f7", "#f7eeec", "#2d2224", "#937f82", "#e5566b"],
          ["#a2479b", "#2f6fcf", "#3f8a45", "#d0662a", "#b07a0e", "#a8979a", "#d03a4f", "#3a75a8", "#22808c"]),
    ]
}

fn rgb(h: &str) -> (f32, f32, f32) {
    let h = h.trim().trim_start_matches('#');
    let v = u32::from_str_radix(h.get(0..6).unwrap_or("000000"), 16).unwrap_or(0);
    (((v >> 16) & 255) as f32, ((v >> 8) & 255) as f32, (v & 255) as f32)
}

fn hex((r, g, b): (f32, f32, f32)) -> String {
    let c = |x: f32| x.round().clamp(0., 255.) as u8;
    format!("#{:02x}{:02x}{:02x}", c(r), c(g), c(b))
}

fn mix(a: &str, b: &str, t: f32) -> String {
    let (a, b) = (rgb(a), rgb(b));
    hex((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, a.2 + (b.2 - a.2) * t))
}

fn alpha(h: &str, a: f32) -> String {
    let base = hex(rgb(h));
    format!("{base}{:02x}", (a * 255.).round().clamp(0., 255.) as u8)
}

/// Turn a palette into a full theme definition.
pub fn build(pal: &Palette, glass_tint: f32) -> Value {
    let dark = pal.dark;
    let (bg, panel, fg, muted, acc) = (&pal.background, &pal.sidebar, &pal.text, &pal.muted, &pal.accent);
    let on_acc = if dark { "#0b0c10" } else { "#ffffff" };
    let ov = if dark { "#ffffff" } else { "#000000" };
    let hover = alpha(ov, if dark { 0.05 } else { 0.045 });
    let active = alpha(ov, if dark { 0.09 } else { 0.07 });
    let surface = alpha(ov, if dark { 0.06 } else { 0.04 });
    let line = alpha(ov, if dark { 0.075 } else { 0.085 });
    let tint = alpha(panel, glass_tint);
    let syn = |c: &str| json!({ "color": c });
    let italic = |c: &str| json!({ "color": c, "font_style": "italic" });
    json!({
        "name": pal.name,
        "mode": if dark { "dark" } else { "light" },
        "font.size": 13,
        "mono_font.family": "JetBrains Mono",
        "mono_font.size": 13,
        "radius": 6,
        "colors": {
            "background": bg, "foreground": fg, "border": line, "window.border": alpha(ov, 0.12),
            "ring": acc, "caret": acc, "overlay": if dark { "#00000066" } else { "#0000002e" },
            "selection.background": alpha(acc, 0.28),
            "accent.background": active, "accent.foreground": fg,
            "primary.background": acc, "primary.hover.background": mix(acc, "#ffffff", 0.12),
            "primary.active.background": mix(acc, "#000000", 0.1), "primary.foreground": on_acc,
            "secondary.background": surface, "secondary.hover.background": hover,
            "secondary.active.background": active, "secondary.foreground": mix(fg, bg, 0.08),
            "muted.background": surface, "muted.foreground": muted, "input.border": alpha(ov, 0.11),
            "popover.background": if dark { mix(bg, fg, 0.05) } else { "#ffffff".into() },
            "popover.foreground": fg,
            "list.background": "#00000000", "list.hover.background": hover,
            "list.active.background": alpha(acc, 0.20), "list.active.border": alpha(acc, 0.0),
            "list.even.background": "#00000000", "list.head.background": "#00000000",
            "list.highlight.foreground": acc,
            "sidebar.background": tint, "sidebar.foreground": mix(fg, bg, 0.12), "sidebar.border": line,
            "sidebar.accent.background": hover, "sidebar.accent.foreground": fg,
            "sidebar.primary.background": acc, "sidebar.primary.foreground": on_acc,
            "tab_bar.background": "#00000000", "tab_bar.segmented.background": surface,
            "tab.background": "#00000000", "tab.foreground": muted,
            "tab.active.background": active, "tab.active.foreground": fg,
            "title_bar.background": "#00000000", "title_bar.border": "#00000000",
            "status_bar.background": "#00000000", "status_bar.border": line,
            "scrollbar.background": "#00000000",
            "scrollbar.thumb.background": if dark { "#ffffff1a" } else { "#00000022" },
            "scrollbar.thumb.hover.background": if dark { "#ffffff30" } else { "#00000036" },
            "table.background": bg, "table.head.background": panel, "table.head.foreground": muted,
            "table.hover.background": hover, "table.row.border": line, "table.even.background": bg,
            "link.foreground": acc, "link.hover.foreground": mix(acc, fg, 0.3), "link.active.foreground": acc,
            "switch.background": alpha(ov, 0.16), "skeleton.background": hover,
            "drag_border": acc, "drop_target.background": alpha(acc, 0.1),
            "progress_bar.background": acc, "slider.bar.background": acc,
            "success.background": "#3fb98a", "success.foreground": on_acc,
            "warning.background": "#e5b567", "warning.foreground": "#0b0c10",
            "danger.background": "#f2727f", "danger.foreground": on_acc,
            "info.background": "#7c9cff", "info.foreground": on_acc,
            "base.red": "#f2727f", "base.green": "#7fd19b", "base.yellow": "#e5c07b",
            "base.blue": "#7c9cff", "base.magenta": "#c49bff", "base.cyan": "#6fd3e6"
        },
        "highlight": {
            "editor.foreground": fg, "editor.background": bg,
            "editor.active_line.background": if dark { "#ffffff07" } else { "#0000000a" },
            "editor.line_number": mix(bg, fg, 0.22), "editor.active_line_number": mix(fg, bg, 0.25),
            "editor.invisible": if dark { "#ffffff1a" } else { "#00000022" },
            "conflict": "#f2727f", "created": "#7fd19b", "deleted": "#f2727f", "error": "#f2727f",
            "hidden": muted, "hint": "#6fd3e6", "ignored": mix(muted, bg, 0.3), "info": "#7c9cff",
            "modified": "#e5c07b", "predictive": mix(muted, bg, 0.15), "renamed": "#7c9cff",
            "success": "#7fd19b", "unreachable": mix(muted, bg, 0.15), "warning": "#e5c07b",
            "syntax": {
                "attribute": syn(&pal.types), "boolean": syn(&pal.number),
                "comment": italic(&pal.comment), "comment.doc": italic(&mix(&pal.comment, fg, 0.15)),
                "constant": syn(&pal.number), "constructor": syn(&pal.types), "embedded": syn(fg),
                "function": syn(&pal.function), "keyword": syn(&pal.keyword),
                "link_text": syn(&pal.function), "link_uri": syn(&pal.operator),
                "number": syn(&pal.number), "operator": syn(&pal.operator),
                "punctuation": syn(&mix(fg, bg, 0.35)), "punctuation.bracket": syn(&mix(fg, bg, 0.3)),
                "punctuation.delimiter": syn(&mix(fg, bg, 0.35)), "property": syn(&pal.property),
                "string": syn(&pal.string), "string.escape": syn(&pal.operator),
                "string.regex": syn(&pal.operator), "string.special": syn(&pal.operator),
                "string.special.symbol": syn(&pal.number), "tag": syn(&pal.tag),
                "text.literal": syn(&pal.string),
                "title": { "color": pal.function, "font_weight": 700 },
                "type": syn(&pal.types), "variable": syn(fg), "variable.special": syn(&pal.tag)
            }
        }
    })
}

pub fn themes_dir() -> std::path::PathBuf {
    Settings::dir().join("themes")
}

/// Register built-in, custom and user-folder themes with the registry.
pub fn register_all(settings: &Settings, cx: &mut App) {
    let tint = settings.glass_tint();
    let mut palettes = builtin();
    palettes.push(settings.custom_theme.clone().renamed("Custom"));
    for pal in &palettes {
        match serde_json::from_value::<ThemeConfig>(build(pal, tint)) {
            Ok(config) => ThemeRegistry::global_mut(cx).upsert_theme(config),
            Err(err) => log::warn!("theme {} failed to build: {err}", pal.name),
        }
    }
    // Hand-written themes: any *.json theme set in the themes folder.
    if let Ok(entries) = std::fs::read_dir(themes_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        for theme in value.get("themes").and_then(|t| t.as_array()).cloned().unwrap_or_default() {
                            if let Ok(config) = serde_json::from_value::<ThemeConfig>(theme) {
                                ThemeRegistry::global_mut(cx).upsert_theme(config);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Theme names in display order: built-ins, Custom, then user themes.
pub fn ordered_names(cx: &App) -> Vec<String> {
    let mut names: Vec<String> = builtin().into_iter().map(|p| p.name).collect();
    names.push("Custom".into());
    let mut extra: Vec<String> = ThemeRegistry::global(cx)
        .themes()
        .keys()
        .map(|k| k.to_string())
        .filter(|k| !names.contains(k) && !k.starts_with("Default"))
        .collect();
    extra.sort();
    names.extend(extra);
    names
}

/// Preview colors (bg, sidebar, text, accent, muted) for a registered theme.
pub fn preview(name: &str, cx: &App) -> Option<[String; 5]> {
    let registry = ThemeRegistry::global(cx);
    let config = registry.themes().get(name)?;
    let c = &config.colors;
    let get = |v: &Option<gpui_kit::SharedString>, fallback: &str| {
        v.as_ref()
            .map(|s| s.to_string().chars().take(7).collect::<String>())
            .unwrap_or_else(|| fallback.to_string())
    };
    let bg = get(&c.background, "#111111");
    Some([
        bg.clone(),
        get(&c.sidebar, &bg),
        get(&c.foreground, "#dddddd"),
        get(&c.primary, "#7c9cff"),
        get(&c.muted_foreground, "#777777"),
    ])
}

/// Palette for a theme name, if it's built-in (used to seed the custom theme).
pub fn palette_for(name: &str, settings: &Settings) -> Palette {
    if name == "Custom" {
        return settings.custom_theme.clone();
    }
    builtin()
        .into_iter()
        .find(|p| p.name == name)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_builds_a_valid_theme() {
        let mut all = builtin();
        all.push(Palette::default());
        assert!(all.len() >= 18, "expected at least 18 themes");
        let mut names = std::collections::HashSet::new();
        for pal in &all {
            assert!(names.insert(pal.name.clone()), "duplicate theme {}", pal.name);
            let config: ThemeConfig = serde_json::from_value(build(pal, 0.68))
                .unwrap_or_else(|e| panic!("{} invalid: {e}", pal.name));
            assert_eq!(config.name.as_ref(), pal.name);
            assert!(config.colors.background.is_some() && config.highlight.is_some());
        }
    }

    #[test]
    fn color_helpers() {
        assert_eq!(mix("#000000", "#ffffff", 0.5), "#808080");
        assert_eq!(alpha("#ff0000", 0.5), "#ff000080");
    }
}
