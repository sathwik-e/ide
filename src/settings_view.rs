//! Settings, as a tab. Every control applies immediately; the JSON file stays
//! available for anything not shown here.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    switch::Switch,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::base::{Disableable as _, Selectable as _};

use crate::settings::Settings;

/// Preview swatches for the theme cards: name, bg, panel, fg, accent, keyword, string, function.
const THEME_PREVIEWS: &[(&str, u32, u32, u32, u32, u32, u32, u32)] = &[
    ("Eclipse", 0x0e0f13, 0x0a0b0e, 0xd8d9df, 0xf2b36d, 0xc4a0ff, 0xa8d995, 0x8ab4ff),
    ("Nebula", 0x110f1c, 0x0c0a15, 0xdcd7ee, 0xb48cff, 0xff8bd0, 0xb5e6a3, 0x9bb8ff),
    ("Aurora", 0x0b1214, 0x080e10, 0xd3e4e4, 0x4fd8b8, 0x7dd8ff, 0xd6e88a, 0x7ee8c3),
    ("Midnight", 0x0f1116, 0x0b0d11, 0xd5d8e0, 0x7c9cff, 0xc49bff, 0xa8d995, 0x82aaff),
    ("Supernova", 0x140f0e, 0x0f0b0a, 0xeadcd6, 0xff7a59, 0xff9f7a, 0xc3e08f, 0xffd28a),
    ("Lunar", 0xfbfbfc, 0xf4f5f7, 0x1d2130, 0x5b6cf0, 0x8b3fd9, 0x2f8a3a, 0x2f5bea),
];

const FONTS: &[&str] = &["JetBrains Mono", "SF Mono", "Menlo", "Monaco"];

pub struct SettingsView {
    focus_handle: FocusHandle,
    _observe: Subscription,
}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            _observe: cx.observe_global::<Settings>(|_, cx| cx.notify()),
        }
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
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.muted_foreground)
            .child(title.to_uppercase()),
    )
}

fn row(label: &'static str, hint: &'static str, control: impl IntoElement, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .py_2p5()
        .gap_6()
        .justify_between()
        .border_b_1()
        .border_color(theme.border.opacity(0.6))
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

        let theme_cards = h_flex().flex_wrap().gap_3().children(THEME_PREVIEWS.iter().map(
            |&(name, bg, panel, fg, accent, kw, st, func)| {
                let selected = s.theme == name;
                let line = |w: f32, c: u32| div().h(px(4.)).w(px(w)).rounded_full().bg(rgb(c));
                v_flex()
                    .id(SharedString::from(format!("theme-{name}")))
                    .w(px(150.))
                    .gap_1p5()
                    .cursor_pointer()
                    .child(
                        h_flex()
                            .h(px(88.))
                            .rounded(theme.radius_lg)
                            .overflow_hidden()
                            .border_2()
                            .border_color(if selected { theme.primary } else { theme.border })
                            .bg(rgb(bg))
                            .child(
                                v_flex()
                                    .w(px(28.))
                                    .h_full()
                                    .bg(rgb(panel))
                                    .p_1p5()
                                    .gap_1()
                                    .child(div().h(px(3.)).w(px(14.)).rounded_full().bg(rgb(fg)).opacity(0.35))
                                    .child(div().h(px(3.)).w(px(12.)).rounded_full().bg(rgb(accent)).opacity(0.8))
                                    .child(div().h(px(3.)).w(px(14.)).rounded_full().bg(rgb(fg)).opacity(0.35)),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .p_2()
                                    .gap_1p5()
                                    .child(h_flex().gap_1().child(line(18., kw)).child(line(36., func)))
                                    .child(h_flex().gap_1().pl_2().child(line(22., fg)).child(line(30., st)))
                                    .child(h_flex().gap_1().pl_2().child(line(14., kw)).child(line(20., accent)))
                                    .child(h_flex().gap_1().child(line(10., fg))),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .text_xs()
                            .text_color(if selected { theme.foreground } else { theme.muted_foreground })
                            .when(selected, |d| d.font_weight(FontWeight::SEMIBOLD))
                            .child(div().size(px(8.)).rounded_full().bg(rgb(accent)))
                            .child(name),
                    )
                    .on_click(move |_, _, cx| Settings::update(cx, |s| s.theme = name.to_string()))
            },
        ));

        let swatch = |id: String, color: Option<Hsla>, selected: bool, label: &'static str| {
            div()
                .id(SharedString::from(id))
                .size(px(26.))
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
                        .when(color.is_none(), |d| {
                            d.border_1().border_color(theme.muted_foreground).flex().items_center().justify_center()
                                .child(div().text_size(px(9.)).text_color(theme.muted_foreground).child("A"))
                        }),
                )
                .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(label).build(window, cx))
        };
        let accents = h_flex()
            .gap_1p5()
            .child(
                swatch("accent-theme".into(), None, s.accent.is_empty(), "Theme default")
                    .on_click(|_, _, cx| Settings::update(cx, |s| s.accent.clear())),
            )
            .children(crate::ACCENTS.iter().map(|&(name, hex)| {
                let color: Hsla = rgb(u32::from_str_radix(&hex[1..], 16).unwrap_or(0)).into();
                swatch(format!("accent-{name}"), Some(color), s.accent.eq_ignore_ascii_case(hex), name)
                    .on_click(move |_, _, cx| Settings::update(cx, |s| s.accent = hex.to_string()))
            }));

        let fonts = h_flex().gap_1().children(FONTS.iter().map(|&font| {
            Button::new(SharedString::from(format!("font-{font}")))
                .small()
                .outline()
                .selected(s.editor_font == font)
                .child(div().font_family(font).text_xs().child(font))
                .on_click(move |_, _, cx| Settings::update(cx, |s| s.editor_font = font.to_string()))
        }));

        let tab_sizes = h_flex().gap_1().children([2usize, 4, 8].into_iter().map(|n| {
            Button::new(SharedString::from(format!("tab-{n}")))
                .small()
                .outline()
                .selected(s.tab_size == n)
                .label(n.to_string())
                .on_click(move |_, _, cx| Settings::update(cx, |s| s.tab_size = n))
        }));

        div()
            .id("settings-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_focus(&self.focus_handle)
            .bg(theme.background)
            .child(
                v_flex()
                    .max_w(px(760.))
                    .mx_auto()
                    .px_10()
                    .py_10()
                    .gap_10()
                    .child(
                        h_flex()
                            .gap_4()
                            .child(img("brand/eclipse.svg").size(px(56.)))
                            .child(
                                v_flex()
                                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("Settings"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child("Changes apply instantly."),
                                    ),
                            ),
                    )
                    .child(
                        section("Appearance", cx)
                            .child(div().pb_2().child(theme_cards))
                            .child(row("Accent color", "Highlights, cursor, selection and buttons", accents, cx))
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
                            .child(row("Word wrap", "Wrap long lines to the window (⌥Z)", toggle("set-wrap", s.soft_wrap, |s, v| s.soft_wrap = v), cx))
                            .child(row("Rainbow indent guides", "Color each indentation level", toggle("set-rainbow", s.rainbow_indent, |s, v| s.rainbow_indent = v), cx))
                            .child(row("Line numbers", "Show the gutter", toggle("set-lines", s.line_numbers, |s, v| s.line_numbers = v), cx))
                            .child(row("Default tab size", "New files; existing files keep their own indentation", tab_sizes, cx))
                            .child(row("Format on save", "Prettier and native formatters", toggle("set-fmt", s.format_on_save, |s, v| s.format_on_save = v), cx))
                            .child(row("Auto save", "Save shortly after you stop typing", toggle("set-autosave", s.auto_save, |s, v| s.auto_save = v), cx)),
                    )
                    .child(
                        section("Code Runner", cx)
                            .child(row("Save before running", "Saves the file when you press ⌘R", toggle("set-run-save", s.runner_save_before_run, |s, v| s.runner_save_before_run = v), cx))
                            .child(row("Clear terminal before running", "Start each run with a clean screen", toggle("set-run-clear", s.runner_clear_before_run, |s, v| s.runner_clear_before_run = v), cx)),
                    )
                    .child(
                        section("AI assistant", cx).child(row(
                            "Enable assistant",
                            "Optional. Claude, ChatGPT or Gemini with your own key (⌘I)",
                            toggle("set-ai", s.ai.enabled, |s, v| s.ai.enabled = v),
                            cx,
                        )),
                    )
                    .child(
                        h_flex()
                            .gap_2()
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
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("Runner commands, Live Server port and AI keys live there."),
                            ),
                    ),
            )
    }
}
