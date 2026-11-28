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
    diff::{Diff, DiffFile, DiffState},
    input::{Editor, EditorState, InputEvent, Position, TabSize},
    text::TextView,
    notification::Notification,
    resizable::{h_resizable, resizable_panel, v_resizable},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{collections::{BTreeMap, HashMap}, rc::Rc, sync::Arc, time::SystemTime};
use gpui_kit::base::Selectable as _;

use crate::{
    ai::{AiPanel, EditorContext},
    browser::BrowserPanel,
    editing,
    explorer::{Explorer, ExplorerEvent},
    formatter,
    git::{GitEvent, GitPanel},
    lsp,
    http_client::HttpPanel,
    icons,
    live_server::LiveServer,
    palette::{CommandEntry, Palette, PaletteEvent},
    runner,
    search::{SearchEvent, SearchPanel},
    symbols,
    tasks,
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
        ToggleComment,
        MoveLineUp,
        MoveLineDown,
        DuplicateLineUp,
        DuplicateLineDown,
        DeleteLine,
        SelectNextOccurrence,
        ReopenClosedTab,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        ToggleMarkdownPreview,
        RunTask,
        GoToSymbol,
        ShowShortcuts,
        ToggleZenMode,
        ShowSourceControl,
        ShowProblems,
        RestartLanguageServers,
    ]
);

const EDITOR_CTX: &str = "Input && mode == editor";

/// Identity for formatter notifications, so a new one replaces the last.
struct FormatterNotice;

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-o", OpenFolder, None),
        KeyBinding::new("cmd-shift-o", GoToSymbol, None),
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
        KeyBinding::new("cmd-shift-t", ReopenClosedTab, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-0", ZoomReset, None),
        KeyBinding::new("cmd-shift-v", ToggleMarkdownPreview, None),
        KeyBinding::new("cmd-alt-r", RunTask, None),
        KeyBinding::new("cmd-k cmd-s", ShowShortcuts, None),
        KeyBinding::new("cmd-k z", ToggleZenMode, None),
        KeyBinding::new("ctrl-shift-g", ShowSourceControl, None),
        KeyBinding::new("cmd-shift-m", ShowProblems, None),
        // Editor commands, bound in the code editor's context so they win over
        // the engine's defaults where they overlap.
        KeyBinding::new("cmd-/", ToggleComment, Some(EDITOR_CTX)),
        KeyBinding::new("alt-up", MoveLineUp, Some(EDITOR_CTX)),
        KeyBinding::new("alt-down", MoveLineDown, Some(EDITOR_CTX)),
        KeyBinding::new("shift-alt-up", DuplicateLineUp, Some(EDITOR_CTX)),
        KeyBinding::new("shift-alt-down", DuplicateLineDown, Some(EDITOR_CTX)),
        KeyBinding::new("cmd-shift-k", DeleteLine, Some(EDITOR_CTX)),
        KeyBinding::new("cmd-d", SelectNextOccurrence, Some(EDITOR_CTX)),
        KeyBinding::new("f12", gpui_kit::component::input::GoToDefinition, Some(EDITOR_CTX)),
        KeyBinding::new("cmd-alt-f", gpui_kit::component::input::Replace, Some(EDITOR_CTX)),
        KeyBinding::new("cmd-shift-f", FindInFiles, Some(EDITOR_CTX)),
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
                MenuItem::action("Open…", OpenFolder),
                MenuItem::action("Open File…", OpenFile),
                MenuItem::separator(),
                MenuItem::action("Save", Save),
                MenuItem::action("Save All", SaveAll),
                MenuItem::separator(),
                MenuItem::action("Close Tab", CloseTab),
                MenuItem::action("Reopen Closed Tab", ReopenClosedTab),
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
                MenuItem::separator(),
                MenuItem::action("Toggle Comment", ToggleComment),
                MenuItem::action("Move Line Up", MoveLineUp),
                MenuItem::action("Move Line Down", MoveLineDown),
                MenuItem::action("Duplicate Line", DuplicateLineDown),
                MenuItem::action("Delete Line", DeleteLine),
                MenuItem::action("Select Next Occurrence", SelectNextOccurrence),
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
                MenuItem::action("Source Control", ShowSourceControl),
                MenuItem::action("Problems", ShowProblems),
                MenuItem::action("Markdown Preview", ToggleMarkdownPreview),
                MenuItem::separator(),
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::action("Reset Zoom", ZoomReset),
                MenuItem::action("Zen Mode", ToggleZenMode),
                MenuItem::separator(),
                MenuItem::action("Toggle Word Wrap", ToggleSoftWrap),
                MenuItem::action("Toggle Rainbow Indent", ToggleRainbowIndent),
                MenuItem::action("Keyboard Shortcuts", ShowShortcuts),
            ],
            disabled: false,
        },
        Menu {
            name: "Run".into(),
            items: vec![
                MenuItem::action("Run File", RunFile),
                MenuItem::action("Run Task…", RunTask),
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
    Diff(Entity<DiffState>, String),
    Image,
    /// First bytes of a non-text file (for the hex view) and its full size.
    Binary(Rc<Vec<u8>>, u64),
}

struct Buffer {
    id: u64,
    path: Option<PathBuf>,
    kind: BufferKind,
    language: String,
    dirty: bool,
    tab: TabSize,
    doc: Option<Rc<lsp::Document>>,
    mtime: Option<SystemTime>,
    _subscription: Option<Subscription>,
}

impl Buffer {
    fn title(&self) -> String {
        if matches!(self.kind, BufferKind::Settings(_)) {
            return "Settings".into();
        }
        if let BufferKind::Diff(_, title) = &self.kind {
            return title.clone();
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
    Git,
}

#[derive(Clone, Copy, PartialEq)]
enum BottomTab {
    Terminal,
    Problems,
}

enum LspEntry {
    Starting(Vec<u64>),
    Ready(Arc<lsp::LspClient>),
    Failed,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    Browser,
    Http,
    Ai,
    Preview,
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
    git: Entity<GitPanel>,
    git_changes: usize,
    bottom_tab: BottomTab,
    lsp: HashMap<&'static str, LspEntry>,
    lsp_tx: smol::channel::Sender<lsp::ServerEvent>,
    diagnostics: BTreeMap<PathBuf, Vec<lsp_types::Diagnostic>>,
    closed: Vec<PathBuf>,
    zen: Option<(bool, bool, Option<Tool>)>,
    sync_pending: bool,
    format_error: Option<String>,
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
        .filter_entry(|e| {
            !matches!(
                e.file_name().to_str(),
                Some(".git" | ".DS_Store" | ".localized" | ".Spotlight-V100" | ".Trashes" | ".fseventsd")
            )
        })
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
                ExplorerEvent::FilesChanged(paths) => {
                    this.branch = this.root(cx).as_deref().and_then(read_branch);
                    this.reindex(cx);
                    this.reload_changed(paths.clone(), window, cx);
                    this.git.update(cx, |g, cx| g.refresh(cx));
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
        let git = cx.new(|cx| GitPanel::new(window, cx));
        subs.push(cx.subscribe_in(&git, window, |this, _, event, window, cx| match event {
            GitEvent::OpenDiff { path, staged, untracked } => {
                this.open_diff(path.clone(), *staged, *untracked, window, cx)
            }
            GitEvent::Changed(n) => {
                this.git_changes = *n;
                cx.notify();
            }
        }));

        // Diagnostics from every language server arrive on one channel.
        let (lsp_tx, lsp_rx) = smol::channel::unbounded::<lsp::ServerEvent>();
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = lsp_rx.recv().await {
                let lsp::ServerEvent::Diagnostics { uri, diagnostics } = event;
                let ok = this.update_in(cx, |this, _, cx| this.set_diagnostics(&uri, diagnostics, cx));
                if ok.is_err() {
                    break;
                }
            }
        })
        .detach();

        let session = Session::load();
        let this = Self {
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
            git,
            git_changes: 0,
            bottom_tab: BottomTab::Terminal,
            lsp: HashMap::new(),
            lsp_tx,
            diagnostics: BTreeMap::new(),
            closed: Vec::new(),
            zen: None,
            sync_pending: false,
            format_error: None,
            session,
            _index_task: None,
            _subscriptions: subs,
        };

        // Open the project after the first frame, so the window appears even
        // while macOS asks for folder access.
        cx.defer_in(window, |this, window, cx| {
            let mut requested: Vec<PathBuf> = std::env::args()
                .skip(1)
                .filter(|a| !a.starts_with("-psn_") && !a.starts_with('-'))
                .map(PathBuf::from)
                .collect();
            if let Some(incoming) = cx.try_global::<crate::opener::Incoming>().cloned() {
                requested.extend(incoming.take());
            }
            let opens_folder = requested.iter().any(|p| p.is_dir());
            if !opens_folder {
                if let Some(last) = this.session.last_folder.clone().filter(|p| p.is_dir()) {
                    this.set_root(Some(last), window, cx);
                    this.restore_tabs(window, cx);
                }
            }
            this.open_paths(requested, window, cx);
            // A terminal is always docked and ready, without taking focus.
            this.spawn_terminal(None, false, window, cx);
        });
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
        if let (Some(r), Some(term)) = (root.as_ref(), self.terminals.first().cloned()) {
            let cd = format!("cd '{}'", r.to_string_lossy().replace('\'', "'\\''"));
            term.update(cx, |t, _| t.run(&cd, true));
        }
        self.branch = root.as_deref().and_then(read_branch);
        self.git.update(cx, |g, cx| g.set_root(root.clone(), cx));
        // Language servers are per project.
        self.lsp.clear();
        self.diagnostics.clear();
        for b in self.buffers.iter_mut() {
            b.doc = None;
        }
        let ids: Vec<u64> = self.buffers.iter().map(|b| b.id).collect();
        for id in ids {
            self.attach_lsp(id, window, cx);
        }
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
                this.schedule_lsp_sync(window, cx);
                if this.tool == Some(Tool::Preview) {
                    cx.notify();
                }
            }
        })
    }

    /// Push edited documents to their language servers, debounced.
    fn schedule_lsp_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync_pending {
            return;
        }
        self.sync_pending = true;
        cx.spawn_in(window, async move |this, cx| {
            smol::Timer::after(std::time::Duration::from_millis(250)).await;
            let _ = this.update(cx, |this, cx| {
                this.sync_pending = false;
                for b in &this.buffers {
                    if let (Some(doc), Some(editor)) = (&b.doc, b.editor()) {
                        doc.sync(&editor.read(cx).value());
                    }
                }
            });
        })
        .detach();
    }

    /// Start (or reuse) the language server for a buffer and wire it up.
    fn attach_lsp(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root(cx) else { return };
        let Some(b) = self.buffers.iter().find(|b| b.id == id) else { return };
        let (Some(path), Some(_)) = (b.path.clone(), b.editor()) else { return };
        if b.doc.is_some() || !path.starts_with(&root) {
            return;
        }
        let language = b.language.clone();
        let Some((spec, bin)) = lsp::resolve(&language) else { return };
        match self.lsp.get_mut(spec.name) {
            Some(LspEntry::Ready(client)) => {
                let client = client.clone();
                self.open_document(id, client, window, cx);
            }
            Some(LspEntry::Starting(waiting)) => waiting.push(id),
            Some(LspEntry::Failed) => {}
            None => {
                self.lsp.insert(spec.name, LspEntry::Starting(vec![id]));
                let tx = self.lsp_tx.clone();
                let name = spec.name;
                let task = cx.background_spawn(async move { lsp::LspClient::start(spec, &bin, &root, tx) });
                cx.spawn_in(window, async move |this, cx| {
                    let result = task.await;
                    let _ = this.update_in(cx, |this, window, cx| {
                        let waiting = match this.lsp.remove(name) {
                            Some(LspEntry::Starting(w)) => w,
                            _ => Vec::new(),
                        };
                        match result {
                            Ok(client) => {
                                this.lsp.insert(name, LspEntry::Ready(client.clone()));
                                for id in waiting {
                                    this.open_document(id, client.clone(), window, cx);
                                }
                            }
                            Err(err) => {
                                log::warn!("language server {name} failed: {err}");
                                this.lsp.insert(name, LspEntry::Failed);
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    fn open_document(&mut self, id: u64, client: Arc<lsp::LspClient>, window: &mut Window, cx: &mut Context<Self>) {
        let ws = cx.weak_entity();
        let Some(b) = self.buffers.iter_mut().find(|b| b.id == id) else { return };
        let (Some(path), Some(editor)) = (b.path.clone(), b.editor().cloned()) else { return };
        let text = editor.read(cx).value().to_string();
        let doc = lsp::Document::open(client, &path, &lsp::language_id(&b.language, &path), &text);
        b.doc = Some(doc.clone());
        let provider = Rc::new(lsp::Provider(doc));
        editor.update(cx, |e, cx| {
            let l = e.lsp_mut();
            l.completion_provider = Some(provider.clone());
            l.hover_provider = Some(provider.clone());
            l.definition_provider = Some(provider);
            // Definitions in other files open in a tab.
            l.show_document = Some(Rc::new(move |params, window, cx| {
                let Some(target) = lsp::uri_to_path(params.uri.as_str()) else { return false };
                let Some(ws) = ws.upgrade() else { return false };
                let line = params.selection.map(|r| r.start.line + 1).unwrap_or(1);
                let col = params.selection.map(|r| r.start.character + 1).unwrap_or(1);
                ws.update(cx, |this, cx| {
                    this.open_path(target, window, cx);
                    this.go_to(line, col, window, cx);
                });
                true
            }));
            cx.notify();
        });
        let _ = window;
        if let Some(diags) = self.diagnostics.get(&path).cloned() {
            self.apply_diagnostics(&path, &diags, cx);
        }
    }

    fn set_diagnostics(&mut self, uri: &str, diagnostics: Vec<lsp_types::Diagnostic>, cx: &mut Context<Self>) {
        let Some(path) = lsp::uri_to_path(uri) else { return };
        let path = path.canonicalize().unwrap_or(path);
        self.apply_diagnostics(&path, &diagnostics, cx);
        if diagnostics.is_empty() {
            self.diagnostics.remove(&path);
        } else {
            self.diagnostics.insert(path, diagnostics);
        }
        cx.notify();
    }

    fn apply_diagnostics(&mut self, path: &Path, diagnostics: &[lsp_types::Diagnostic], cx: &mut Context<Self>) {
        use gpui_kit::component::highlighter::Diagnostic;
        let Some(editor) = self
            .buffers
            .iter()
            .find(|b| b.path.as_deref() == Some(path))
            .and_then(|b| b.editor().cloned())
        else {
            return;
        };
        let items: Vec<Diagnostic> = diagnostics
            .iter()
            .map(|d| {
                let mut diag = Diagnostic::new(d.range.start..d.range.end, d.message.clone());
                if let Some(sev) = d.severity {
                    diag = diag.with_severity(sev);
                }
                diag
            })
            .collect();
        editor.update(cx, |e, cx| {
            if let Some(set) = e.diagnostics_mut() {
                set.clear();
                set.extend(items);
            }
            cx.notify();
        });
    }

    fn diagnostic_counts(&self) -> (usize, usize) {
        let mut errors = 0;
        let mut warnings = 0;
        for d in self.diagnostics.values().flatten() {
            match d.severity {
                Some(lsp_types::DiagnosticSeverity::ERROR) => errors += 1,
                Some(lsp_types::DiagnosticSeverity::WARNING) => warnings += 1,
                _ => {}
            }
        }
        (errors, warnings)
    }

    /// Reload open, unmodified files that changed on disk.
    fn reload_changed(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        for path in paths {
            let path = path.canonicalize().unwrap_or(path);
            let Some(b) = self.buffers.iter_mut().find(|b| b.path.as_ref() == Some(&path)) else { continue };
            if b.dirty {
                continue;
            }
            let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            if mtime.is_none() || mtime == b.mtime {
                continue;
            }
            b.mtime = mtime;
            let Some(editor) = b.editor().cloned() else { continue };
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            if editor.read(cx).value().as_ref() == text {
                continue;
            }
            let pos = editor.read(cx).cursor_position();
            editor.update(cx, |e, cx| {
                e.set_value(text, window, cx);
                e.set_cursor_position(pos, window, cx);
            });
            if let Some(b) = self.buffers.iter_mut().find(|b| b.path.as_ref() == Some(&path)) {
                b.dirty = false;
            }
        }
    }

    fn save_tabs(&mut self, cx: &App) {
        let Some(root) = self.root(cx) else { return };
        let key = root.to_string_lossy().to_string();
        let tabs: Vec<PathBuf> = self
            .buffers
            .iter()
            .filter(|b| matches!(b.kind, BufferKind::Text(_) | BufferKind::Image))
            .filter_map(|b| b.path.clone())
            .filter(|p| p.starts_with(&root))
            .collect();
        self.session.tabs.insert(key.clone(), tabs);
        match self.active_path() {
            Some(p) => {
                self.session.active_tab.insert(key, p);
            }
            None => {
                self.session.active_tab.remove(&key);
            }
        }
        self.session.save();
    }

    fn restore_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root(cx) else { return };
        let key = root.to_string_lossy().to_string();
        let tabs = self.session.tabs.get(&key).cloned().unwrap_or_default();
        let active = self.session.active_tab.get(&key).cloned();
        for p in tabs.into_iter().filter(|p| p.is_file()) {
            self.open_path(p, window, cx);
        }
        if let Some(active) = active {
            if let Some(ix) = self.buffers.iter().position(|b| b.path.as_ref() == Some(&active)) {
                self.activate(ix, window, cx);
            }
        }
    }

    fn open_diff(&mut self, rel: String, staged: bool, untracked: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root(cx) else { return };
        let title = format!(
            "{} ({})",
            Path::new(&rel).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(rel.clone()),
            if staged { "staged" } else { "changes" }
        );
        if let Some(ix) = self.buffers.iter().position(|b| matches!(&b.kind, BufferKind::Diff(_, t) if *t == title)) {
            self.remove_buffer(ix, window, cx);
        }
        let patch = match crate::git::diff(&root, &rel, staged, untracked) {
            Ok(p) => p,
            Err(err) => {
                window.push_notification(Notification::error(err).title("git diff"), cx);
                return;
            }
        };
        let language = icons::language_for(Path::new(&rel));
        let files: Vec<DiffFile> = DiffFile::parse(&patch)
            .unwrap_or_default()
            .into_iter()
            .map(|f| f.with_language(language.clone()))
            .collect();
        if files.is_empty() {
            self.open_path(root.join(&rel), window, cx);
            return;
        }
        let state = cx.new(|cx| DiffState::new(files, cx));
        let id = self.next_id;
        self.next_id += 1;
        self.push_buffer(
            Buffer {
                id,
                path: Some(root.join(&rel)),
                kind: BufferKind::Diff(state, title),
                language,
                dirty: false,
                tab: TabSize::default(),
                doc: None,
                mtime: None,
                _subscription: None,
            },
            window,
            cx,
        );
    }

    /// Run an editing command against the active code editor.
    fn edit_active(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut EditorState, &str, &mut Window, &mut Context<EditorState>),
    ) {
        let Some(ix) = self.active else { return };
        let language = self.buffers[ix].language.clone();
        if let Some(editor) = self.buffers[ix].editor().cloned() {
            editor.update(cx, |e, cx| f(e, &language, window, cx));
        }
    }

    fn show_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        const GROUPS: &[(&str, &[(&str, &str)])] = &[
            ("Files", &[("⌘P", "Go to file"), ("⇧⌘O", "Go to symbol"), ("⌃G", "Go to line"), ("⌘N", "New file"), ("⌘O", "Open folder"), ("⌘S", "Save"), ("⌘W", "Close tab"), ("⇧⌘T", "Reopen closed tab")]),
            ("Editing", &[("⌘/", "Toggle comment"), ("⌥↑ ⌥↓", "Move line"), ("⇧⌥↑ ⇧⌥↓", "Duplicate line"), ("⇧⌘K", "Delete line"), ("⌘D", "Select next occurrence"), ("⌥⌘↑ ⌥⌘↓", "Add cursor"), ("⌘F / ⌥⌘F", "Find / Replace"), ("⇧⌥F", "Format document"), ("F12", "Go to definition")]),
            ("Workspace", &[("⇧⌘P", "Commands"), ("⇧⌘F", "Find in files"), ("⌃⇧G", "Source control"), ("⇧⌘M", "Problems"), ("⌘B", "Sidebar"), ("⌘J", "Terminal"), ("⌘K Z", "Zen mode"), ("⌘= ⌘- ⌘0", "Zoom")]),
            ("Run & Tools", &[("⌘R", "Run file"), ("⌥⌘R", "Run task"), ("⌥L", "Live Server"), ("⇧⌘B", "Browser"), ("⌥⌘H", "HTTP client"), ("⇧⌘V", "Markdown preview"), ("⌘I", "AI assistant"), ("⌘,", "Settings")]),
        ];
        window.open_dialog(cx, |dialog, _, cx| {
            let theme = cx.theme().clone();
            dialog.title("Keyboard Shortcuts").w(px(720.)).child(
                div().grid().grid_cols(2).gap_x_8().gap_y_4().children(GROUPS.iter().map(|(title, rows)| {
                    v_flex()
                        .gap_1()
                        .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(theme.muted_foreground).child(*title))
                        .children(rows.iter().map(|(keys, label)| {
                            h_flex()
                                .justify_between()
                                .text_sm()
                                .child(div().child(*label))
                                .child(div().text_color(theme.muted_foreground).child(*keys))
                        }))
                })),
            )
        });
    }

    fn toggle_zen(&mut self, cx: &mut Context<Self>) {
        match self.zen.take() {
            Some((sidebar, terminal, tool)) => {
                self.sidebar_visible = sidebar;
                self.terminal_visible = terminal;
                self.tool = tool;
            }
            None => {
                self.zen = Some((self.sidebar_visible, self.terminal_visible, self.tool));
                self.sidebar_visible = false;
                self.terminal_visible = false;
                self.tool = None;
            }
        }
        self.sync_browser(cx);
        cx.notify();
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
                doc: None,
                mtime: None,
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
            let decoded = crate::opener::decode_text(&bytes);
            if decoded.is_none() {
                let total = bytes.len() as u64;
                let mut head = bytes;
                head.truncate(256 * 1024);
                Buffer {
                    id,
                    path: Some(path),
                    kind: BufferKind::Binary(Rc::new(head), total),
                    language,
                    dirty: false,
                    tab: TabSize::default(),
                    doc: None,
                    mtime: None,
                    _subscription: None,
                }
            } else {
                let text = decoded.unwrap_or_default();
                let tab = detect_indent(&text, Settings::get(cx).tab_size);
                let editor = self.make_editor(text, &language, window, cx);
                let sub = self.subscribe_editor(id, &editor, window, cx);
                Buffer {
                    id,
                    mtime: std::fs::metadata(&path).and_then(|m| m.modified()).ok(),
                    path: Some(path),
                    kind: BufferKind::Text(editor),
                    language,
                    dirty: false,
                    tab,
                    doc: None,
                    _subscription: Some(sub),
                }
            }
        };
        log::info!(
            "opened {} as {}",
            buffer.path.as_deref().map(|p| p.display().to_string()).unwrap_or_default(),
            match &buffer.kind {
                BufferKind::Text(_) => "text",
                BufferKind::Image => "image",
                BufferKind::Binary(..) => "binary",
                _ => "other",
            }
        );
        self.push_buffer(buffer, window, cx);
        self.attach_lsp(id, window, cx);
        self.save_tabs(cx);
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
                doc: None,
                mtime: None,
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
        self.save_tabs(cx);
        cx.notify();
    }

    fn remove_buffer(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.buffers.len() {
            return;
        }
        let removed = self.buffers.remove(ix);
        if let Some(doc) = &removed.doc {
            doc.close();
        }
        if let (Some(p), BufferKind::Text(_)) = (&removed.path, &removed.kind) {
            self.closed.retain(|c| c != p);
            self.closed.push(p.clone());
        }
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
        self.save_tabs(cx);
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

        let (trim, final_nl) = {
            let s = Settings::get(cx);
            (s.trim_trailing_whitespace, s.insert_final_newline)
        };
        if trim || final_nl {
            let current = editor.read(cx).value().to_string();
            let edits = editing::tidy_edits(&current, trim, final_nl);
            if !edits.is_empty() {
                editor.update(cx, |e, cx| e.replace_text_in_ranges(&edits, window, cx));
            }
        }
        let text = editor.read(cx).value().to_string();
        let format_on_save = Settings::get(cx).format_on_save;
        if (format || format_on_save) && formatter::can_format(&path) {
            let path2 = path.clone();
            let task = cx.background_spawn(async move { formatter::format(&path2, &text).map(|t| (t, text)) });
            cx.spawn_in(window, async move |this, cx| {
                let result = task.await;
                let result_ok = result.is_ok();
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
                            // Explicit "Format Document" explains itself; format-on-save
                            // stays quiet and flags the status line instead, so autosave
                            // can't stack up warnings.
                            if format {
                                window.push_notification(
                                    Notification::warning(err.clone()).title("Formatter").id::<FormatterNotice>(),
                                    cx,
                                );
                            }
                            this.format_error = Some(err);
                            editor.read(cx).value().to_string()
                        }
                    };
                    if result_ok {
                        this.format_error = None;
                    }
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
                    b.mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
                    if let Some(doc) = &b.doc {
                        doc.sync(text);
                        doc.saved();
                    }
                }
                self.git.update(cx, |g, cx| g.refresh(cx));
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

    /// Re-open a binary buffer as (lossy) text in the editor.
    fn open_as_text(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.buffers.get(ix).and_then(|b| b.path.clone()) else { return };
        let Ok(bytes) = std::fs::read(&path) else { return };
        let text = String::from_utf8_lossy(&bytes).replace('\0', "\u{2400}");
        let id = self.buffers[ix].id;
        let editor = self.make_editor(text, "text", window, cx);
        let sub = self.subscribe_editor(id, &editor, window, cx);
        let b = &mut self.buffers[ix];
        b.kind = BufferKind::Text(editor);
        b.language = "text".into();
        b._subscription = Some(sub);
        self.activate(ix, window, cx);
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
        let view = cx.new(|cx| SettingsView::new(window, cx));
        self.push_buffer(
            Buffer {
                id,
                path: None,
                kind: BufferKind::Settings(view),
                language: "settings".into(),
                dirty: false,
                tab: TabSize::default(),
                doc: None,
                mtime: None,
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
        self.spawn_terminal(cwd, true, window, cx)
    }

    fn spawn_terminal(
        &mut self,
        cwd: Option<PathBuf>,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalView> {
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
        if focus {
            let handle = term.focus_handle(cx);
            window.defer(cx, move |window, cx| handle.focus(window, cx));
        }
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
                Tool::Preview => {}
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
            ("Open File or Folder…", Some("cmd-o"), Box::new(OpenFolder)),
            ("Open File…", None, Box::new(OpenFile)),
            ("New File", Some("cmd-n"), Box::new(NewFile)),
            ("Save", Some("cmd-s"), Box::new(Save)),
            ("Save All", Some("cmd-alt-s"), Box::new(SaveAll)),
            ("Close Tab", Some("cmd-w"), Box::new(CloseTab)),
            ("Close Folder", None, Box::new(CloseFolder)),
            ("Toggle Sidebar", Some("cmd-b"), Box::new(ToggleSidebar)),
            ("Toggle Word Wrap", Some("alt-z"), Box::new(ToggleSoftWrap)),
            ("Toggle Rainbow Indent", None, Box::new(ToggleRainbowIndent)),
            ("Theme: Toggle Light / Dark", None, Box::new(ToggleTheme)),
            ("Run Task…", Some("cmd-alt-r"), Box::new(RunTask)),
            ("Go to Symbol in File", Some("cmd-shift-o"), Box::new(GoToSymbol)),
            ("Source Control", Some("ctrl-shift-g"), Box::new(ShowSourceControl)),
            ("Problems", Some("cmd-shift-m"), Box::new(ShowProblems)),
            ("Markdown: Open Preview", Some("cmd-shift-v"), Box::new(ToggleMarkdownPreview)),
            ("Reopen Closed Tab", Some("cmd-shift-t"), Box::new(ReopenClosedTab)),
            ("Toggle Comment", Some("cmd-/"), Box::new(ToggleComment)),
            ("Duplicate Line", Some("shift-alt-down"), Box::new(DuplicateLineDown)),
            ("Delete Line", Some("cmd-shift-k"), Box::new(DeleteLine)),
            ("Zen Mode", None, Box::new(ToggleZenMode)),
            ("Zoom In", Some("cmd-="), Box::new(ZoomIn)),
            ("Zoom Out", Some("cmd--"), Box::new(ZoomOut)),
            ("Reset Zoom", Some("cmd-0"), Box::new(ZoomReset)),
            ("Keyboard Shortcuts", None, Box::new(ShowShortcuts)),
            ("Restart Language Servers", None, Box::new(RestartLanguageServers)),
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
        let symbols = self
            .active
            .and_then(|ix| self.buffers.get(ix))
            .and_then(|b| b.editor().map(|e| symbols::extract(&e.read(cx).value(), &b.language)))
            .unwrap_or_default();
        let task_list = (initial == "!task").then(|| root.as_deref().map(tasks::detect).unwrap_or_default());
        let initial = if task_list.is_some() { String::new() } else { initial.to_string() };
        let palette = cx.new(|cx| Palette::new(&initial, root, files, commands, symbols, task_list, window, cx));
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
                PaletteEvent::RunTask(command) => {
                    let command = command.clone();
                    this.close_palette(window, cx);
                    let t = this.new_terminal(None, window, cx);
                    t.update(cx, |t, _| t.run(&command, false));
                    this.bottom_tab = BottomTab::Terminal;
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

    /// ⌘O: pick any files and/or a folder.
    fn open_folder_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt_open(true, true, window, cx);
    }

    fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt_open(true, false, window, cx);
    }

    fn prompt_open(&mut self, files: bool, directories: bool, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files,
            directories,
            multiple: true,
            prompt: Some("Open".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                let _ = this.update_in(cx, |this, window, cx| this.open_paths(paths, window, cx));
            }
        })
        .detach();
    }

    /// Open whatever was asked for: a folder becomes the project, files open
    /// as tabs. Works with or without a folder open.
    pub fn open_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let (dirs, files): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(|p| p.is_dir());
        if let Some(dir) = dirs.into_iter().next() {
            self.set_root(Some(dir), window, cx);
        }
        for file in files {
            if file.exists() {
                self.open_path(file, window, cx);
            } else {
                window.push_notification(
                    Notification::warning(format!("{} doesn't exist", file.display())),
                    cx,
                );
            }
        }
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
        let settings_open = self
            .active
            .and_then(|ix| self.buffers.get(ix))
            .is_some_and(|b| matches!(b.kind, BufferKind::Settings(_)));

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
                    )
                    .child(
                        capsule().child(
                            tool("tb-settings", Lucide::Settings, settings_open, "Settings  ⌘,")
                                .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
                        ),
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
            SidebarMode::Git => self.branch.clone().unwrap_or_else(|| "Source Control".into()),
        };
        let git_changes = self.git_changes;
        let has_root = self.root(cx).is_some();
        let icon_btn = |id: &'static str, icon: Lucide, tip: &'static str| {
            Button::new(id).ghost().xsmall().icon(icon).tooltip(tip)
        };
        let _ = (chrome_fill, chrome_line);
        v_flex()
            .size_full()
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
                        let mode_btn = |id: &'static str, icon: Lucide, m: SidebarMode, tip: &'static str| {
                            Button::new(id).ghost().xsmall().icon(icon).selected(mode == m).tooltip(tip)
                        };
                        d.child(
                            h_flex()
                                .gap(px(1.))
                                .child(mode_btn("sb-mode-files", Lucide::Files, SidebarMode::Files, "Explorer  ⇧⌘E").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.sidebar_mode = SidebarMode::Files;
                                        cx.notify();
                                    }),
                                ))
                                .child(mode_btn("sb-mode-search", Lucide::Search, SidebarMode::Search, "Search  ⇧⌘F").on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.sidebar_mode = SidebarMode::Search;
                                        let q = this.search.read(cx).input();
                                        q.update(cx, |s, cx| s.focus(window, cx));
                                        cx.notify();
                                    }),
                                ))
                                .child(
                                    div()
                                        .relative()
                                        .child(mode_btn("sb-mode-git", Lucide::GitBranch, SidebarMode::Git, "Source Control  ⌃⇧G").on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.sidebar_mode = SidebarMode::Git;
                                                this.git.update(cx, |g, cx| g.refresh(cx));
                                                cx.notify();
                                            }),
                                        ))
                                        .when(git_changes > 0, |d| {
                                            d.child(
                                                div()
                                                    .absolute()
                                                    .top(px(-2.))
                                                    .right(px(-3.))
                                                    .min_w(px(14.))
                                                    .h(px(14.))
                                                    .px(px(3.))
                                                    .rounded_full()
                                                    .bg(theme.primary)
                                                    .text_color(theme.primary_foreground)
                                                    .text_size(px(9.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(if git_changes > 99 { "99+".to_string() } else { git_changes.to_string() }),
                                            )
                                        }),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .when(mode == SidebarMode::Files, |d| d.child(self.explorer.clone()))
                    .when(mode == SidebarMode::Search, |d| d.child(self.search.clone()))
                    .when(mode == SidebarMode::Git, |d| d.child(self.git.clone())),
            )
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let dark = theme.is_dark();
        let active_fill = if dark { white().opacity(0.075) } else { black().opacity(0.055) };
        let hover_fill = if dark { white().opacity(0.04) } else { black().opacity(0.03) };
        h_flex()
            .id("tabs")
            .h(px(40.))
            .flex_shrink_0()
            .px_1p5()
            .gap_1()
            .border_b_1()
            .border_color(theme.border.opacity(0.6))
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
                    .when(!has_root, |d| d.child(hint("⌘O", "Open File or Folder")))
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
            BufferKind::Diff(state, _) => Diff::new(state)
                .line_number(true)
                .syntax_highlight(true)
                .font_family(theme.mono_font_family.clone())
                .text_size(px(font_size - 0.5))
                .into_any_element(),
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
            BufferKind::Binary(bytes, total) => {
                let bytes = bytes.clone();
                let rows = bytes.len().div_ceil(16);
                let shown_all = bytes.len() as u64 >= *total;
                let path = buffer.path.clone();
                let mono = theme.mono_font_family.clone();
                let muted = theme.muted_foreground;
                let fg = theme.foreground;
                v_flex()
                    .size_full()
                    .child(
                        h_flex()
                            .h(px(40.))
                            .px_3()
                            .gap_2()
                            .border_b_1()
                            .border_color(theme.border.opacity(0.6))
                            .text_sm()
                            .child(div().text_color(muted).child(format!(
                                "Binary file · {}{}",
                                crate::util::human_size(*total as usize),
                                if shown_all { String::new() } else { " · showing the first 256 KB".into() }
                            )))
                            .child(div().flex_1())
                            .child(
                                Button::new("bin-as-text")
                                    .small()
                                    .ghost()
                                    .icon(Lucide::FileText)
                                    .label("Open as Text")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Some(ix) = this.active {
                                            this.open_as_text(ix, window, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new("bin-default-app")
                                    .small()
                                    .ghost()
                                    .icon(Lucide::ExternalLink)
                                    .label("Open with Default App")
                                    .on_click(move |_, _, _| {
                                        if let Some(p) = &path {
                                            let _ = open::that(p);
                                        }
                                    }),
                            ),
                    )
                    .child(
                        uniform_list("hex-view", rows, move |range, _, _| {
                            range
                                .map(|row| {
                                    let (off, hex, ascii) = crate::opener::hex_row(&bytes, row);
                                    h_flex()
                                        .h(px(20.))
                                        .px_4()
                                        .gap_4()
                                        .font_family(mono.clone())
                                        .text_xs()
                                        .child(div().w(px(70.)).text_color(muted).child(off))
                                        .child(div().w(px(380.)).text_color(fg).child(hex))
                                        .child(div().text_color(muted).child(ascii))
                                })
                                .collect()
                        })
                        .flex_1()
                        .py_2(),
                    )
                    .into_any_element()
            }
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
        let tab = self.bottom_tab;
        let (errors, warnings) = self.diagnostic_counts();
        let panel_tab = |id: &'static str, label: &'static str, t: BottomTab| {
            let active = tab == t;
            h_flex()
                .id(id)
                .h(px(24.))
                .px_2()
                .gap_1p5()
                .rounded(px(6.))
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .when(active, |d| d.text_color(theme.foreground))
                .when(!active, |d| d.text_color(theme.muted_foreground).hover(|d| d.text_color(theme.foreground)))
                .child(label)
        };
        v_flex()
            .size_full()
            .border_t_1()
            .border_color(theme.border.opacity(0.7))
            .child(
                h_flex()
                    .h(px(32.))
                    .px_1p5()
                    .gap_1()
                    .child(panel_tab("ptab-term", "Terminal", BottomTab::Terminal).on_click(cx.listener(|this, _, _, cx| {
                        this.bottom_tab = BottomTab::Terminal;
                        cx.notify();
                    })))
                    .child(
                        panel_tab("ptab-problems", "Problems", BottomTab::Problems)
                            .when(errors + warnings > 0, |d| {
                                d.child(
                                    div()
                                        .px_1p5()
                                        .rounded_full()
                                        .bg(if errors > 0 { theme.danger.opacity(0.18) } else { theme.warning.opacity(0.18) })
                                        .text_color(if errors > 0 { theme.danger } else { theme.warning })
                                        .child((errors + warnings).to_string()),
                                )
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.bottom_tab = BottomTab::Problems;
                                cx.notify();
                            })),
                    )
                    .child(div().w(px(1.)).h(px(14.)).mx_1().bg(theme.border))
                    .when(tab == BottomTab::Terminal, |d| d.children(self.terminals.iter().enumerate().map(|(ix, t)| {
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
                    }).collect::<Vec<_>>()))
                    .when(tab == BottomTab::Terminal, |d| d.child(
                        Button::new("term-new")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .tooltip("New Terminal  ⌃⇧`")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_terminal(None, window, cx);
                            })),
                    ))
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
                    .when(tab == BottomTab::Terminal, |d| d.children(self.terminals.get(self.active_terminal).cloned()))
                    .when(tab == BottomTab::Problems, |d| d.child(self.render_problems(cx))),
            )
    }

    fn render_preview(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let source = self
            .active
            .and_then(|ix| self.buffers.get(ix))
            .filter(|b| b.language == "markdown")
            .and_then(|b| b.editor().map(|e| (b.title(), e.read(cx).value().to_string())));
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .h(px(40.))
                    .px_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .text_sm()
                    .child(gpui_kit::component::Icon::new(Lucide::BookOpen).size(px(14.)).text_color(theme.muted_foreground))
                    .child(div().font_weight(FontWeight::MEDIUM).child(
                        source.as_ref().map(|(t, _)| format!("Preview — {t}")).unwrap_or_else(|| "Markdown Preview".into()),
                    )),
            )
            .child(match source {
                Some((_, text)) => div()
                    .id("md-preview-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_6()
                    .py_5()
                    .child(TextView::markdown("md-preview", text).selectable(true))
                    .into_any_element(),
                None => div()
                    .p_4()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Open a Markdown file to see its live preview.")
                    .into_any_element(),
            })
    }

    fn render_problems(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let root = self.root(cx);
        if self.diagnostics.is_empty() {
            let servers: Vec<&str> = self
                .lsp
                .iter()
                .filter(|(_, e)| matches!(e, LspEntry::Ready(_)))
                .map(|(n, _)| *n)
                .collect();
            return div()
                .p_3()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(if servers.is_empty() {
                    "No problems. Install a language server (rust-analyzer, pyright, typescript-language-server, gopls, clangd…) to see errors and warnings here.".to_string()
                } else {
                    format!("No problems detected ({})", servers.join(", "))
                })
                .into_any_element();
        }
        div()
            .id("problems")
            .size_full()
            .overflow_y_scroll()
            .py_1()
            .children(self.diagnostics.iter().map(|(path, diags)| {
                let rel = root
                    .as_ref()
                    .and_then(|r| path.strip_prefix(r).ok())
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string());
                v_flex()
                    .child(
                        h_flex()
                            .h(px(24.))
                            .px_3()
                            .gap_1p5()
                            .text_sm()
                            .child(img(icons::file_icon(path)).size(px(14.)))
                            .child(rel)
                            .child(
                                div()
                                    .px_1p5()
                                    .rounded_full()
                                    .bg(theme.secondary)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(diags.len().to_string()),
                            ),
                    )
                    .children(diags.iter().enumerate().map(|(i, d)| {
                        let (icon, color) = match d.severity {
                            Some(lsp_types::DiagnosticSeverity::ERROR) => (IconName::CircleX, theme.danger),
                            Some(lsp_types::DiagnosticSeverity::WARNING) => (IconName::TriangleAlert, theme.warning),
                            _ => (IconName::Info, theme.info),
                        };
                        let path = path.clone();
                        let line = d.range.start.line + 1;
                        let col = d.range.start.character + 1;
                        let msg = d.message.lines().next().unwrap_or("").to_string();
                        h_flex()
                            .id(SharedString::from(format!("diag-{}-{i}", path.display())))
                            .h(px(24.))
                            .pl(px(30.))
                            .pr_3()
                            .gap_2()
                            .text_sm()
                            .cursor_pointer()
                            .hover(|d| d.bg(theme.list_hover))
                            .child(gpui_kit::component::Icon::new(icon).size(px(13.)).text_color(color))
                            .child(div().flex_1().truncate().child(msg))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "{}{}:{}",
                                        d.source.as_ref().map(|s| format!("{s}  ")).unwrap_or_default(),
                                        line,
                                        col
                                    )),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_path(path.clone(), window, cx);
                                this.go_to(line, col, window, cx);
                            }))
                    }))
            }))
            .into_any_element()
    }

    fn render_tool_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (_, chrome_line) = chrome(&theme);
        let content: Option<AnyElement> = match self.tool {
            Some(Tool::Browser) => self.browser.clone().map(|b| b.into_any_element()),
            Some(Tool::Http) => self.http.clone().map(|b| b.into_any_element()),
            Some(Tool::Ai) => self.ai.clone().map(|b| b.into_any_element()),
            Some(Tool::Preview) => Some(self.render_preview(cx).into_any_element()),
            None => None,
        };
        div()
            .size_full()
            .overflow_hidden()
            .bg(theme.background)
            .border_l_1()
            .border_t_1()
            .border_color(chrome_line)
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
                if let Some((spec, _)) = lsp::resolve(&b.language).filter(|_| b.path.is_some()) {
                    let state = match self.lsp.get(spec.name) {
                        Some(LspEntry::Ready(_)) if b.doc.is_some() => Some((spec.name.to_string(), theme.muted_foreground)),
                        Some(LspEntry::Starting(_)) => Some((format!("{} starting…", spec.name), theme.muted_foreground)),
                        Some(LspEntry::Failed) => Some((format!("{} failed", spec.name), theme.danger)),
                        _ => None,
                    };
                    if let Some((label, color)) = state {
                        right.push(
                            item("sb-lsp")
                                .text_color(color)
                                .child(gpui_kit::component::Icon::new(Lucide::Braces).size(px(11.)))
                                .child(label)
                                .tooltip(|window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new("Language server · click to restart").build(window, cx)
                                })
                                .on_click(|_, window, cx| window.dispatch_action(Box::new(RestartLanguageServers), cx))
                                .into_any_element(),
                        );
                    }
                }
            }
            if let Some(name) = b.path.as_ref().and_then(|p| formatter::formatter_name(p)) {
                let fmt_error: SharedString = self
                    .format_error
                    .clone()
                    .map(|e| format!("Formatting failed: {}", e.lines().next().unwrap_or("")))
                    .unwrap_or_else(|| "Format Document  ⇧⌥F".into())
                    .into();
                right.push(
                    item("sb-fmt")
                        .when(self.format_error.is_some(), |d| d.text_color(theme.danger))
                        .when(self.format_error.is_some(), |d| {
                            d.child(gpui_kit::component::Icon::new(IconName::TriangleAlert).size(px(11.)))
                        })
                        .when(settings.format_on_save && self.format_error.is_none(), |d| {
                            d.child(gpui_kit::component::Icon::new(IconName::Check).size(px(11.)))
                        })
                        .child(name)
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(fmt_error.clone()).build(window, cx)
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

        let empty = right.is_empty() && self.branch.is_none() && self.live.is_none();
        h_flex()
            .h(px(26.))
            .flex_shrink_0()
            .px_2()
            .justify_between()
            .text_xs()
            .text_color(theme.muted_foreground)
            .when(empty, |d| d.hidden())
            .child(
                h_flex()
                    .gap_0p5()
                    .when_some(self.branch.clone(), |d, branch| {
                        let changes = self.git_changes;
                        d.child(
                            item("sb-branch")
                                .child(gpui_kit::component::Icon::new(Lucide::GitBranch).size(px(12.)))
                                .child(branch)
                                .when(changes > 0, |d| d.child(format!("· {changes}")))
                                .tooltip(|window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new("Source Control  ⌃⇧G").build(window, cx)
                                })
                                .on_click(|_, window, cx| window.dispatch_action(Box::new(ShowSourceControl), cx)),
                        )
                    })
                    .map(|d| {
                        let (errors, warnings) = self.diagnostic_counts();
                        d.child(
                            item("sb-problems")
                                .child(gpui_kit::component::Icon::new(IconName::CircleX).size(px(12.)))
                                .child(errors.to_string())
                                .child(gpui_kit::component::Icon::new(IconName::TriangleAlert).size(px(12.)))
                                .child(warnings.to_string())
                                .tooltip(|window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new("Problems  ⇧⌘M").build(window, cx)
                                })
                                .on_click(|_, window, cx| window.dispatch_action(Box::new(ShowProblems), cx)),
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

        // One continuous solid surface for tabs + editor, terminal and the
        // status line; only its leading corner is rounded against the glass.
        let card = v_flex()
            .size_full()
            .overflow_hidden()
            .bg(theme.background)
            .when(sidebar, |d| d.rounded_tl(px(10.)).border_l_1().border_t_1().border_color(chrome_line))
            .when(!sidebar, |d| d.border_t_1().border_color(chrome_line))
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
                    .child(self.render_sidebar(cx)),
            )
            .child(
                resizable_panel().size_range(px(240.)..Pixels::MAX).child(card),
            )
            .child(
                resizable_panel()
                    .size(px(480.))
                    .size_range(px(300.)..px(1600.))
                    .visible(tool_open)
                    .child(self.render_tool_panel(cx)),
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
                        let hint = this.buffers[ix]
                            .path
                            .as_deref()
                            .and_then(formatter::missing_hint)
                            .unwrap_or("There's no formatter for this file type.");
                        window.push_notification(
                            Notification::info(hint).title("No formatter installed").id::<FormatterNotice>(),
                            cx,
                        );
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
            .on_action(cx.listener(|this, _: &ToggleComment, window, cx| {
                this.edit_active(window, cx, |e, lang, window, cx| editing::toggle_comment(e, lang, window, cx))
            }))
            .on_action(cx.listener(|this, _: &MoveLineUp, window, cx| {
                this.edit_active(window, cx, |e, _, window, cx| editing::move_lines(e, true, window, cx))
            }))
            .on_action(cx.listener(|this, _: &MoveLineDown, window, cx| {
                this.edit_active(window, cx, |e, _, window, cx| editing::move_lines(e, false, window, cx))
            }))
            .on_action(cx.listener(|this, _: &DuplicateLineUp, window, cx| {
                this.edit_active(window, cx, |e, _, window, cx| editing::duplicate_lines(e, false, window, cx))
            }))
            .on_action(cx.listener(|this, _: &DuplicateLineDown, window, cx| {
                this.edit_active(window, cx, |e, _, window, cx| editing::duplicate_lines(e, true, window, cx))
            }))
            .on_action(cx.listener(|this, _: &DeleteLine, window, cx| {
                this.edit_active(window, cx, |e, _, window, cx| editing::delete_lines(e, window, cx))
            }))
            .on_action(cx.listener(|this, _: &SelectNextOccurrence, window, cx| {
                this.edit_active(window, cx, |e, _, _, cx| editing::select_next_occurrence(e, cx))
            }))
            .on_action(cx.listener(|this, _: &ReopenClosedTab, window, cx| {
                if let Some(p) = this.closed.pop() {
                    this.open_path(p, window, cx);
                }
            }))
            .on_action(cx.listener(|_, _: &ZoomIn, _, cx| {
                Settings::update(cx, |s| {
                    s.font_size = (s.font_size + 1.).min(32.);
                    s.terminal_font_size = (s.terminal_font_size + 1.).min(28.);
                })
            }))
            .on_action(cx.listener(|_, _: &ZoomOut, _, cx| {
                Settings::update(cx, |s| {
                    s.font_size = (s.font_size - 1.).max(9.);
                    s.terminal_font_size = (s.terminal_font_size - 1.).max(9.);
                })
            }))
            .on_action(cx.listener(|_, _: &ZoomReset, _, cx| {
                Settings::update(cx, |s| {
                    s.font_size = 13.;
                    s.terminal_font_size = 13.;
                })
            }))
            .on_action(cx.listener(|this, _: &ToggleMarkdownPreview, window, cx| this.show_tool(Tool::Preview, window, cx)))
            .on_action(cx.listener(|this, _: &RunTask, window, cx| this.open_palette("!task", window, cx)))
            .on_action(cx.listener(|this, _: &GoToSymbol, window, cx| this.open_palette("@", window, cx)))
            .on_action(cx.listener(|this, _: &ShowShortcuts, window, cx| this.show_shortcuts(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleZenMode, _, cx| this.toggle_zen(cx)))
            .on_action(cx.listener(|this, _: &ShowSourceControl, _, cx| {
                this.sidebar_visible = true;
                this.sidebar_mode = SidebarMode::Git;
                this.git.update(cx, |g, cx| g.refresh(cx));
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowProblems, window, cx| {
                if this.terminals.is_empty() {
                    this.spawn_terminal(None, false, window, cx);
                }
                this.terminal_visible = true;
                this.bottom_tab = BottomTab::Problems;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &RestartLanguageServers, window, cx| {
                this.lsp.clear();
                this.diagnostics.clear();
                for b in this.buffers.iter_mut() {
                    b.doc = None;
                }
                let ids: Vec<u64> = this.buffers.iter().map(|b| b.id).collect();
                for id in ids {
                    this.attach_lsp(id, window, cx);
                }
                window.push_notification("Language servers restarted", cx);
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                for p in paths.paths() {
                    this.open_path(p.clone(), window, cx);
                }
            }))
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
