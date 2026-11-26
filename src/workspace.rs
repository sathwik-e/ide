//! The window: title bar, explorer/search sidebar, tabbed editors, terminal
//! panel, a right-hand tool panel (browser, HTTP client, optional AI) and a
//! status bar. Every command is reachable from the keyboard and the palette.

use std::path::{Path, PathBuf};

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _, Theme, ThemeMode, ThemeRegistry, TitleBar,
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Editor, EditorState, InputEvent, Position, TabSize},
    notification::Notification,
    resizable::{h_resizable, resizable_panel, v_resizable},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::base::Selectable as _;

use crate::{
    ai::{AiPanel, EditorContext},
    browser::BrowserPanel,
    explorer::{Explorer, ExplorerEvent},
    formatter,
    http_client::HttpPanel,
    icons,
    live_server::LiveServer,
    palette::{CommandEntry, Palette, PaletteEvent},
    runner,
    search::{SearchEvent, SearchPanel},
    settings::{Session, Settings},
    settings_view::SettingsView,
    terminal::{TerminalEvent, TerminalView},
};

actions!(
    ide,
    [
        OpenFolder,
        OpenFile,
        NewFile,
        Save,
        SaveAll,
        CloseTab,
        QuickOpen,
        CommandPalette,
        GoToLine,
        ToggleSidebar,
        ShowExplorer,
        FindInFiles,
        ToggleTerminal,
        NewTerminal,
        ToggleBrowser,
        ToggleHttp,
        ToggleAi,
        RunFile,
        FormatDocument,
        ToggleLiveServer,
        OpenSettings,
        OpenSettingsJson,
        NextTab,
        PrevTab,
        ToggleTheme,
        ToggleSoftWrap,
        ToggleRainbowIndent,
        SetupAi,
        CloseFolder,
        Quit,
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-o", OpenFolder, None),
        KeyBinding::new("cmd-shift-o", OpenFile, None),
        KeyBinding::new("cmd-n", NewFile, None),
        KeyBinding::new("cmd-s", Save, None),
        KeyBinding::new("cmd-alt-s", SaveAll, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-p", QuickOpen, None),
        KeyBinding::new("cmd-shift-p", CommandPalette, None),
        KeyBinding::new("ctrl-g", GoToLine, None),
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-shift-e", ShowExplorer, None),
        KeyBinding::new("cmd-shift-f", FindInFiles, None),
        KeyBinding::new("cmd-j", ToggleTerminal, None),
        KeyBinding::new("ctrl-`", ToggleTerminal, None),
        KeyBinding::new("ctrl-shift-`", NewTerminal, None),
        KeyBinding::new("cmd-shift-b", ToggleBrowser, None),
        KeyBinding::new("cmd-alt-h", ToggleHttp, None),
        KeyBinding::new("cmd-i", ToggleAi, None),
        KeyBinding::new("ctrl-alt-n", RunFile, None),
        KeyBinding::new("cmd-r", RunFile, None),
        KeyBinding::new("shift-alt-f", FormatDocument, None),
        KeyBinding::new("alt-l", ToggleLiveServer, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PrevTab, None),
        KeyBinding::new("cmd-alt-right", NextTab, None),
        KeyBinding::new("cmd-alt-left", PrevTab, None),
        KeyBinding::new("alt-z", ToggleSoftWrap, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-enter", crate::http_client::SendRequest, Some("HttpPanel")),
    ]);
}

pub fn set_menus(cx: &mut App) {
    cx.set_menus(vec![
        Menu {
            name: "Orion".into(),
            items: vec![
                MenuItem::action("Settings…", OpenSettings),
                MenuItem::action("Toggle Light/Dark", ToggleTheme),
                MenuItem::action("Edit settings.json", OpenSettingsJson),
                MenuItem::separator(),
                MenuItem::action("Quit Orion", Quit),
            ],
            disabled: false,
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New File", NewFile),
                MenuItem::action("Open File…", OpenFile),
                MenuItem::action("Open Folder…", OpenFolder),
                MenuItem::separator(),
                MenuItem::action("Save", Save),
                MenuItem::action("Save All", SaveAll),
                MenuItem::separator(),
                MenuItem::action("Close Tab", CloseTab),
                MenuItem::action("Close Folder", CloseFolder),
            ],
            disabled: false,
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::os_action("Undo", gpui_kit::component::input::Undo, OsAction::Undo),
                MenuItem::os_action("Redo", gpui_kit::component::input::Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", gpui_kit::component::input::Cut, OsAction::Cut),
                MenuItem::os_action("Copy", gpui_kit::component::input::Copy, OsAction::Copy),
                MenuItem::os_action("Paste", gpui_kit::component::input::Paste, OsAction::Paste),
                MenuItem::os_action(
                    "Select All",
                    gpui_kit::component::input::SelectAll,
                    OsAction::SelectAll,
                ),
                MenuItem::separator(),
                MenuItem::action("Format Document", FormatDocument),
            ],
            disabled: false,
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Command Palette", CommandPalette),
                MenuItem::action("Quick Open", QuickOpen),
                MenuItem::separator(),
                MenuItem::action("Explorer", ShowExplorer),
                MenuItem::action("Find in Files", FindInFiles),
                MenuItem::action("Toggle Sidebar", ToggleSidebar),
                MenuItem::action("Toggle Terminal", ToggleTerminal),
                MenuItem::action("Browser", ToggleBrowser),
                MenuItem::action("HTTP Client", ToggleHttp),
                MenuItem::action("AI Assistant", ToggleAi),
                MenuItem::separator(),
                MenuItem::action("Toggle Word Wrap", ToggleSoftWrap),
                MenuItem::action("Toggle Rainbow Indent", ToggleRainbowIndent),
            ],
            disabled: false,
        },
        Menu {
            name: "Run".into(),
            items: vec![
                MenuItem::action("Run File", RunFile),
                MenuItem::action("Live Server", ToggleLiveServer),
                MenuItem::action("New Terminal", NewTerminal),
            ],
            disabled: false,
        },
    ]);
}

enum BufferKind {
    Text(Entity<EditorState>),
    Settings(Entity<SettingsView>),
    Image,
    Binary,
}

struct Buffer {
    id: u64,
    path: Option<PathBuf>,
    kind: BufferKind,
    language: String,
    dirty: bool,
    tab: TabSize,
    _subscription: Option<Subscription>,
}

impl Buffer {
    fn title(&self) -> String {
        if matches!(self.kind, BufferKind::Settings(_)) {
            return "Settings".into();
        }
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".into())
    }

    fn editor(&self) -> Option<&Entity<EditorState>> {
        match &self.kind {
            BufferKind::Text(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SidebarMode {
    Files,
    Search,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    Browser,
    Http,
    Ai,
}

pub struct Workspace {
    focus_handle: FocusHandle,
    explorer: Entity<Explorer>,
    search: Entity<SearchPanel>,
    sidebar_mode: SidebarMode,
    sidebar_visible: bool,
    buffers: Vec<Buffer>,
    active: Option<usize>,
    next_id: u64,
    terminals: Vec<Entity<TerminalView>>,
    active_terminal: usize,
    terminal_visible: bool,
    runner_terminal: Option<EntityId>,
    tool: Option<Tool>,
    browser: Option<Entity<BrowserPanel>>,
    http: Option<Entity<HttpPanel>>,
    ai: Option<Entity<AiPanel>>,
    palette: Option<Entity<Palette>>,
    files_index: Vec<String>,
    live: Option<LiveServer>,
    branch: Option<String>,
    session: Session,
    _index_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

fn detect_indent(text: &str, default: usize) -> TabSize {
    let mut tabs = 0;
    let mut spaces: [usize; 9] = [0; 9];
    for line in text.lines().take(2000) {
        if line.starts_with('\t') {
            tabs += 1;
        } else {
            let n = line.chars().take_while(|c| *c == ' ').count();
            if n > 0 && n <= 8 && line.len() > n {
                spaces[n] += 1;
            }
        }
    }
    let space_lines: usize = spaces.iter().sum();
    if tabs > space_lines && tabs > 0 {
        return TabSize {
            tab_size: default,
            hard_tabs: true,
        };
    }
    let size = if spaces[2] > 0 && spaces[2] >= spaces[4] / 2 && spaces[2] >= spaces[3] {
        2
    } else if spaces[4] > 0 {
        4
    } else if space_lines == 0 {
        default
    } else {
        default
    };
    TabSize {
        tab_size: size,
        hard_tabs: false,
    }
}

fn walk_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in ignore::WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
        .flatten()
    {
        if entry.file_type().is_some_and(|t| t.is_file()) {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                out.push(rel.to_string_lossy().to_string());
            }
            if out.len() >= 100_000 {
                break;
            }
        }
    }
    out.sort_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)));
    out
}

fn read_branch(root: &Path) -> Option<String> {
    let head = std::fs::read_to_string(root.join(".git/HEAD")).ok()?;
    let head = head.trim();
    Some(match head.strip_prefix("ref: refs/heads/") {
        Some(branch) => branch.to_string(),
        None => head.chars().take(7).collect(),
    })
}

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "svg", "tiff", "avif"];

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let explorer = cx.new(Explorer::new);
        let search = cx.new(|cx| SearchPanel::new(window, cx));
        let mut subs = vec![
            cx.subscribe_in(&explorer, window, |this, _, event, window, cx| match event {
                ExplorerEvent::Open(path) => this.open_path(path.clone(), window, cx),
                ExplorerEvent::OpenTerminalAt(dir) => {
                    this.new_terminal(Some(dir.clone()), window, cx);
                }
                ExplorerEvent::FilesChanged => {
                    this.branch = this.root(cx).as_deref().and_then(read_branch);
                    this.reindex(cx);
                }
                ExplorerEvent::Deleted(path) => {
                    let gone: Vec<usize> = this
                        .buffers
                        .iter()
                        .enumerate()
                        .filter(|(_, b)| b.path.as_ref().is_some_and(|p| p.starts_with(path)))
                        .map(|(i, _)| i)
                        .collect();
                    for ix in gone.into_iter().rev() {
                        this.remove_buffer(ix, window, cx);
                    }
                }
                ExplorerEvent::Renamed(from, to) => {
                    for b in this.buffers.iter_mut() {
                        if let Some(p) = &b.path {
                            if let Ok(rest) = p.strip_prefix(from) {
                                b.path = Some(if rest.as_os_str().is_empty() {
                                    to.clone()
                                } else {
                                    to.join(rest)
                                });
                            }
                        }
                    }
                    cx.notify();
                }
            }),
            cx.subscribe_in(&search, window, |this, _, event, window, cx| match event {
                SearchEvent::Open(path, line) => {
                    this.open_path(path.clone(), window, cx);
                    this.go_to(*line, 1, window, cx);
                }
            }),
        ];
        subs.push(cx.observe_global_in::<Settings>(window, |this, window, cx| this.apply_settings(window, cx)));

        let session = Session::load();
        let mut this = Self {
            focus_handle: cx.focus_handle(),
            explorer,
            search,
            sidebar_mode: SidebarMode::Files,
            sidebar_visible: true,
            buffers: Vec::new(),
            active: None,
            next_id: 1,
            terminals: Vec::new(),
            active_terminal: 0,
            terminal_visible: false,
            runner_terminal: None,
            tool: None,
            browser: None,
            http: None,
            ai: None,
            palette: None,
            files_index: Vec::new(),
            live: None,
            branch: None,
            session,
            _index_task: None,
            _subscriptions: subs,
        };

        let arg = std::env::args().nth(1).map(PathBuf::from);
        match arg {
            Some(p) if p.is_dir() => this.set_root(Some(p), window, cx),
            Some(p) if p.is_file() => this.open_path(p, window, cx),
            _ => {
                if let Some(last) = this.session.last_folder.clone().filter(|p| p.is_dir()) {
                    this.set_root(Some(last), window, cx);
                }
            }
        }
        this
    }

    fn root(&self, cx: &App) -> Option<PathBuf> {
        self.explorer.read(cx).root().cloned()
    }

    fn set_root(&mut self, root: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let root = root.map(|p| p.canonicalize().unwrap_or(p));
        self.explorer
            .update(cx, |e, cx| e.set_root(root.clone(), cx));
        self.search.update(cx, |s, cx| s.set_root(root.clone(), cx));
        self.live = None;
        if let Some(r) = &root {
            self.session.push_recent(r.clone());
            window.set_window_title(&r.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
        } else {
            self.session.last_folder = None;
            self.session.save();
            window.set_window_title(crate::APP_NAME);
        }
        self.files_index.clear();
        self.branch = root.as_deref().and_then(read_branch);
        self.reindex(cx);
        cx.notify();
    }

    fn reindex(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root(cx) else { return };
        let task = cx.background_spawn(async move { walk_files(&root) });
        self._index_task = Some(cx.spawn(async move |this, cx| {
            let files = task.await;
            let _ = this.update(cx, |this, _| this.files_index = files);
        }));
    }

    fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = Settings::get(cx).clone();
        for b in &self.buffers {
            if let Some(editor) = b.editor() {
                editor.update(cx, |e, cx| {
                    e.set_indent_guides(settings.rainbow_indent, window, cx);
                    e.set_soft_wrap(settings.soft_wrap, window, cx);
                    e.set_line_number(settings.line_numbers, window, cx);
                });
            }
        }
        if settings.ai_ready() && self.ai.is_none() && self.tool == Some(Tool::Ai) {
            cx.notify();
        }
        cx.notify();
    }

    // ---------------------------------------------------------------- files

    fn make_editor(
        &mut self,
        text: String,
        language: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<EditorState> {
        let settings = Settings::get(cx).clone();
        let tab = detect_indent(&text, settings.tab_size);
        let language = language.to_string();
        cx.new(|cx| {
            EditorState::new(window, cx)
                .language(language)
                .line_number(settings.line_numbers)
                .indent_guides(settings.rainbow_indent)
                .tab_size(tab)
                .soft_wrap(settings.soft_wrap)
                .default_value(text)
        })
    }

    fn push_buffer(&mut self, buffer: Buffer, window: &mut Window, cx: &mut Context<Self>) {
        let ix = match self.active {
            Some(a) => a + 1,
            None => self.buffers.len(),
        };
        self.buffers.insert(ix.min(self.buffers.len()), buffer);
        self.activate(ix.min(self.buffers.len() - 1), window, cx);
    }

    fn subscribe_editor(&mut self, id: u64, editor: &Entity<EditorState>, window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        cx.subscribe_in(editor, window, move |this, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(b) = this.buffers.iter_mut().find(|b| b.id == id) {
                    if !b.dirty {
                        b.dirty = true;
                        cx.notify();
                    }
                }
                if Settings::get(cx).auto_save {
                    this.schedule_auto_save(id, window, cx);
                }
            }
        })
    }

    fn schedule_auto_save(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            smol::Timer::after(std::time::Duration::from_millis(900)).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Some(ix) = this.buffers.iter().position(|b| b.id == id && b.dirty && b.path.is_some()) {
                    this.save_buffer(ix, false, window, cx);
                }
            });
        })
        .detach();
    }

    pub fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let path = path.canonicalize().unwrap_or(path);
        if path.is_dir() {
            self.set_root(Some(path), window, cx);
            return;
        }
        if let Some(ix) = self.buffers.iter().position(|b| b.path.as_ref() == Some(&path)) {
            self.activate(ix, window, cx);
            return;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let id = self.next_id;
        self.next_id += 1;
        let language = icons::language_for(&path);
        let buffer = if IMAGE_EXTS.contains(&ext.as_str()) {
            Buffer {
                id,
                path: Some(path),
                kind: BufferKind::Image,
                language,
                dirty: false,
                tab: TabSize::default(),
                _subscription: None,
            }
        } else {
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(err) => {
                    window.push_notification(
                        Notification::error(format!("Can't open {}: {err}", path.display())),
                        cx,
                    );
                    return;
                }
            };
            let binary = bytes.iter().take(8000).any(|b| *b == 0);
            if binary {
                Buffer {
                    id,
                    path: Some(path),
                    kind: BufferKind::Binary,
                    language,
                    dirty: false,
                    tab: TabSize::default(),
                    _subscription: None,
                }
            } else {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let tab = detect_indent(&text, Settings::get(cx).tab_size);
                let editor = self.make_editor(text, &language, window, cx);
                let sub = self.subscribe_editor(id, &editor, window, cx);
                Buffer {
                    id,
                    path: Some(path),
                    kind: BufferKind::Text(editor),
                    language,
                    dirty: false,
                    tab,
                    _subscription: Some(sub),
                }
            }
        };
        self.push_buffer(buffer, window, cx);
    }

    fn new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        let editor = self.make_editor(String::new(), "text", window, cx);
        let sub = self.subscribe_editor(id, &editor, window, cx);
        self.push_buffer(
            Buffer {
                id,
                path: None,
                kind: BufferKind::Text(editor),
                language: "text".into(),
                dirty: false,
                tab: detect_indent("", Settings::get(cx).tab_size),
                _subscription: Some(sub),
            },
            window,
            cx,
        );
    }

    fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.buffers.len() {
            return;
        }
        self.active = Some(ix);
        let path = self.buffers[ix].path.clone();
        self.explorer.update(cx, |e, cx| e.set_active_file(path, cx));
        if let Some(editor) = self.buffers[ix].editor().cloned() {
            window.defer(cx, move |window, cx| {
                editor.update(cx, |e, cx| e.focus(window, cx));
            });
        }
        cx.notify();
    }

    fn remove_buffer(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.buffers.len() {
            return;
        }
        self.buffers.remove(ix);
        if self.buffers.is_empty() {
            self.active = None;
            self.explorer.update(cx, |e, cx| e.set_active_file(None, cx));
            self.focus_handle.focus(window, cx);
        } else {
            let next = match self.active {
                Some(a) if a > ix => a - 1,
                Some(a) if a == ix => ix.min(self.buffers.len() - 1),
                Some(a) => a,
                None => 0,
            };
            self.activate(next, window, cx);
        }
        cx.notify();
    }

    fn close_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(buffer) = self.buffers.get(ix) else { return };
        if !buffer.dirty {
            self.remove_buffer(ix, window, cx);
            return;
        }
        let id = buffer.id;
        let name = buffer.title();
        let ws = cx.entity();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let ws = ws.clone();
            let ws2 = ws.clone();
            dialog
                .title(format!("Save changes to “{name}”?"))
                .description("Your changes will be lost if you don't save them.")
                .confirm()
                .ok_text("Save")
                .cancel_text("Don't Save")
                .on_ok(move |_, window, cx| {
                    ws.update(cx, |this, cx| {
                        if let Some(ix) = this.buffers.iter().position(|b| b.id == id) {
                            if this.save_buffer(ix, false, window, cx) {
                                if let Some(ix) = this.buffers.iter().position(|b| b.id == id) {
                                    this.remove_buffer(ix, window, cx);
                                }
                            }
                        }
                    });
                    true
                })
                .on_cancel(move |_, window, cx| {
                    ws2.update(cx, |this, cx| {
                        if let Some(ix) = this.buffers.iter().position(|b| b.id == id) {
                            this.remove_buffer(ix, window, cx);
                        }
                    });
                    true
                })
        });
    }

    /// Save buffer `ix`. Returns false if it needed a path and the user was
    /// prompted (the save then completes asynchronously).
    fn save_buffer(&mut self, ix: usize, format: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(buffer) = self.buffers.get(ix) else { return false };
        let Some(editor) = buffer.editor().cloned() else { return true };
        let id = buffer.id;
        let Some(path) = buffer.path.clone() else {
            let dir = self.root(cx).unwrap_or_else(|| dirs::home_dir().unwrap_or_default());
            let rx = cx.prompt_for_new_path(&dir, Some("untitled.txt"));
            cx.spawn_in(window, async move |this, cx| {
                if let Ok(Ok(Some(path))) = rx.await {
                    let _ = this.update_in(cx, |this, window, cx| {
                        if let Some(ix) = this.buffers.iter().position(|b| b.id == id) {
                            let language = icons::language_for(&path);
                            if let Some(editor) = this.buffers[ix].editor() {
                                let lang = language.clone();
                                editor.update(cx, |e, cx| e.set_highlighter(lang, cx));
                            }
                            this.buffers[ix].path = Some(path.clone());
                            this.buffers[ix].language = language;
                            this.save_buffer(ix, false, window, cx);
                            this.explorer.update(cx, |e, cx| e.set_active_file(Some(path), cx));
                        }
                    });
                }
            })
            .detach();
            return false;
        };

        let text = editor.read(cx).value().to_string();
        let format_on_save = Settings::get(cx).format_on_save;
        if (format || format_on_save) && formatter::can_format(&path) {
            let path2 = path.clone();
            let task = cx.background_spawn(async move { formatter::format(&path2, &text).map(|t| (t, text)) });
            cx.spawn_in(window, async move |this, cx| {
                let result = task.await;
                let _ = this.update_in(cx, |this, window, cx| {
                    let final_text = match result {
                        Ok((formatted, original)) => {
                            if formatted != original {
                                let pos = editor.read(cx).cursor_position();
                                editor.update(cx, |e, cx| {
                                    e.set_value(formatted.clone(), window, cx);
                                    e.set_cursor_position(pos, window, cx);
                                });
                            }
                            formatted
                        }
                        Err(err) => {
                            window.push_notification(
                                Notification::warning(err).title("Formatter"),
                                cx,
                            );
                            editor.read(cx).value().to_string()
                        }
                    };
                    this.write_file(id, &path, &final_text, window, cx);
                });
            })
            .detach();
            return true;
        }
        self.write_file(id, &path, &text, window, cx);
        true
    }

    fn write_file(&mut self, id: u64, path: &Path, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        match std::fs::write(path, text) {
            Ok(()) => {
                if let Some(b) = self.buffers.iter_mut().find(|b| b.id == id) {
                    b.dirty = false;
                }
                if path == Settings::path() {
                    let settings = Settings::load();
                    crate::apply_theme(&settings, cx);
                    cx.set_global(settings);
                }
                cx.notify();
            }
            Err(err) => window.push_notification(
                Notification::error(format!("Couldn't save {}: {err}", path.display())),
                cx,
            ),
        }
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self
            .buffers
            .iter()
            .position(|b| matches!(b.kind, BufferKind::Settings(_)))
        {
            self.activate(ix, window, cx);
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let view = cx.new(SettingsView::new);
        self.push_buffer(
            Buffer {
                id,
                path: None,
                kind: BufferKind::Settings(view),
                language: "settings".into(),
                dirty: false,
                tab: TabSize::default(),
                _subscription: None,
            },
            window,
            cx,
        );
    }

    fn active_editor(&self) -> Option<Entity<EditorState>> {
        self.active
            .and_then(|ix| self.buffers.get(ix))
            .and_then(|b| b.editor().cloned())
    }

    fn active_path(&self) -> Option<PathBuf> {
        self.active
            .and_then(|ix| self.buffers.get(ix))
            .and_then(|b| b.path.clone())
    }

    fn go_to(&mut self, line: u32, col: u32, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = self.active_editor() {
            window.defer(cx, move |window, cx| {
                editor.update(cx, |e, cx| {
                    e.set_cursor_position(
                        Position::new(line.saturating_sub(1), col.saturating_sub(1)),
                        window,
                        cx,
                    );
                    e.focus(window, cx);
                });
            });
        }
    }

    pub fn editor_context(&self, cx: &App) -> EditorContext {
        let Some(ix) = self.active else { return EditorContext::default() };
        let b = &self.buffers[ix];
        let Some(editor) = b.editor() else { return EditorContext::default() };
        let state = editor.read(cx);
        let content: String = state.value().to_string();
        let content = if content.len() > 200_000 {
            content.chars().take(200_000).collect()
        } else {
            content
        };
        EditorContext {
            file_name: Some(b.title()),
            language: Some(b.language.clone()),
            content: Some(content),
            selection: Some(state.selected_text().to_string()).filter(|s| !s.is_empty()),
        }
    }

    // ------------------------------------------------------------- terminal

    fn new_terminal(&mut self, cwd: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Entity<TerminalView> {
        let cwd = cwd.or_else(|| self.root(cx));
        let term = cx.new(|cx| TerminalView::new(cwd, window, cx));
        let sub = cx.subscribe_in(&term, window, |this, term, event, window, cx| match event {
            TerminalEvent::Exited => {
                if let Some(ix) = this.terminals.iter().position(|t| t == term) {
                    this.close_terminal(ix, window, cx);
                }
            }
            TerminalEvent::TitleChanged => cx.notify(),
        });
        self._subscriptions.push(sub);
        self.terminals.push(term.clone());
        self.active_terminal = self.terminals.len() - 1;
        self.terminal_visible = true;
        let focus = term.focus_handle(cx);
        window.defer(cx, move |window, cx| focus.focus(window, cx));
        cx.notify();
        term
    }

    fn close_terminal(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.terminals.len() {
            return;
        }
        let removed = self.terminals.remove(ix);
        if Some(removed.entity_id()) == self.runner_terminal {
            self.runner_terminal = None;
        }
        if self.terminals.is_empty() {
            self.terminal_visible = false;
            self.active_terminal = 0;
            if let Some(editor) = self.active_editor() {
                editor.update(cx, |e, cx| e.focus(window, cx));
            }
        } else {
            self.active_terminal = self.active_terminal.min(self.terminals.len() - 1);
        }
        cx.notify();
    }

    fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.terminals.is_empty() {
            self.new_terminal(None, window, cx);
            return;
        }
        let term = self.terminals[self.active_terminal].clone();
        let focused = term.focus_handle(cx).is_focused(window);
        if self.terminal_visible && !focused {
            term.focus_handle(cx).focus(window, cx);
        } else {
            self.terminal_visible = !self.terminal_visible;
            if self.terminal_visible {
                term.focus_handle(cx).focus(window, cx);
            } else if let Some(editor) = self.active_editor() {
                editor.update(cx, |e, cx| e.focus(window, cx));
            }
        }
        cx.notify();
    }

    fn run_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active else {
            window.push_notification("Open a file to run it", cx);
            return;
        };
        let Some(path) = self.buffers[ix].path.clone() else {
            self.save_buffer(ix, false, window, cx);
            return;
        };
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if self.buffers[ix].dirty && Settings::get(cx).runner_save_before_run {
            self.save_buffer(ix, false, window, cx);
        }
        if ext == "html" || ext == "htm" {
            self.start_live(Some(path), window, cx);
            return;
        }
        let settings = Settings::get(cx).clone();
        let Some(command) = runner::command_for(&path, &settings.runner_executor_map) else {
            window.push_notification(
                Notification::warning(format!(
                    "No runner for .{ext} files. Add one under \"runner_executor_map\" in settings."
                )),
                cx,
            );
            return;
        };
        let cwd = self
            .root(cx)
            .filter(|r| path.starts_with(r))
            .or_else(|| path.parent().map(Path::to_path_buf));
        let existing = self
            .runner_terminal
            .and_then(|id| self.terminals.iter().position(|t| t.entity_id() == id));
        let term = match existing {
            Some(tix) => {
                self.active_terminal = tix;
                self.terminal_visible = true;
                self.terminals[tix].clone()
            }
            None => {
                let t = self.new_terminal(cwd.clone(), window, cx);
                self.runner_terminal = Some(t.entity_id());
                t
            }
        };
        let cd = cwd
            .map(|d| format!("cd '{}' && ", d.to_string_lossy().replace('\'', "'\\''")))
            .unwrap_or_default();
        let line = format!("{cd}{command}");
        term.update(cx, |t, _| t.run(&line, settings.runner_clear_before_run));
        // Keep typing focus in the terminal so programs can read input.
        term.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    // ---------------------------------------------------------------- tools

    fn show_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        if self.tool == Some(tool) {
            self.tool = None;
        } else {
            self.tool = Some(tool);
            match tool {
                Tool::Browser => {
                    if self.browser.is_none() {
                        self.browser = Some(cx.new(|cx| BrowserPanel::new(window, cx)));
                    }
                }
                Tool::Http => {
                    if self.http.is_none() {
                        self.http = Some(cx.new(|cx| HttpPanel::new(window, cx)));
                    }
                }
                Tool::Ai => {
                    if self.ai.is_none() {
                        let ws = cx.weak_entity();
                        let ai = cx.new(|cx| {
                            let mut panel = AiPanel::new(window, cx);
                            panel.set_context_source(move |cx| {
                                ws.upgrade()
                                    .map(|w| w.read(cx).editor_context(cx))
                                    .unwrap_or_default()
                            });
                            panel
                        });
                        self.ai = Some(ai);
                    }
                    if let Some(ai) = &self.ai {
                        let ai = ai.clone();
                        window.defer(cx, move |window, cx| { let p = ai.read(cx).prompt(); p.update(cx, |s, cx| s.focus(window, cx)); });
                    }
                }
            }
        }
        self.sync_browser(cx);
        cx.notify();
    }

    fn sync_browser(&mut self, cx: &mut Context<Self>) {
        let visible = self.tool == Some(Tool::Browser) && self.palette.is_none();
        if let Some(b) = &self.browser {
            b.update(cx, |b, cx| b.set_visible(visible, cx));
        }
    }

    fn start_live(&mut self, file: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        if self.live.is_none() {
            let root = self
                .root(cx)
                .or_else(|| file.as_ref().and_then(|f| f.parent().map(Path::to_path_buf)));
            let Some(root) = root else {
                window.push_notification("Open a folder to start Live Server", cx);
                return;
            };
            match LiveServer::start(root, Settings::get(cx).live_server_port) {
                Ok(server) => self.live = Some(server),
                Err(err) => {
                    window.push_notification(Notification::error(format!("Live Server: {err}")), cx);
                    return;
                }
            }
        }
        let Some(live) = &self.live else { return };
        let target = file
            .or_else(|| self.active_path().filter(|p| {
                p.extension().is_some_and(|e| e == "html" || e == "htm")
            }))
            .map(|f| live.url_for(&f))
            .unwrap_or_else(|| live.url());
        if self.tool != Some(Tool::Browser) {
            self.show_tool(Tool::Browser, window, cx);
        }
        if let Some(b) = &self.browser {
            b.update(cx, |b, cx| b.navigate(&target, window, cx));
        }
        cx.notify();
    }

    fn toggle_live(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.live.take().is_some() {
            window.push_notification("Live Server stopped", cx);
            cx.notify();
        } else {
            self.start_live(None, window, cx);
        }
    }

    // -------------------------------------------------------------- palette

    fn commands(&self, cx: &App) -> Vec<CommandEntry> {
        let ai_enabled = Settings::get(cx).ai.enabled;
        let mut c = vec![
            ("Run File", Some("cmd-r"), Box::new(RunFile) as Box<dyn Action>),
            ("Format Document", Some("shift-alt-f"), Box::new(FormatDocument)),
            ("Live Server: Start / Stop", Some("alt-l"), Box::new(ToggleLiveServer)),
            ("Browser", Some("cmd-shift-b"), Box::new(ToggleBrowser)),
            ("HTTP Client", Some("cmd-alt-h"), Box::new(ToggleHttp)),
            ("Terminal: Toggle", Some("cmd-j"), Box::new(ToggleTerminal)),
            ("Terminal: New", Some("ctrl-shift-`"), Box::new(NewTerminal)),
            ("Find in Files", Some("cmd-shift-f"), Box::new(FindInFiles)),
            ("Go to Line", Some("ctrl-g"), Box::new(GoToLine)),
            ("Open Folder…", Some("cmd-o"), Box::new(OpenFolder)),
            ("Open File…", Some("cmd-shift-o"), Box::new(OpenFile)),
            ("New File", Some("cmd-n"), Box::new(NewFile)),
            ("Save", Some("cmd-s"), Box::new(Save)),
            ("Save All", Some("cmd-alt-s"), Box::new(SaveAll)),
            ("Close Tab", Some("cmd-w"), Box::new(CloseTab)),
            ("Close Folder", None, Box::new(CloseFolder)),
            ("Toggle Sidebar", Some("cmd-b"), Box::new(ToggleSidebar)),
            ("Toggle Word Wrap", Some("alt-z"), Box::new(ToggleSoftWrap)),
            ("Toggle Rainbow Indent", None, Box::new(ToggleRainbowIndent)),
            ("Theme: Toggle Light / Dark", None, Box::new(ToggleTheme)),
            ("Settings", Some("cmd-,"), Box::new(OpenSettings)),
            ("Settings: Edit settings.json", None, Box::new(OpenSettingsJson)),
        ];
        if ai_enabled {
            c.push(("AI Assistant", Some("cmd-i"), Box::new(ToggleAi)));
        }
        c.push(("AI: Set Up Assistant (Claude, ChatGPT, Gemini)", None, Box::new(SetupAi)));
        c.into_iter()
            .map(|(label, keys, action)| CommandEntry {
                label: label.into(),
                keys,
                action,
            })
            .collect()
    }

    fn open_palette(&mut self, initial: &str, window: &mut Window, cx: &mut Context<Self>) {
        let root = self.root(cx);
        let files = self.files_index.clone();
        let commands = self.commands(cx);
        let initial = initial.to_string();
        let palette = cx.new(|cx| Palette::new(&initial, root, files, commands, window, cx));
        let sub = cx.subscribe_in(&palette, window, |this, _, event, window, cx| {
            match event {
                PaletteEvent::OpenFile(path) => {
                    let path = path.clone();
                    this.close_palette(window, cx);
                    this.open_path(path, window, cx);
                }
                PaletteEvent::Run(action) => {
                    let action = action.boxed_clone();
                    this.close_palette(window, cx);
                    window.dispatch_action(action, cx);
                }
                PaletteEvent::GoToLine(line, col) => {
                    let (line, col) = (*line, *col);
                    this.close_palette(window, cx);
                    this.go_to(line, col, window, cx);
                }
                PaletteEvent::Dismiss => this.close_palette(window, cx),
            }
        });
        self._subscriptions.push(sub);
        let p = palette.clone();
        window.defer(cx, move |window, cx| { let i = p.read(cx).input(); i.update(cx, |s, cx| s.focus(window, cx)); });
        self.palette = Some(palette);
        self.sync_browser(cx);
        cx.notify();
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.take().is_some() {
            if let Some(editor) = self.active_editor() {
                editor.update(cx, |e, cx| e.focus(window, cx));
            } else {
                self.focus_handle.focus(window, cx);
            }
            self.sync_browser(cx);
            cx.notify();
        }
    }

    fn open_folder_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if let Some(path) = paths.into_iter().next() {
                    let _ = this.update_in(cx, |this, window, cx| this.set_root(Some(path), window, cx));
                }
            }
        })
        .detach();
    }

    fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                let _ = this.update_in(cx, |this, window, cx| {
                    for p in paths {
                        this.open_path(p, window, cx);
                    }
                });
            }
        })
        .detach();
    }

    // --------------------------------------------------------------- render

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (chrome_fill, chrome_line) = chrome(&theme);
        let root_name = self
            .root(cx)
            .and_then(|r| r.file_name().map(|n| n.to_string_lossy().to_string()));
        let file_name = self.active.and_then(|ix| self.buffers.get(ix)).map(|b| b.title());
        let settings = Settings::get(cx).clone();
        let runnable = self.active_path().is_some_and(|p| {
            runner::can_run(&p, &settings.runner_executor_map)
        });
        let ai_on = settings.ai.enabled;

        let tool = |id: &'static str, icon: Lucide, active: bool, tip: &'static str| {
            Button::new(id).ghost().small().icon(icon).selected(active).tooltip(tip)
        };
        let capsule = || {
            h_flex()
                .h(px(30.))
                .px(px(3.))
                .gap(px(2.))
                .rounded(px(9.))
                .bg(chrome_fill)
                .border_1()
                .border_color(chrome_line)
        };

        TitleBar::new()
            .h(px(44.))
            .bg(transparent_black())
            .border_color(transparent_black())
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        capsule().child(
                            tool("tb-sidebar", Lucide::PanelLeft, self.sidebar_visible, "Sidebar  ⌘B")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.sidebar_visible = !this.sidebar_visible;
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .child(
                h_flex().flex_1().justify_center().child(
                    h_flex()
                        .id("tb-title")
                        .gap_1p5()
                        .px_2p5()
                        .h(px(26.))
                        .max_w(px(520.))
                        .rounded(px(7.))
                        .text_sm()
                        .cursor_pointer()
                        .hover(|d| d.bg(chrome_fill))
                        .child(
                            div()
                                .flex_shrink_0()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground.opacity(0.92))
                                .child(root_name.clone().unwrap_or_else(|| crate::APP_NAME.to_string())),
                        )
                        .when_some(file_name, |d, name| {
                            d.child(div().text_color(theme.muted_foreground.opacity(0.7)).child("—"))
                                .child(div().truncate().text_color(theme.muted_foreground).child(name))
                        })
                        .tooltip(|window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new("Go to File  ⌘P").build(window, cx)
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.root(cx).is_some() {
                                this.open_palette("", window, cx)
                            } else {
                                this.open_folder_dialog(window, cx)
                            }
                        })),
                ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .pr_3()
                    .when(runnable || self.live.is_some(), |d| {
                        d.child(
                            capsule()
                                .when(runnable, |d| {
                                    d.child(
                                        Button::new("tb-run")
                                            .ghost()
                                            .small()
                                            .icon(Lucide::Play)
                                            .tooltip("Run File  ⌘R")
                                            .on_click(cx.listener(|this, _, window, cx| this.run_file(window, cx))),
                                    )
                                })
                                .child(
                                    tool("tb-live", Lucide::RadioTower, self.live.is_some(), "Live Server  ⌥L")
                                        .on_click(cx.listener(|this, _, window, cx| this.toggle_live(window, cx))),
                                ),
                        )
                    })
                    .child(
                        capsule()
                            .child(
                                tool("tb-term", Lucide::SquareTerminal, self.terminal_visible, "Terminal  ⌘J")
                                    .on_click(cx.listener(|this, _, window, cx| this.toggle_terminal(window, cx))),
                            )
                            .child(
                                tool("tb-browser", Lucide::Globe, self.tool == Some(Tool::Browser), "Browser  ⇧⌘B")
                                    .on_click(cx.listener(|this, _, window, cx| this.show_tool(Tool::Browser, window, cx))),
                            )
                            .child(
                                tool("tb-http", Lucide::Send, self.tool == Some(Tool::Http), "HTTP Client  ⌥⌘H")
                                    .on_click(cx.listener(|this, _, window, cx| this.show_tool(Tool::Http, window, cx))),
                            )
                            .when(ai_on, |d| {
                                d.child(
                                    tool("tb-ai", Lucide::Sparkles, self.tool == Some(Tool::Ai), "Assistant  ⌘I")
                                        .on_click(cx.listener(|this, _, window, cx| this.show_tool(Tool::Ai, window, cx))),
                                )
                            }),
                    ),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (chrome_fill, chrome_line) = chrome(&theme);
        let mode = self.sidebar_mode;
        let title = match mode {
            SidebarMode::Files => self
                .root(cx)
                .and_then(|r| r.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| "Explorer".into()),
            SidebarMode::Search => "Search".into(),
        };
        let has_root = self.root(cx).is_some();
        let icon_btn = |id: &'static str, icon: Lucide, tip: &'static str| {
            Button::new(id).ghost().xsmall().icon(icon).tooltip(tip)
        };
        v_flex()
            .size_full()
            .rounded(px(12.))
            .bg(chrome_fill)
            .border_1()
            .border_color(chrome_line)
            .overflow_hidden()
            .child(
                h_flex()
                    .group("sidebar-head")
                    .h(px(38.))
                    .pl_3()
                    .pr_1p5()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground.opacity(0.85))
                            .child(title),
                    )
                    .when(has_root && mode == SidebarMode::Files, |d| {
                        d.child(
                            h_flex()
                                .invisible()
                                .group_hover("sidebar-head", |s| s.visible())
                                .child(icon_btn("sb-new-file", Lucide::FilePlus, "New File").on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.explorer.update(cx, |e, cx| e.prompt_new(false, None, window, cx))
                                    }),
                                ))
                                .child(icon_btn("sb-new-folder", Lucide::FolderPlus, "New Folder").on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.explorer.update(cx, |e, cx| e.prompt_new(true, None, window, cx))
                                    }),
                                ))
                                .child(icon_btn("sb-collapse", Lucide::ChevronsDownUp, "Collapse All").on_click(
                                    cx.listener(|this, _, _, cx| this.explorer.update(cx, |e, cx| e.collapse_all(cx))),
                                )),
                        )
                    })
                    .when(has_root, |d| {
                        d.child(
                            Button::new("sb-search")
                                .ghost()
                                .xsmall()
                                .icon(if mode == SidebarMode::Search { Lucide::Files } else { Lucide::Search })
                                .tooltip(if mode == SidebarMode::Search { "Explorer  ⇧⌘E" } else { "Search  ⇧⌘F" })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if this.sidebar_mode == SidebarMode::Search {
                                        this.sidebar_mode = SidebarMode::Files;
                                    } else {
                                        this.sidebar_mode = SidebarMode::Search;
                                        let q = this.search.read(cx).input();
                                        q.update(cx, |s, cx| s.focus(window, cx));
                                    }
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .when(mode == SidebarMode::Files, |d| d.child(self.explorer.clone()))
                    .when(mode == SidebarMode::Search, |d| d.child(self.search.clone())),
            )
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let dark = theme.is_dark();
        let active_fill = if dark { white().opacity(0.075) } else { black().opacity(0.055) };
        let hover_fill = if dark { white().opacity(0.04) } else { black().opacity(0.03) };
        h_flex()
            .id("tabs")
            .h(px(42.))
            .flex_shrink_0()
            .px_1p5()
            .gap_1()
            .border_b_1()
            .border_color(theme.border)
            .overflow_x_scroll()
            .children(self.buffers.iter().enumerate().map(|(ix, b)| {
                let active = self.active == Some(ix);
                let icon = b
                    .path
                    .as_ref()
                    .map(|p| icons::file_icon(p))
                    .unwrap_or_else(|| "icons/files/default_file.svg".to_string());
                let dirty = b.dirty;
                let group = SharedString::from(format!("tab-{}", b.id));
                h_flex()
                    .id(("tab", b.id))
                    .group(group.clone())
                    .h(px(30.))
                    .pl_2p5()
                    .pr_1()
                    .gap_1p5()
                    .flex_shrink_0()
                    .rounded(px(8.))
                    .text_sm()
                    .cursor_pointer()
                    .when(active, |d| d.bg(active_fill).text_color(theme.foreground))
                    .when(!active, |d| {
                        d.text_color(theme.muted_foreground)
                            .hover(|d| d.bg(hover_fill).text_color(theme.foreground))
                    })
                    .map(|d| match &b.kind {
                        BufferKind::Settings(_) => d.child(
                            gpui_kit::component::Icon::new(IconName::Settings)
                                .size(px(14.))
                                .text_color(theme.muted_foreground),
                        ),
                        _ => d.child(img(icon).size(px(15.))),
                    })
                    .child(b.title())
                    .child(
                        div()
                            .id(("tab-close", b.id))
                            .size(px(18.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5.))
                            .hover(|d| d.bg(hover_fill))
                            .when(dirty, |d| {
                                d.child(
                                    div()
                                        .size(px(6.))
                                        .rounded_full()
                                        .bg(theme.foreground.opacity(0.6))
                                        .group_hover(group.clone(), |s| s.invisible()),
                                )
                            })
                            .child(
                                div()
                                    .absolute()
                                    .invisible()
                                    .group_hover(group.clone(), |s| s.visible())
                                    .child(
                                        gpui_kit::component::Icon::new(IconName::Close)
                                            .size(px(11.))
                                            .text_color(theme.muted_foreground),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close_tab(ix, window, cx);
                            })),
                    )
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(move |this, _, window, cx| this.close_tab(ix, window, cx)),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| this.activate(ix, window, cx)))
            }))
    }

    fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let has_root = self.root(cx).is_some();
        let hint = |keys: &'static str, label: &'static str| {
            h_flex()
                .w(px(240.))
                .justify_between()
                .text_sm()
                .child(div().text_color(theme.muted_foreground).child(label))
                .child(div().text_color(theme.muted_foreground.opacity(0.65)).child(keys))
        };
        let recent: Vec<PathBuf> = self
            .session
            .recent
            .iter()
            .filter(|p| p.is_dir())
            .take(6)
            .cloned()
            .collect();
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_6()
            .child(
                v_flex()
                    .gap_2p5()
                    .when(!has_root, |d| d.child(hint("⌘O", "Open Folder")))
                    .child(hint("⌘P", "Go to File"))
                    .child(hint("⇧⌘P", "Commands"))
                    .child(hint("⌘N", "New File"))
                    .child(hint("⌘J", "Terminal")),
            )
            .when(!has_root && !recent.is_empty(), |d| {
                d.child(
                    v_flex()
                        .w(px(240.))
                        .gap_0p5()
                        .pt_2()
                        .border_t_1()
                        .border_color(theme.border)
                        .children(recent.into_iter().enumerate().map(|(i, p)| {
                            let name = p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            h_flex()
                                .id(("recent", i))
                                .px_2()
                                .mx(px(-8.))
                                .h(px(26.))
                                .rounded(px(6.))
                                .text_sm()
                                .text_color(theme.foreground.opacity(0.85))
                                .cursor_pointer()
                                .hover(|d| d.bg(theme.list_hover))
                                .child(div().truncate().child(name))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.set_root(Some(p.clone()), window, cx)
                                }))
                        })),
                )
            })
    }

    fn render_editor_area(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(ix) = self.active else {
            return self.render_welcome(cx).into_any_element();
        };
        let buffer = &self.buffers[ix];
        let font_size = Settings::get(cx).font_size;
        let content: AnyElement = match &buffer.kind {
            BufferKind::Text(editor) => Editor::new(editor)
                .bordered(false)
                .p_0()
                .h_full()
                .font_family(theme.mono_font_family.clone())
                .text_size(px(font_size))
                .into_any_element(),
            BufferKind::Settings(view) => view.clone().into_any_element(),
            BufferKind::Image => {
                let path = buffer.path.clone().unwrap_or_default();
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .p_8()
                    .child(img(path).max_w_full().max_h_full().object_fit(ObjectFit::Contain))
                    .into_any_element()
            }
            BufferKind::Binary => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("Binary file — not shown in the editor.")
                .into_any_element(),
        };
        v_flex()
            .size_full()
            .child(self.render_tabs(cx))
            .child(div().flex_1().min_h_0().child(content))
            .into_any_element()
    }

    fn render_terminal_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let dark = theme.is_dark();
        let active_fill = if dark { white().opacity(0.075) } else { black().opacity(0.055) };
        v_flex()
            .size_full()
            .border_t_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .h(px(34.))
                    .px_1p5()
                    .gap_1()
                    .children(self.terminals.iter().enumerate().map(|(ix, t)| {
                        let active = ix == self.active_terminal;
                        let title = t.read(cx).title.clone();
                        let group = SharedString::from(format!("term-tab-{ix}"));
                        h_flex()
                            .id(("term-tab", ix))
                            .group(group.clone())
                            .h(px(24.))
                            .pl_2()
                            .pr_1()
                            .gap_1()
                            .rounded(px(6.))
                            .text_xs()
                            .cursor_pointer()
                            .when(active, |d| d.bg(active_fill).text_color(theme.foreground))
                            .when(!active, |d| {
                                d.text_color(theme.muted_foreground)
                                    .hover(|d| d.text_color(theme.foreground))
                            })
                            .child(div().max_w(px(160.)).truncate().child(title))
                            .child(
                                div()
                                    .id(("term-close", ix))
                                    .invisible()
                                    .group_hover(group, |s| s.visible())
                                    .rounded(px(4.))
                                    .child(gpui_kit::component::Icon::new(IconName::Close).size(px(10.)))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.close_terminal(ix, window, cx);
                                    })),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.active_terminal = ix;
                                let f = this.terminals[ix].focus_handle(cx);
                                f.focus(window, cx);
                                cx.notify();
                            }))
                    }))
                    .child(
                        Button::new("term-new")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .tooltip("New Terminal  ⌃⇧`")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_terminal(None, window, cx);
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("term-hide")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ChevronDown)
                            .tooltip("Hide  ⌘J")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.terminal_visible = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .children(self.terminals.get(self.active_terminal).cloned()),
            )
    }

    fn render_tool_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (_, chrome_line) = chrome(&theme);
        let content: Option<AnyElement> = match self.tool {
            Some(Tool::Browser) => self.browser.clone().map(|b| b.into_any_element()),
            Some(Tool::Http) => self.http.clone().map(|b| b.into_any_element()),
            Some(Tool::Ai) => self.ai.clone().map(|b| b.into_any_element()),
            None => None,
        };
        div()
            .size_full()
            .rounded(px(12.))
            .overflow_hidden()
            .bg(theme.background)
            .border_1()
            .border_color(chrome_line)
            .shadow_md()
            .children(content)
    }

    /// The status line at the foot of the editor card.
    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let settings = Settings::get(cx).clone();
        let mut right: Vec<AnyElement> = Vec::new();
        let item = |id: &'static str| {
            h_flex()
                .id(id)
                .gap_1()
                .px_1p5()
                .h(px(20.))
                .rounded(px(5.))
                .cursor_pointer()
                .hover(|d| d.bg(theme.list_hover).text_color(theme.foreground))
        };
        if let Some(ix) = self.active {
            let b = &self.buffers[ix];
            if let Some(editor) = b.editor() {
                let pos = editor.read(cx).cursor_position();
                right.push(
                    item("sb-pos")
                        .child(format!("Ln {}, Col {}", pos.line + 1, pos.character + 1))
                        .on_click(cx.listener(|this, _, window, cx| this.open_palette(":", window, cx)))
                        .into_any_element(),
                );
                let tab = b.tab;
                right.push(
                    item("sb-indent")
                        .child(if tab.hard_tabs { "Tabs".to_string() } else { format!("Spaces {}", tab.tab_size) })
                        .into_any_element(),
                );
                right.push(
                    item("sb-wrap")
                        .when(settings.soft_wrap, |d| d.text_color(theme.primary))
                        .child(gpui_kit::component::Icon::new(Lucide::TextWrap).size(px(12.)))
                        .child("Wrap")
                        .tooltip(|window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new("Word Wrap  ⌥Z").build(window, cx)
                        })
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleSoftWrap), cx))
                        .into_any_element(),
                );
                right.push(item("sb-lang").child(icons::language_label(&b.language)).into_any_element());
            }
            if let Some(name) = b.path.as_ref().and_then(|p| formatter::formatter_name(p)) {
                right.push(
                    item("sb-fmt")
                        .when(settings.format_on_save, |d| {
                            d.child(gpui_kit::component::Icon::new(IconName::Check).size(px(11.)))
                        })
                        .child(name)
                        .tooltip(|window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new("Format Document  ⇧⌥F").build(window, cx)
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            if let Some(ix) = this.active {
                                this.save_buffer(ix, true, window, cx);
                            }
                        }))
                        .into_any_element(),
                );
            }
        }

        h_flex()
            .h(px(28.))
            .flex_shrink_0()
            .px_2()
            .justify_between()
            .text_xs()
            .text_color(theme.muted_foreground)
            .border_t_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .gap_0p5()
                    .when_some(self.branch.clone(), |d, branch| {
                        d.child(
                            item("sb-branch")
                                .child(gpui_kit::component::Icon::new(Lucide::GitBranch).size(px(12.)))
                                .child(branch)
                                .tooltip(|window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new("git status").build(window, cx)
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let t = this.new_terminal(None, window, cx);
                                    t.update(cx, |t, _| t.run("git status", false));
                                })),
                        )
                    })
                    .when_some(self.live.as_ref(), |d, live| {
                        d.child(
                            item("sb-live")
                                .text_color(theme.success)
                                .child(div().size(px(6.)).rounded_full().bg(theme.success))
                                .child(format!("Live :{}", live.port))
                                .tooltip(|window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new("Stop Live Server").build(window, cx)
                                })
                                .on_click(cx.listener(|this, _, window, cx| this.toggle_live(window, cx))),
                        )
                    }),
            )
            .child(h_flex().gap_0p5().children(right))
    }
}

/// Glass chrome colors: translucent fill and hairline for floating panels.
fn chrome(theme: &Theme) -> (Hsla, Hsla) {
    if theme.is_dark() {
        (white().opacity(0.045), white().opacity(0.085))
    } else {
        (white().opacity(0.55), black().opacity(0.08))
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (_, chrome_line) = chrome(&theme);
        let glass = Settings::get(cx).glass;
        let tint = if glass { theme.sidebar } else { Hsla { a: 1.0, ..theme.sidebar } };
        let tool_open = self.tool.is_some();
        let sidebar = self.sidebar_visible;

        // The editor card: tabs + editor, terminal, and the status line.
        let card = v_flex()
            .size_full()
            .rounded(px(12.))
            .overflow_hidden()
            .bg(theme.background)
            .border_1()
            .border_color(chrome_line)
            .shadow_md()
            .child(
                div().flex_1().min_h_0().child(
                    v_resizable("center")
                        .child(
                            resizable_panel()
                                .size_range(px(120.)..Pixels::MAX)
                                .child(self.render_editor_area(cx)),
                        )
                        .child(
                            resizable_panel()
                                .size(px(260.))
                                .size_range(px(80.)..px(1200.))
                                .visible(self.terminal_visible && !self.terminals.is_empty())
                                .child(self.render_terminal_panel(cx)),
                        ),
                ),
            )
            .child(self.render_status_bar(cx));

        let body = h_resizable("body")
            .child(
                resizable_panel()
                    .size(px(260.))
                    .size_range(px(180.)..px(560.))
                    .visible(sidebar)
                    .child(div().size_full().pl_2().pr_1().pb_2().child(self.render_sidebar(cx))),
            )
            .child(
                resizable_panel().size_range(px(240.)..Pixels::MAX).child(
                    div()
                        .size_full()
                        .pb_2()
                        .when(sidebar, |d| d.pl_1())
                        .when(!sidebar, |d| d.pl_2())
                        .when(tool_open, |d| d.pr_1())
                        .when(!tool_open, |d| d.pr_2())
                        .child(card),
                ),
            )
            .child(
                resizable_panel()
                    .size(px(480.))
                    .size_range(px(300.)..px(1600.))
                    .visible(tool_open)
                    .child(div().size_full().pl_1().pr_2().pb_2().child(self.render_tool_panel(cx))),
            );

        let palette = self.palette.clone().map(|p| {
            deferred(
                div()
                    .absolute()
                    .top(px(56.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(p),
            )
            .with_priority(10)
        });
        window.set_background_appearance(if glass {
            WindowBackgroundAppearance::Blurred
        } else {
            WindowBackgroundAppearance::Opaque
        });

        v_flex()
            .id("workspace")
            .key_context("Workspace")
            .track_focus(&self.focus_handle)
            .size_full()
            .relative()
            .bg(tint)
            .text_color(theme.foreground)
            .on_action(cx.listener(|this, _: &OpenFolder, window, cx| this.open_folder_dialog(window, cx)))
            .on_action(cx.listener(|this, _: &OpenFile, window, cx| this.open_file_dialog(window, cx)))
            .on_action(cx.listener(|this, _: &CloseFolder, window, cx| {
                this.set_root(None, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewFile, window, cx| this.new_file(window, cx)))
            .on_action(cx.listener(|this, _: &Save, window, cx| {
                if let Some(ix) = this.active {
                    this.save_buffer(ix, false, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SaveAll, window, cx| {
                let dirty: Vec<usize> = this
                    .buffers
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.dirty && b.path.is_some())
                    .map(|(i, _)| i)
                    .collect();
                for ix in dirty {
                    this.save_buffer(ix, false, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                if let Some(ix) = this.active {
                    this.close_tab(ix, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &QuickOpen, window, cx| {
                if this.palette.is_some() {
                    this.close_palette(window, cx)
                } else {
                    this.open_palette("", window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &CommandPalette, window, cx| {
                if this.palette.is_some() {
                    this.close_palette(window, cx)
                } else {
                    this.open_palette(">", window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &GoToLine, window, cx| this.open_palette(":", window, cx)))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.sidebar_visible = !this.sidebar_visible;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowExplorer, _, cx| {
                this.sidebar_visible = true;
                this.sidebar_mode = SidebarMode::Files;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FindInFiles, window, cx| {
                this.sidebar_visible = true;
                this.sidebar_mode = SidebarMode::Search;
                { let q = this.search.read(cx).input(); q.update(cx, |s, cx| s.focus(window, cx)); }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| this.toggle_terminal(window, cx)))
            .on_action(cx.listener(|this, _: &NewTerminal, window, cx| {
                this.new_terminal(None, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleBrowser, window, cx| this.show_tool(Tool::Browser, window, cx)))
            .on_action(cx.listener(|this, _: &ToggleHttp, window, cx| this.show_tool(Tool::Http, window, cx)))
            .on_action(cx.listener(|this, _: &ToggleAi, window, cx| {
                if Settings::get(cx).ai.enabled {
                    this.show_tool(Tool::Ai, window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &SetupAi, window, cx| {
                if !Settings::get(cx).ai.enabled {
                    Settings::update(cx, |s| s.ai.enabled = true);
                }
                if this.tool != Some(Tool::Ai) {
                    this.show_tool(Tool::Ai, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &RunFile, window, cx| this.run_file(window, cx)))
            .on_action(cx.listener(|this, _: &FormatDocument, window, cx| {
                if let Some(ix) = this.active {
                    let can = this.buffers[ix].path.as_ref().is_some_and(|p| formatter::can_format(p));
                    if can {
                        this.save_buffer(ix, true, window, cx);
                    } else {
                        window.push_notification("No formatter for this file type", cx);
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleLiveServer, window, cx| this.toggle_live(window, cx)))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| this.open_settings(window, cx)))
            .on_action(cx.listener(|this, _: &OpenSettingsJson, window, cx| {
                let path = Settings::path();
                if !path.exists() {
                    Settings::get(cx).save();
                }
                this.open_path(path, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextTab, window, cx| {
                if let Some(a) = this.active {
                    let n = this.buffers.len();
                    this.activate((a + 1) % n, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &PrevTab, window, cx| {
                if let Some(a) = this.active {
                    let n = this.buffers.len();
                    this.activate((a + n - 1) % n, window, cx);
                }
            }))
            .on_action(cx.listener(|_, _: &ToggleTheme, _, cx| {
                let dark = cx.theme().is_dark();
                Settings::update(cx, |s| s.theme = if dark { "Lunar".into() } else { "Orion".into() });
            }))
            .on_action(cx.listener(|_, _: &ToggleSoftWrap, _, cx| {
                Settings::update(cx, |s| s.soft_wrap = !s.soft_wrap);
            }))
            .on_action(cx.listener(|_, _: &ToggleRainbowIndent, _, cx| {
                Settings::update(cx, |s| s.rainbow_indent = !s.rainbow_indent);
            }))
            .on_action(cx.listener(|_, _: &Quit, _, cx| cx.quit()))
            .child(self.render_title_bar(cx))
            .child(div().flex_1().min_h_0().child(body))
            .children(palette)
    }
}

#[allow(dead_code)]
fn _assert_theme_api(cx: &mut App) {
    let _ = ThemeRegistry::global(cx);
    Theme::change(ThemeMode::Dark, None, cx);
}

#[cfg(test)]
mod tests {
    use super::detect_indent;

    #[test]
    fn detects_indentation() {
        assert!(detect_indent("fn a() {\n\tx();\n\ty();\n}", 4).hard_tabs);
        assert_eq!(detect_indent("a:\n  b: 1\n  c:\n    d: 2\n", 4).tab_size, 2);
        assert_eq!(detect_indent("def f():\n    return 1\n", 2).tab_size, 4);
        assert_eq!(detect_indent("", 4).tab_size, 4);
    }
}
