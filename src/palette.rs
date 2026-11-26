//! One overlay for everything you'd otherwise click through menus for:
//! type to fuzzy-find files, `>` for commands, `:` to go to a line.

use std::path::PathBuf;

use gpui_kit::component::{
    ActiveTheme as _,
    input::{self, Input, InputEvent, InputState},
    kbd::Kbd,
    v_flex, h_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use nucleo_matcher::{
    Config, Matcher,
    pattern::{CaseMatching, Normalization, Pattern},
};

use crate::icons;

pub struct CommandEntry {
    pub label: SharedString,
    pub keys: Option<&'static str>,
    pub action: Box<dyn Action>,
}

pub enum PaletteEvent {
    OpenFile(PathBuf),
    Run(Box<dyn Action>),
    GoToLine(u32, u32),
    Dismiss,
}

impl EventEmitter<PaletteEvent> for Palette {}

#[derive(Clone)]
enum Item {
    File { rel: String },
    Command { ix: usize },
    Line { line: u32, col: u32 },
}

pub struct Palette {
    input: Entity<InputState>,
    root: Option<PathBuf>,
    files: Vec<String>,
    commands: Vec<CommandEntry>,
    items: Vec<Item>,
    selected: usize,
    scroll: UniformListScrollHandle,
    matcher: Matcher,
    _subscriptions: Vec<Subscription>,
}

impl Palette {
    pub fn new(
        initial: &str,
        root: Option<PathBuf>,
        files: Vec<String>,
        commands: Vec<CommandEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = if root.is_some() {
            "Search files by name  ·  > commands  ·  : go to line"
        } else {
            "> run a command  ·  : go to line"
        };
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(initial.to_string())
        });
        let sub = cx.subscribe_in(&input, window, |this, _, event, _, cx| match event {
            InputEvent::Change => {
                this.refresh(cx);
            }
            InputEvent::PressEnter { .. } => this.confirm(cx),
            _ => {}
        });
        let mut this = Self {
            input,
            root,
            files,
            commands,
            items: Vec::new(),
            selected: 0,
            scroll: UniformListScrollHandle::new(),
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
            _subscriptions: vec![sub],
        };
        this.refresh(cx);
        this
    }

    pub fn input(&self) -> Entity<InputState> {
        self.input.clone()
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().to_string();
        self.selected = 0;
        if let Some(rest) = query.strip_prefix(':') {
            let mut parts = rest.split(':').map(|s| s.trim().parse::<u32>().ok());
            let line = parts.next().flatten();
            let col = parts.next().flatten().unwrap_or(1);
            self.items = line
                .map(|line| vec![Item::Line { line, col }])
                .unwrap_or_default();
        } else if let Some(rest) = query.strip_prefix('>') {
            let pattern = Pattern::parse(rest.trim(), CaseMatching::Ignore, Normalization::Smart);
            let labels: Vec<(usize, String)> = self
                .commands
                .iter()
                .enumerate()
                .map(|(ix, c)| (ix, c.label.to_string()))
                .collect();
            if rest.trim().is_empty() {
                self.items = labels.into_iter().map(|(ix, _)| Item::Command { ix }).collect();
            } else {
                let mut matched = pattern.match_list(
                    labels.iter().map(|(ix, l)| Labeled(*ix, l.clone())),
                    &mut self.matcher,
                );
                matched.sort_by(|a, b| b.1.cmp(&a.1));
                self.items = matched
                    .into_iter()
                    .map(|(l, _)| Item::Command { ix: l.0 })
                    .collect();
            }
        } else {
            let q = query.trim();
            if q.is_empty() {
                self.items = self
                    .files
                    .iter()
                    .take(200)
                    .map(|rel| Item::File { rel: rel.clone() })
                    .collect();
            } else {
                let pattern = Pattern::parse(q, CaseMatching::Smart, Normalization::Smart);
                let mut matched = pattern.match_list(self.files.iter(), &mut self.matcher);
                matched.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.len().cmp(&b.0.len())));
                self.items = matched
                    .into_iter()
                    .take(200)
                    .map(|(rel, _)| Item::File { rel: rel.clone() })
                    .collect();
            }
        }
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.items.get(self.selected).cloned() else {
            return;
        };
        match item {
            Item::File { rel } => {
                if let Some(root) = &self.root {
                    cx.emit(PaletteEvent::OpenFile(root.join(rel)));
                }
            }
            Item::Command { ix } => {
                let action = self.commands[ix].action.boxed_clone();
                cx.emit(PaletteEvent::Run(action));
            }
            Item::Line { line, col } => cx.emit(PaletteEvent::GoToLine(line, col)),
        }
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.items.is_empty() {
            return;
        }
        let len = self.items.len() as isize;
        self.selected = ((self.selected as isize + delta).rem_euclid(len)) as usize;
        self.scroll
            .scroll_to_item(self.selected, ScrollStrategy::Center);
        cx.notify();
    }
}

struct Labeled(usize, String);
impl AsRef<str> for Labeled {
    fn as_ref(&self) -> &str {
        &self.1
    }
}

impl Render for Palette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let view = cx.entity();
        let count = self.items.len();
        let empty_hint = if self.input.read(cx).value().starts_with(':') {
            "Type a line number, e.g. :42 or :42:8"
        } else {
            "No matches"
        };

        v_flex()
            .id("palette")
            .key_context("Palette")
            .w(px(620.))
            .max_h(px(440.))
            .rounded(theme.radius_lg)
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .shadow_2xl()
            .overflow_hidden()
            .capture_action(cx.listener(|this, _: &input::MoveUp, _, cx| {
                this.move_selection(-1, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|this, _: &input::MoveDown, _, cx| {
                this.move_selection(1, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|_, _: &input::Escape, _, cx| {
                cx.emit(PaletteEvent::Dismiss);
                cx.stop_propagation();
            }))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(PaletteEvent::Dismiss)))
            .child(
                div().p_2().border_b_1().border_color(theme.border).child(
                    Input::new(&self.input)
                        .appearance(false)
                        .bordered(false),
                ),
            )
            .when(count == 0, |d| {
                d.child(
                    div()
                        .px_4()
                        .py_3()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(empty_hint),
                )
            })
            .when(count > 0, |d| {
                d.child(
                    uniform_list("palette-items", count, move |range, _, cx| {
                        let theme = cx.theme().clone();
                        let this = view.read(cx);
                        range
                            .map(|ix| {
                                let selected = ix == this.selected;
                                let row = h_flex()
                                    .id(("palette-row", ix))
                                    .mx_1()
                                    .px_3()
                                    .h(px(30.))
                                    .gap_2()
                                    .rounded(theme.radius)
                                    .text_sm()
                                    .cursor_pointer()
                                    .when(selected, |d| d.bg(theme.list_active))
                                    .when(!selected, |d| d.hover(|d| d.bg(theme.list_hover)));
                                let row = match &this.items[ix] {
                                    Item::File { rel } => {
                                        let path = std::path::Path::new(rel);
                                        let name = path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| rel.clone());
                                        let dir = path
                                            .parent()
                                            .map(|p| p.to_string_lossy().to_string())
                                            .unwrap_or_default();
                                        row.child(img(icons::file_icon(path)).size(px(16.)).flex_shrink_0())
                                            .child(div().flex_shrink_0().child(name))
                                            .child(
                                                div()
                                                    .truncate()
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground)
                                                    .child(dir),
                                            )
                                    }
                                    Item::Command { ix: cix } => {
                                        let entry = &this.commands[*cix];
                                        row.justify_between()
                                            .child(entry.label.clone())
                                            .children(entry.keys.map(|k| {
                                                Kbd::new(Keystroke::parse(k).unwrap())
                                            }))
                                    }
                                    Item::Line { line, col } => row.child(format!(
                                        "Go to line {line}, column {col}"
                                    )),
                                };
                                let view = view.clone();
                                row.on_click(move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.selected = ix;
                                        this.confirm(cx);
                                    })
                                })
                            })
                            .collect()
                    })
                    .track_scroll(&self.scroll)
                    .py_1()
                    .h(px((count.min(12) as f32) * 30. + 8.)),
                )
            })
    }
}
