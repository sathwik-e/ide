//! File explorer: a virtualized tree of the open folder that follows the
//! disk through a file watcher. Right-click for file operations.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::Duration,
};

use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariant, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    menu::{ContextMenuExt as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use notify::{RecursiveMode, Watcher};

use crate::icons;

#[derive(Clone)]
struct Row {
    path: PathBuf,
    name: SharedString,
    depth: usize,
    is_dir: bool,
}

pub enum ExplorerEvent {
    Open(PathBuf),
    OpenTerminalAt(PathBuf),
    FilesChanged,
    Deleted(PathBuf),
    Renamed(PathBuf, PathBuf),
}

impl EventEmitter<ExplorerEvent> for Explorer {}

pub struct Explorer {
    root: Option<PathBuf>,
    expanded: HashSet<PathBuf>,
    rows: Vec<Row>,
    selected: Option<PathBuf>,
    active_file: Option<PathBuf>,
    context_target: Option<PathBuf>,
    scroll: UniformListScrollHandle,
    focus_handle: FocusHandle,
    _watcher: Option<notify::RecommendedWatcher>,
    _watch_task: Option<Task<()>>,
}

const HIDDEN: &[&str] = &[".git", ".DS_Store", ".hg", ".svn"];

fn read_dir_sorted(dir: &Path) -> Vec<(PathBuf, String, bool)> {
    let mut out: Vec<(PathBuf, String, bool)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if HIDDEN.contains(&name.as_str()) {
                        return None;
                    }
                    let path = e.path();
                    let is_dir = path.is_dir();
                    Some((path, name, is_dir))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    out
}

impl Explorer {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            root: None,
            expanded: HashSet::new(),
            rows: Vec::new(),
            selected: None,
            active_file: None,
            context_target: None,
            scroll: UniformListScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            _watcher: None,
            _watch_task: None,
        }
    }

    pub fn root(&self) -> Option<&PathBuf> {
        self.root.as_ref()
    }

    pub fn set_root(&mut self, root: Option<PathBuf>, cx: &mut Context<Self>) {
        self.root = root.clone();
        self.expanded.clear();
        self.selected = None;
        self._watcher = None;
        self._watch_task = None;
        if let Some(root) = root {
            let (tx, rx) = smol::channel::unbounded::<()>();
            let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res {
                    let noisy = ev.paths.iter().all(|p| {
                        p.components()
                            .any(|c| matches!(c.as_os_str().to_str(), Some(".git" | "target" | "node_modules")))
                    });
                    if !noisy && !ev.kind.is_access() {
                        let _ = tx.try_send(());
                    }
                }
            });
            if let Ok(mut w) = watcher {
                let _ = w.watch(&root, RecursiveMode::Recursive);
                self._watcher = Some(w);
            }
            self._watch_task = Some(cx.spawn(async move |this, cx| {
                while rx.recv().await.is_ok() {
                    // Coalesce bursts (git checkout, builds) into one refresh.
                    smol::Timer::after(Duration::from_millis(150)).await;
                    while rx.try_recv().is_ok() {}
                    if this
                        .update(cx, |this, cx| {
                            this.rebuild();
                            cx.emit(ExplorerEvent::FilesChanged);
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        self.rebuild();
        cx.notify();
    }

    fn rebuild(&mut self) {
        self.rows.clear();
        let Some(root) = self.root.clone() else {
            return;
        };
        self.push_children(&root, 0);
    }

    fn push_children(&mut self, dir: &Path, depth: usize) {
        for (path, name, is_dir) in read_dir_sorted(dir) {
            let expanded = is_dir && self.expanded.contains(&path);
            self.rows.push(Row {
                path: path.clone(),
                name: name.into(),
                depth,
                is_dir,
            });
            if expanded {
                self.push_children(&path, depth + 1);
            }
        }
    }

    pub fn set_active_file(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.active_file == path {
            return;
        }
        self.active_file = path.clone();
        // Reveal the active file: expand its ancestors and scroll to it.
        if let (Some(path), Some(root)) = (path, self.root.clone()) {
            if path.starts_with(&root) {
                let mut changed = false;
                let mut p = path.parent();
                while let Some(dir) = p {
                    if dir == root {
                        break;
                    }
                    changed |= self.expanded.insert(dir.to_path_buf());
                    p = dir.parent();
                }
                if changed {
                    self.rebuild();
                }
                if let Some(ix) = self.rows.iter().position(|r| r.path == path) {
                    self.scroll.scroll_to_item(ix, ScrollStrategy::Center);
                }
                self.selected = Some(path);
            }
        }
        cx.notify();
    }

    fn toggle(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(row) = self.rows.get(ix).cloned() else {
            return;
        };
        self.selected = Some(row.path.clone());
        if row.is_dir {
            if !self.expanded.remove(&row.path) {
                self.expanded.insert(row.path.clone());
            }
            self.rebuild();
        } else {
            cx.emit(ExplorerEvent::Open(row.path));
        }
        cx.notify();
    }

    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.expanded.clear();
        self.rebuild();
        cx.notify();
    }

    /// Folder new items go into: the right-clicked/selected folder, or the
    /// parent of the selected file, or the root.
    fn target_dir(&self, explicit: Option<&PathBuf>) -> Option<PathBuf> {
        let pick = explicit.or(self.selected.as_ref());
        match pick {
            Some(p) if p.is_dir() => Some(p.clone()),
            Some(p) => p.parent().map(Path::to_path_buf),
            None => self.root.clone(),
        }
    }

    pub fn prompt_new(&mut self, folder: bool, explicit: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dir) = self.target_dir(explicit.as_ref()) else {
            return;
        };
        if let Some(d) = explicit.as_ref().filter(|p| p.is_dir()) {
            self.expanded.insert(d.clone());
        } else if dir != self.root.clone().unwrap_or_default() {
            self.expanded.insert(dir.clone());
        }
        let explorer = cx.entity();
        let title = if folder { "New Folder" } else { "New File" };
        prompt_text(title, "", window, cx, move |name, _window, cx| {
            let name = name.trim();
            if name.is_empty() {
                return;
            }
            let path = dir.join(name);
            let result = if folder {
                std::fs::create_dir_all(&path)
            } else {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map(|_| ())
            };
            explorer.update(cx, |this, cx| {
                if result.is_ok() {
                    this.rebuild();
                    this.selected = Some(path.clone());
                    if !folder {
                        cx.emit(ExplorerEvent::Open(path));
                    }
                }
                cx.notify();
            });
        });
    }

    fn prompt_rename(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let explorer = cx.entity();
        let current = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        prompt_text("Rename", &current, window, cx, move |name, _, cx| {
            let name = name.trim();
            if name.is_empty() {
                return;
            }
            let Some(parent) = path.parent() else { return };
            let to = parent.join(name);
            if to != path && std::fs::rename(&path, &to).is_ok() {
                explorer.update(cx, |this, cx| {
                    this.rebuild();
                    this.selected = Some(to.clone());
                    cx.emit(ExplorerEvent::Renamed(path.clone(), to));
                    cx.notify();
                });
            }
        });
    }

    fn confirm_delete(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let explorer = cx.entity();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let path = path.clone();
            let explorer = explorer.clone();
            dialog
                .title(format!("Move “{name}” to Trash?"))
                .description("You can restore it from the Trash.")
                .confirm()
                .ok_text("Move to Trash")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    if trash::delete(&path).is_ok() {
                        explorer.update(cx, |this, cx| {
                            this.rebuild();
                            cx.emit(ExplorerEvent::Deleted(path.clone()));
                            cx.notify();
                        });
                    }
                    true
                })
        });
    }

    fn relative(&self, path: &Path) -> String {
        self.root
            .as_ref()
            .and_then(|r| path.strip_prefix(r).ok())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string())
    }
}

/// A small dialog with one text field; `on_submit` runs on OK or Enter.
pub fn prompt_text(
    title: &'static str,
    initial: &str,
    window: &mut Window,
    cx: &mut App,
    on_submit: impl Fn(&str, &mut Window, &mut App) + 'static,
) {
    let input = cx.new(|cx| InputState::new(window, cx).default_value(initial.to_string()));
    let on_submit = std::rc::Rc::new(on_submit);
    let focus_once = std::rc::Rc::new(std::cell::Cell::new(true));
    window.open_alert_dialog(cx, move |dialog, window, cx| {
        if focus_once.replace(false) {
            input.update(cx, |s, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
        }
        let input2 = input.clone();
        let on_submit = on_submit.clone();
        dialog
            .title(title)
            .child(Input::new(&input))
            .confirm()
            .ok_text(if title == "Rename" { "Rename" } else { "Create" })
            .on_ok(move |_, window, cx| {
                let value = input2.read(cx).value().to_string();
                on_submit(&value, window, cx);
                true
            })
    });
}

impl Focusable for Explorer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Explorer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(root) = self.root.clone() else {
            return v_flex()
                .size_full()
                .p_4()
                .gap_3()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("No folder open"),
                )
                .child(
                    Button::new("open-folder-side")
                        .small()
                        .primary()
                        .label("Open Folder")
                        .on_click(|_, window, cx| {
                            window.dispatch_action(Box::new(crate::workspace::OpenFolder), cx)
                        }),
                )
                .into_any_element();
        };
        let root_name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_uppercase())
            .unwrap_or_default();
        let view = cx.entity();
        let count = self.rows.len();

        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .child(
                h_flex()
                    .group("explorer-header")
                    .h(px(32.))
                    .pl_3()
                    .pr_1()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(root_name),
                    )
                    .child(
                        h_flex()
                            .invisible()
                            .group_hover("explorer-header", |s| s.visible())
                            .child(
                                Button::new("new-file")
                                    .ghost()
                                    .xsmall()
                                    .icon(gpui_kit::assets::IconName::FilePlus)
                                    .tooltip("New File")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.prompt_new(false, None, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("new-folder")
                                    .ghost()
                                    .xsmall()
                                    .icon(gpui_kit::assets::IconName::FolderPlus)
                                    .tooltip("New Folder")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.prompt_new(true, None, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("collapse")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ChevronsUpDown)
                                    .tooltip("Collapse All")
                                    .on_click(cx.listener(|this, _, _, cx| this.collapse_all(cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .capture_any_mouse_down(cx.listener(|this, e: &MouseDownEvent, _, _| {
                        if e.button == MouseButton::Right {
                            this.context_target = None;
                        }
                    }))
                    .context_menu({
                        let view = view.clone();
                        move |menu, _, cx| {
                            let target = view.read(cx).context_target.clone();
                            let view = view.clone();
                            let mut menu = menu;
                            let t = target.clone();
                            let v = view.clone();
                            menu = menu.item(PopupMenuItem::new("New File…").on_click(
                                move |_, window, cx| {
                                    v.update(cx, |this, cx| this.prompt_new(false, t.clone(), window, cx))
                                },
                            ));
                            let t = target.clone();
                            let v = view.clone();
                            menu = menu.item(PopupMenuItem::new("New Folder…").on_click(
                                move |_, window, cx| {
                                    v.update(cx, |this, cx| this.prompt_new(true, t.clone(), window, cx))
                                },
                            ));
                            if let Some(path) = target {
                                menu = menu.separator();
                                let p = path.clone();
                                let v = view.clone();
                                menu = menu.item(PopupMenuItem::new("Rename…").on_click(
                                    move |_, window, cx| {
                                        v.update(cx, |this, cx| this.prompt_rename(p.clone(), window, cx))
                                    },
                                ));
                                let p = path.clone();
                                let v = view.clone();
                                menu = menu.item(PopupMenuItem::new("Move to Trash").on_click(
                                    move |_, window, cx| {
                                        v.update(cx, |this, cx| this.confirm_delete(p.clone(), window, cx))
                                    },
                                ));
                                menu = menu.separator();
                                let p = path.clone();
                                menu = menu.item(PopupMenuItem::new("Copy Path").on_click(
                                    move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            p.to_string_lossy().to_string(),
                                        ))
                                    },
                                ));
                                let p = path.clone();
                                let v = view.clone();
                                menu = menu.item(PopupMenuItem::new("Copy Relative Path").on_click(
                                    move |_, _, cx| {
                                        let rel = v.read(cx).relative(&p);
                                        cx.write_to_clipboard(ClipboardItem::new_string(rel))
                                    },
                                ));
                                let p = path.clone();
                                menu = menu.item(PopupMenuItem::new("Reveal in Finder").on_click(
                                    move |_, _, _| {
                                        let _ = std::process::Command::new("open")
                                            .arg("-R")
                                            .arg(&p)
                                            .spawn();
                                    },
                                ));
                                let p = path.clone();
                                let v = view.clone();
                                menu = menu.item(PopupMenuItem::new("Open in Terminal").on_click(
                                    move |_, _, cx| {
                                        let dir = if p.is_dir() {
                                            p.clone()
                                        } else {
                                            p.parent().map(Path::to_path_buf).unwrap_or_default()
                                        };
                                        v.update(cx, |_, cx| cx.emit(ExplorerEvent::OpenTerminalAt(dir)))
                                    },
                                ));
                            }
                            menu
                        }
                    })
                    .child(
                        uniform_list("explorer-rows", count, move |range, _, cx| {
                            let theme = cx.theme().clone();
                            let this = view.read(cx);
                            range
                                .filter_map(|ix| {
                                    let row = this.rows.get(ix)?.clone();
                                    let expanded = row.is_dir && this.expanded.contains(&row.path);
                                    let selected = this.selected.as_ref() == Some(&row.path);
                                    let active = this.active_file.as_ref() == Some(&row.path);
                                    let icon = if row.is_dir {
                                        icons::folder_icon(&row.name, expanded)
                                    } else {
                                        icons::file_icon(&row.path)
                                    };
                                    let view = view.clone();
                                    let view2 = view.clone();
                                    let path = row.path.clone();
                                    Some(
                                        h_flex()
                                            .id(SharedString::from(row.path.to_string_lossy().to_string()))
                                            .h(px(24.))
                                            .mx_1()
                                            .pl(px(8. + row.depth as f32 * 14.))
                                            .pr_2()
                                            .gap_1p5()
                                            .rounded(theme.radius)
                                            .text_sm()
                                            .cursor_pointer()
                                            .when(active, |d| {
                                                d.bg(theme.list_active).text_color(theme.foreground)
                                            })
                                            .when(!active && selected, |d| d.bg(theme.list_hover))
                                            .when(!active, |d| {
                                                d.text_color(theme.sidebar_foreground)
                                                    .hover(|d| d.bg(theme.list_hover))
                                            })
                                            .child(
                                                div()
                                                    .w(px(12.))
                                                    .flex_shrink_0()
                                                    .when(row.is_dir, |d| {
                                                        d.child(
                                                            gpui_kit::component::Icon::new(if expanded {
                                                                IconName::ChevronDown
                                                            } else {
                                                                IconName::ChevronRight
                                                            })
                                                            .size(px(12.))
                                                            .text_color(theme.muted_foreground),
                                                        )
                                                    }),
                                            )
                                            .child(img(icon).size(px(16.)).flex_shrink_0())
                                            .child(div().truncate().child(row.name.clone()))
                                            .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                                                let path = path.clone();
                                                view2.update(cx, |this, _| {
                                                    this.context_target = Some(path)
                                                });
                                            })
                                            .on_click(move |_, _, cx| {
                                                view.update(cx, |this, cx| this.toggle(ix, cx))
                                            }),
                                    )
                                })
                                .collect()
                        })
                        .track_scroll(&self.scroll)
                        .size_full()
                        .pb_4(),
                    )
            )
            .into_any_element()
    }
}
