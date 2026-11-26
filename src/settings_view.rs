//! Settings, as a tab. Every control applies immediately; the JSON file stays
//! available for anything not shown here.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    h_flex,
    switch::Switch,
    v_flex,
};
use gpui_kit::base::{Disableable as _, Selectable as _};
use gpui_kit::{prelude::FluentBuilder as _, *};

use crate::settings::Settings;
use crate::themes::{self, Palette};

const FONTS: &[&str] = &["JetBrains Mono", "SF Mono", "Menlo", "Monaco"];

/// Editable slots of the custom theme: (label, getter, setter).
type Slot = (&'static str, fn(&Palette) -> &String, fn(&mut Palette, String));
const SLOTS: &[Slot] = &[
    ("Background", |p| &p.background, |p, v| p.background = v),
    ("Sidebar", |p| &p.sidebar, |p, v| p.sidebar = v),
    ("Text", |p| &p.text, |p, v| p.text = v),
    ("Muted text", |p| &p.muted, |p, v| p.muted = v),
    ("Accent", |p| &p.accent, |p, v| p.accent = v),
    ("Keywords", |p| &p.keyword, |p, v| p.keyword = v),
    ("Functions", |p| &p.function, |p, v| p.function = v),
    ("Strings", |p| &p.string, |p, v| p.string = v),
    ("Numbers", |p| &p.number, |p, v| p.number = v),
    ("Types", |p| &p.types, |p, v| p.types = v),
    ("Comments", |p| &p.comment, |p, v| p.comment = v),
    ("Tags", |p| &p.tag, |p, v| p.tag = v),
    ("Properties", |p| &p.property, |p, v| p.property = v),
    ("Operators", |p| &p.operator, |p, v| p.operator = v),
];

fn to_hsla(hex: &str) -> Hsla {
    let h = hex.trim().trim_start_matches('#');
    rgb(u32::from_str_radix(h.get(0..6).unwrap_or("000000"), 16).unwrap_or(0)).into()
}

fn to_hex(c: Hsla) -> String {
    let c = c.to_rgb();
    let b = |x: f32| (x * 255.).round().clamp(0., 255.) as u8;
    format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
}

pub struct SettingsView {
    focus_handle: FocusHandle,
    pickers: Vec<Entity<ColorPickerState>>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let custom = Settings::get(cx).custom_theme.clone();
        let mut pickers = Vec::new();
        let mut subs = vec![cx.observe_global::<Settings>(|_, cx| cx.notify())];
        for (ix, (_, get, _)) in SLOTS.iter().enumerate() {
            let value = to_hsla(get(&custom));
            let state = cx.new(|cx| ColorPickerState::new(window, cx).default_value(value));
            subs.push(cx.subscribe(&state, move |_, _, event: &ColorPickerEvent, cx| {
                let ColorPickerEvent::Change(Some(color)) = event else { return };
                let hex = to_hex(*color);
                let set = SLOTS[ix].2;
                Settings::update(cx, |s| {
                    set(&mut s.custom_theme, hex);
                    s.theme = "Custom".into();
                });
            }));
            pickers.push(state);
        }
        Self {
            focus_handle: cx.focus_handle(),
            pickers,
            _subscriptions: subs,
        }
    }

    /// Copy a theme's palette into the custom theme and refresh the pickers.
    fn start_from(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let settings = Settings::get(cx).clone();
        let palette = themes::palette_for(name, &settings).renamed("Custom");
        for (state, (_, get, _)) in self.pickers.iter().zip(SLOTS) {
            let color = to_hsla(get(&palette));
            state.update(cx, |s, cx| s.set_value(color, window, cx));
        }
        Settings::update(cx, |s| {
            s.custom_theme = palette;
            s.theme = "Custom".into();
        });
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn section(title: &'static str, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex().gap_1().child(
        div()
            .pb_2()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.foreground.opacity(0.9))
            .child(title),
    )
}

fn row(label: &'static str, hint: &'static str, control: impl IntoElement, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .py_2p5()
        .gap_6()
        .justify_between()
        .border_b_1()
        .border_color(theme.border)
        .child(
            v_flex()
                .gap_0p5()
                .child(div().text_sm().text_color(theme.foreground).child(label))
                .child(div().text_xs().text_color(theme.muted_foreground).child(hint)),
        )
        .child(control)
}

fn toggle(id: &'static str, checked: bool, apply: fn(&mut Settings, bool)) -> Switch {
    Switch::new(id).checked(checked).on_click(move |value: &bool, _, cx| {
        let value = *value;
        Settings::update(cx, |s| apply(s, value));
    })
}

fn segmented<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: &[(T, &'static str)],
    current: T,
    apply: fn(&mut Settings, T),
) -> impl IntoElement {
    h_flex().gap_1().children(
        options
            .iter()
            .map(move |&(value, label)| {
                Button::new(SharedString::from(format!("{id}-{label}")))
                    .small()
                    .outline()
                    .selected(current == value)
                    .label(label)
                    .on_click(move |_, _, cx| Settings::update(cx, |s| apply(s, value)))
            })
            .collect::<Vec<_>>(),
    )
}

fn stepper(id: &'static str, value: f32, min: f32, max: f32, apply: fn(&mut Settings, f32), cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    h_flex()
        .gap_1()
        .child(
            Button::new(SharedString::from(format!("{id}-dec")))
                .xsmall()
                .outline()
                .icon(IconName::Minus)
                .disabled(value <= min)
                .on_click(move |_, _, cx| Settings::update(cx, |s| apply(s, (value - 1.).max(min)))),
        )
        .child(
            div()
                .w(px(36.))
                .text_center()
                .text_sm()
                .text_color(theme.foreground)
                .child(format!("{}", value as i32)),
        )
        .child(
            Button::new(SharedString::from(format!("{id}-inc")))
                .xsmall()
                .outline()
                .icon(IconName::Plus)
                .disabled(value >= max)
                .on_click(move |_, _, cx| Settings::update(cx, |s| apply(s, (value + 1.).min(max)))),
        )
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let s = Settings::get(cx).clone();

        // Theme gallery, built from the registry so custom and user themes appear too.
        let names = themes::ordered_names(cx);
        let cards = h_flex().flex_wrap().gap_3().children(
            names
                .into_iter()
                .filter_map(|name| {
                    let [bg, side, fg, accent, muted] = themes::preview(&name, cx)?;
                    let selected = s.theme == name;
                    let (bg, side, fg, accent, muted) =
                        (to_hsla(&bg), to_hsla(&side), to_hsla(&fg), to_hsla(&accent), to_hsla(&muted));
                    let line = |w: f32, c: Hsla| div().h(px(4.)).w(px(w)).rounded_full().bg(c);
                    let label = name.clone();
                    Some(
                        v_flex()
                            .id(SharedString::from(format!("theme-{name}")))
                            .w(px(132.))
                            .gap_1p5()
                            .cursor_pointer()
                            .child(
                                h_flex()
                                    .h(px(80.))
                                    .rounded(px(10.))
                                    .overflow_hidden()
                                    .border_2()
                                    .border_color(if selected { theme.primary } else { theme.border })
                                    .bg(bg)
                                    .child(
                                        v_flex()
                                            .w(px(26.))
                                            .h_full()
                                            .bg(side)
                                            .p_1p5()
                                            .gap_1()
                                            .child(div().h(px(3.)).w(px(12.)).rounded_full().bg(fg).opacity(0.35))
                                            .child(div().h(px(3.)).w(px(10.)).rounded_full().bg(accent))
                                            .child(div().h(px(3.)).w(px(12.)).rounded_full().bg(fg).opacity(0.35)),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .p_2()
                                            .gap_1p5()
                                            .child(h_flex().gap_1().child(line(16., accent)).child(line(30., fg.opacity(0.8))))
                                            .child(h_flex().gap_1().pl_2().child(line(20., muted)).child(line(26., fg.opacity(0.6))))
                                            .child(h_flex().gap_1().pl_2().child(line(12., accent.opacity(0.7))).child(line(18., muted)))
                                            .child(line(10., fg.opacity(0.5))),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(if selected { theme.foreground } else { theme.muted_foreground })
                                    .when(selected, |d| d.font_weight(FontWeight::SEMIBOLD))
                                    .child(label),
                            )
                            .on_click(move |_, _, cx| Settings::update(cx, |s| s.theme = name.clone())),
                    )
                })
                .collect::<Vec<_>>(),
        );

        let swatch = |id: String, color: Option<Hsla>, selected: bool, label: &'static str| {
            div()
                .id(SharedString::from(id))
                .size(px(24.))
                .rounded_full()
                .cursor_pointer()
                .border_2()
                .border_color(if selected { theme.foreground } else { transparent_black() })
                .p(px(2.))
                .child(
                    div()
                        .size_full()
                        .rounded_full()
                        .when_some(color, |d, c| d.bg(c))
                        .when(color.is_none(), |d| d.border_1().border_color(theme.muted_foreground)),
                )
                .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(label).build(window, cx))
        };
        let accents = h_flex()
            .gap_1()
            .child(
                swatch("accent-theme".into(), None, s.accent.is_empty(), "Theme default")
                    .on_click(|_, _, cx| Settings::update(cx, |s| s.accent.clear())),
            )
            .children(
                crate::ACCENTS
                    .iter()
                    .map(|&(name, hex)| {
                        swatch(format!("accent-{name}"), Some(to_hsla(hex)), s.accent.eq_ignore_ascii_case(hex), name)
                            .on_click(move |_, _, cx| Settings::update(cx, |s| s.accent = hex.to_string()))
                    })
                    .collect::<Vec<_>>(),
            );

        let fonts = h_flex().gap_1().children(
            FONTS
                .iter()
                .map(|&font| {
                    Button::new(SharedString::from(format!("font-{font}")))
                        .small()
                        .outline()
                        .selected(s.editor_font == font)
                        .child(div().font_family(font).text_xs().child(font))
                        .on_click(move |_, _, cx| Settings::update(cx, |s| s.editor_font = font.to_string()))
                })
                .collect::<Vec<_>>(),
        );

        // Custom theme editor: a grid of color pickers.
        let current = s.theme.clone();
        let editor = v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(segmented(
                        "custom-base",
                        &[(true, "Dark"), (false, "Light")],
                        s.custom_theme.dark,
                        |s, v| {
                            s.custom_theme.dark = v;
                            s.theme = "Custom".into();
                        },
                    ))
                    .child(div().flex_1())
                    .when(current != "Custom", |d| {
                        d.child(
                            Button::new("custom-from")
                                .small()
                                .ghost()
                                .icon(IconName::Copy)
                                .label(format!("Start from {current}"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.start_from(&current, window, cx)
                                })),
                        )
                    }),
            )
            .child(
                div().grid().grid_cols(2).gap_x_6().gap_y_1().children(
                    SLOTS
                        .iter()
                        .zip(self.pickers.iter())
                        .map(|((label, get, _), state)| {
                            h_flex()
                                .h(px(34.))
                                .justify_between()
                                .border_b_1()
                                .border_color(theme.border)
                                .child(div().text_sm().child(*label))
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_family(theme.mono_font_family.clone())
                                                .text_color(theme.muted_foreground)
                                                .child(get(&s.custom_theme).to_uppercase()),
                                        )
                                        .child(ColorPicker::new(state).small()),
                                )
                        })
                        .collect::<Vec<_>>(),
                ),
            );

        let strength = match s.glass_strength.as_str() {
            "clear" => "clear",
            "tinted" => "tinted",
            _ => "frosted",
        };

        div()
            .id("settings-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_focus(&self.focus_handle)
            .bg(theme.background)
            .child(
                v_flex()
                    .max_w(px(820.))
                    .mx_auto()
                    .px_10()
                    .py_8()
                    .gap_8()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("Settings"))
                    .child(
                        section("Theme", cx)
                            .child(div().pb_3().child(cards))
                            .child(row("Accent color", "Overrides the theme's accent", accents, cx))
                            .child(row(
                                "Window",
                                "Translucent glass or a solid background",
                                segmented("material", &[(true, "Glass"), (false, "Solid")], s.glass, |s, v| s.glass = v),
                                cx,
                            ))
                            .when(s.glass, |d| {
                                d.child(row(
                                    "Glass strength",
                                    "How much of your desktop shows through",
                                    segmented(
                                        "glass-strength",
                                        &[("clear", "Clear"), ("frosted", "Frosted"), ("tinted", "Tinted")],
                                        strength,
                                        |s, v| s.glass_strength = v.to_string(),
                                    ),
                                    cx,
                                ))
                            }),
                    )
                    .child(section("Custom theme", cx).child(editor))
                    .child(
                        section("Typography", cx)
                            .child(row("Editor font", "Used by the editor and terminal", fonts, cx))
                            .child(row(
                                "Editor font size",
                                "Points",
                                stepper("font-size", s.font_size, 10., 28., |s, v| s.font_size = v, cx),
                                cx,
                            ))
                            .child(row(
                                "Terminal font size",
                                "Points",
                                stepper("term-size", s.terminal_font_size, 10., 24., |s, v| s.terminal_font_size = v, cx),
                                cx,
                            )),
                    )
                    .child(
                        section("Editor", cx)
                            .child(row("Word wrap", "Wrap long lines to the window  ⌥Z", toggle("set-wrap", s.soft_wrap, |s, v| s.soft_wrap = v), cx))
                            .child(row("Rainbow indent guides", "Color each indentation level", toggle("set-rainbow", s.rainbow_indent, |s, v| s.rainbow_indent = v), cx))
                            .child(row("Line numbers", "Show the gutter", toggle("set-lines", s.line_numbers, |s, v| s.line_numbers = v), cx))
                            .child(row(
                                "Default tab size",
                                "New files; existing files keep their own indentation",
                                segmented("tab", &[(2usize, "2"), (4, "4"), (8, "8")], s.tab_size, |s, v| s.tab_size = v),
                                cx,
                            ))
                            .child(row("Format on save", "Prettier and native formatters", toggle("set-fmt", s.format_on_save, |s, v| s.format_on_save = v), cx))
                            .child(row("Auto save", "Save shortly after you stop typing", toggle("set-autosave", s.auto_save, |s, v| s.auto_save = v), cx)),
                    )
                    .child(
                        section("Code Runner", cx)
                            .child(row("Save before running", "Saves the file when you press ⌘R", toggle("set-run-save", s.runner_save_before_run, |s, v| s.runner_save_before_run = v), cx))
                            .child(row("Clear terminal before running", "Start each run with a clean screen", toggle("set-run-clear", s.runner_clear_before_run, |s, v| s.runner_clear_before_run = v), cx)),
                    )
                    .child(
                        section("Assistant", cx).child(row(
                            "Enable AI assistant",
                            "Optional. Claude, ChatGPT or Gemini with your own key  ⌘I",
                            toggle("set-ai", s.ai.enabled, |s, v| s.ai.enabled = v),
                            cx,
                        )),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("open-json")
                                    .small()
                                    .ghost()
                                    .icon(Lucide::Braces)
                                    .label("Edit settings.json")
                                    .on_click(|_, window, cx| {
                                        window.dispatch_action(Box::new(crate::workspace::OpenSettingsJson), cx)
                                    }),
                            )
                            .child(
                                Button::new("open-themes")
                                    .small()
                                    .ghost()
                                    .icon(Lucide::FolderOpen)
                                    .label("Open themes folder")
                                    .tooltip("Drop theme .json files here; they appear in the gallery")
                                    .on_click(|_, _, cx| {
                                        let dir = themes::themes_dir();
                                        let _ = std::fs::create_dir_all(&dir);
                                        let _ = open::that(&dir);
                                        let s = Settings::get(cx).clone();
                                        crate::apply_theme(&s, cx);
                                    }),
                            ),
                    ),
            )
    }
}
