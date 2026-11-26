//! Find in files. Walks the folder in the background (respecting .gitignore)
//! and lists matching lines; click a line to jump to it.

use std::path::PathBuf;

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};

use crate::icons;

pub enum SearchEvent {
    Open(PathBuf, u32),
}

impl EventEmitter<SearchEvent> for SearchPanel {}

#[derive(Clone)]
struct FileHits {
    path: PathBuf,
    rel: String,
    lines: Vec<(u32, String)>,
}

pub struct SearchPanel {
    root: Option<PathBuf>,
    query: Entity<InputState>,
    results: Vec<FileHits>,
    total: usize,
    searching: bool,
    _task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

const MAX_HITS: usize = 2000;

impl SearchPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search in files"));
        let sub = cx.subscribe_in(&query, window, |this, _, event, _, cx| match event {
            InputEvent::Change | InputEvent::PressEnter { .. } => this.run(cx),
            _ => {}
        });
        Self {
            root: None,
            query,
            results: Vec::new(),
            total: 0,
            searching: false,
            _task: None,
            _subscriptions: vec![sub],
        }
    }

    pub fn set_root(&mut self, root: Option<PathBuf>, cx: &mut Context<Self>) {
        self.root = root;
        self.results.clear();
        self.total = 0;
        cx.notify();
    }

    pub fn input(&self) -> Entity<InputState> {
        self.query.clone()
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let needle = self.query.read(cx).value().to_string();
        let Some(root) = self.root.clone() else { return };
        if needle.len() < 2 {
            self.results.clear();
            self.total = 0;
            self._task = None;
            self.searching = false;
            cx.notify();
            return;
        }
        self.searching = true;
        cx.notify();
        let work = cx.background_spawn(async move {
            // Small debounce so typing doesn't start a walk per keystroke.
            smol::Timer::after(std::time::Duration::from_millis(120)).await;
            let smart_case = needle.chars().any(|c| c.is_uppercase());
            let lower = needle.to_lowercase();
            let mut out = Vec::new();
            let mut total = 0;
            for entry in ignore::WalkBuilder::new(&root).hidden(false).build().flatten() {
                if total >= MAX_HITS {
                    break;
                }
                let path = entry.path();
                if !entry.file_type().is_some_and(|t| t.is_file())
                    || path.components().any(|c| c.as_os_str() == ".git")
                {
                    continue;
                }
                let Ok(bytes) = std::fs::read(path) else { continue };
                if bytes.len() > 4 * 1024 * 1024 || bytes.iter().take(8000).any(|b| *b == 0) {
                    continue;
                }
                let text = String::from_utf8_lossy(&bytes);
                let mut lines = Vec::new();
                for (i, line) in text.lines().enumerate() {
                    let hit = if smart_case {
                        line.contains(&needle)
                    } else {
                        line.to_lowercase().contains(&lower)
                    };
                    if hit {
                        let trimmed: String = line.trim().chars().take(160).collect();
                        lines.push((i as u32 + 1, trimmed));
                        total += 1;
                        if total >= MAX_HITS {
                            break;
                        }
                    }
                }
                if !lines.is_empty() {
                    let rel = path
                        .strip_prefix(&root)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .to_string();
                    out.push(FileHits {
                        path: path.to_path_buf(),
                        rel,
                        lines,
                    });
                }
            }
            (out, total)
        });
        self._task = Some(cx.spawn(async move |this, cx| {
            let (results, total) = work.await;
            let _ = this.update(cx, |this, cx| {
                this.results = results;
                this.total = total;
                this.searching = false;
                cx.notify();
            });
        }));
    }
}

impl Render for SearchPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let summary = if self.searching {
            "Searching…".to_string()
        } else if self.total > 0 {
            format!(
                "{}{} results in {} files",
                self.total,
                if self.total >= MAX_HITS { "+" } else { "" },
                self.results.len()
            )
        } else {
            String::new()
        };
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .p_2()
                    .gap_1()
                    .child(Input::new(&self.query).small().cleanable(true))
                    .when(!summary.is_empty(), |d| {
                        d.child(
                            div()
                                .px_1()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(summary),
                        )
                    }),
            )
            .child(
                div()
                    .id("search-results")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pb_4()
                    .children(self.results.iter().enumerate().map(|(fix, file)| {
                        let name = file
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let dir = std::path::Path::new(&file.rel)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        v_flex()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .px_3()
                                    .h(px(24.))
                                    .text_sm()
                                    .child(img(icons::file_icon(&file.path)).size(px(14.)))
                                    .child(div().flex_shrink_0().child(name))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(dir),
                                    ),
                            )
                            .children(file.lines.iter().enumerate().map(|(lix, (line, text))| {
                                let path = file.path.clone();
                                let line = *line;
                                div()
                                    .id(("hit", fix * 100_000 + lix))
                                    .flex()
                                    .gap_2()
                                    .pl(px(32.))
                                    .pr_2()
                                    .h(px(22.))
                                    .items_center()
                                    .text_xs()
                                    .cursor_pointer()
                                    .hover(|d| d.bg(theme.list_hover))
                                    .child(
                                        div()
                                            .w(px(28.))
                                            .flex_shrink_0()
                                            .text_color(theme.muted_foreground)
                                            .child(line.to_string()),
                                    )
                                    .child(div().truncate().child(text.clone()))
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.emit(SearchEvent::Open(path.clone(), line))
                                    }))
                            }))
                    })),
            )
    }
}
