//! Integrated terminal: alacritty's emulator core, rendered natively with GPUI.

use std::{borrow::Cow, collections::HashMap, path::PathBuf, sync::Arc};

use alacritty_terminal::{
    event::{Event as TermEvent, EventListener, Notify, OnResize, WindowSize},
    event_loop::{EventLoop, Notifier},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point as GridPoint, Side},
    selection::{Selection, SelectionType},
    sync::FairMutex,
    term::{Config, Term, TermMode, cell::Flags},
    tty,
    vte::ansi::{Color, NamedColor, Rgb},
};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{prelude::FluentBuilder as _, *};

#[derive(Clone)]
pub struct Listener(smol::channel::Sender<TermEvent>);

impl EventListener for Listener {
    fn send_event(&self, event: TermEvent) {
        let _ = self.0.try_send(event);
    }
}

#[derive(Clone, Copy, PartialEq)]
struct GridSize {
    cols: usize,
    rows: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

pub enum TerminalEvent {
    TitleChanged,
    Exited,
}

pub struct TerminalView {
    term: Arc<FairMutex<Term<Listener>>>,
    notifier: Notifier,
    focus_handle: FocusHandle,
    pub title: SharedString,
    pub exited: bool,
    size: GridSize,
    cell: Size<Pixels>,
    origin: Point<Pixels>,
    selecting: bool,
    _events: Task<()>,
}

impl EventEmitter<TerminalEvent> for TerminalView {}


impl TerminalView {
    pub fn new(cwd: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (tx, rx) = smol::channel::unbounded();
        let listener = Listener(tx);
        let size = GridSize { cols: 100, rows: 24 };
        let term = Term::new(Config::default(), &size, listener.clone());
        let term = Arc::new(FairMutex::new(term));

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let mut env = HashMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());
        env.insert("TERM_PROGRAM".to_string(), "orion".to_string());
        let options = tty::Options {
            shell: Some(tty::Shell::new(shell.clone(), vec!["-l".into()])),
            working_directory: cwd.filter(|p| p.is_dir()).or_else(dirs::home_dir),
            drain_on_exit: true,
            env,
        };
        let window_size = WindowSize {
            num_lines: size.rows as u16,
            num_cols: size.cols as u16,
            cell_width: 8,
            cell_height: 16,
        };
        let pty = tty::new(&options, window_size, 0).expect("failed to open a pty");
        let event_loop = EventLoop::new(term.clone(), listener, pty, true, false)
            .expect("failed to start terminal");
        let notifier = Notifier(event_loop.channel());
        event_loop.spawn();

        let events = cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = rx.recv().await {
                let keep = this.update(cx, |this, cx| this.handle_event(event, cx));
                if keep.is_err() {
                    break;
                }
            }
        });

        let name = std::path::Path::new(&shell)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "shell".into());

        Self {
            term,
            notifier,
            focus_handle: cx.focus_handle(),
            title: name.into(),
            exited: false,
            size,
            cell: gpui_kit::size(px(8.), px(16.)),
            origin: Point::default(),
            selecting: false,
            _events: events,
        }
    }

    fn handle_event(&mut self, event: TermEvent, cx: &mut Context<Self>) {
        match event {
            TermEvent::Wakeup | TermEvent::CursorBlinkingChange | TermEvent::MouseCursorDirty => {
                cx.notify()
            }
            TermEvent::Title(title) => {
                self.title = title.into();
                cx.emit(TerminalEvent::TitleChanged);
                cx.notify();
            }
            TermEvent::PtyWrite(text) => self.write(text.into_bytes()),
            TermEvent::ClipboardStore(_, text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text))
            }
            TermEvent::ClipboardLoad(_, format) => {
                let text = cx
                    .read_from_clipboard()
                    .and_then(|c| c.text())
                    .unwrap_or_default();
                self.write(format(&text).into_bytes());
            }
            TermEvent::ColorRequest(index, format) => {
                let rgb = to_rgb(default_color(index));
                self.write(format(rgb).into_bytes());
            }
            TermEvent::TextAreaSizeRequest(format) => {
                let ws = self.window_size();
                self.write(format(ws).into_bytes());
            }
            TermEvent::Exit | TermEvent::ChildExit(_) => {
                self.exited = true;
                cx.emit(TerminalEvent::Exited);
                cx.notify();
            }
            TermEvent::Bell | TermEvent::ResetTitle => {}
        }
    }

    fn window_size(&self) -> WindowSize {
        WindowSize {
            num_lines: self.size.rows as u16,
            num_cols: self.size.cols as u16,
            cell_width: f32::from(self.cell.width) as u16,
            cell_height: f32::from(self.cell.height) as u16,
        }
    }

    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        self.notifier.notify(bytes);
    }

    /// Run a shell command line in this terminal, as if typed.
    pub fn run(&mut self, command: &str, clear: bool) {
        let mut line = String::new();
        if clear {
            line.push_str("clear; ");
        }
        line.push_str(command);
        line.push('\r');
        self.term.lock().scroll_display(Scroll::Bottom);
        self.write(line.into_bytes());
    }

    fn resize(&mut self, bounds: Bounds<Pixels>, cell: Size<Pixels>) {
        self.origin = bounds.origin;
        self.cell = cell;
        let cols = ((bounds.size.width / cell.width).floor() as usize).max(2);
        let rows = ((bounds.size.height / cell.height).floor() as usize).max(1);
        let next = GridSize { cols, rows };
        if next != self.size {
            self.size = next;
            self.term.lock().resize(next);
            let ws = self.window_size();
            self.notifier.on_resize(ws);
        }
    }

    fn grid_point(&self, position: Point<Pixels>) -> (GridPoint, Side) {
        let x = (position.x - self.origin.x).max(px(0.));
        let y = (position.y - self.origin.y).max(px(0.));
        let col_f = x / self.cell.width;
        let col = (col_f.floor() as usize).min(self.size.cols.saturating_sub(1));
        let row = (y / self.cell.height).floor() as i32;
        let row = row.min(self.size.rows as i32 - 1);
        let offset = self.term.lock().grid().display_offset() as i32;
        let side = if col_f.fract() > 0.5 { Side::Right } else { Side::Left };
        (GridPoint::new(Line(row - offset), Column(col)), side)
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let m = ks.modifiers;
        if m.platform {
            match ks.key.as_str() {
                "c" => {
                    if let Some(text) = self.term.lock().selection_to_string() {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                        self.paste(&text);
                    }
                }
                "k" => {
                    self.write(b"\x0c".as_slice());
                    let mut term = self.term.lock();
                    term.grid_mut().clear_history();
                }
                "a" => {
                    let mut term = self.term.lock();
                    let top = term.grid().topmost_line();
                    let bottom = term.grid().bottommost_line();
                    let last = term.grid().last_column();
                    let mut sel = Selection::new(
                        SelectionType::Simple,
                        GridPoint::new(top, Column(0)),
                        Side::Left,
                    );
                    sel.update(GridPoint::new(bottom, last), Side::Right);
                    term.selection = Some(sel);
                }
                _ => {
                    cx.propagate();
                    return;
                }
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }

        let mode = *self.term.lock().mode();
        let app_cursor = mode.contains(TermMode::APP_CURSOR);
        let arrow = |c: char| {
            if m.shift || m.alt || m.control {
                let code = 1 + (m.shift as u8) + 2 * (m.alt as u8) + 4 * (m.control as u8);
                format!("\x1b[1;{code}{c}")
            } else if app_cursor {
                format!("\x1bO{c}")
            } else {
                format!("\x1b[{c}")
            }
        };
        let bytes: Option<Vec<u8>> = match ks.key.as_str() {
            "enter" => Some(if m.alt { b"\x1b\r".to_vec() } else { b"\r".to_vec() }),
            "backspace" => Some(if m.alt {
                b"\x1b\x7f".to_vec()
            } else if m.control {
                b"\x08".to_vec()
            } else {
                b"\x7f".to_vec()
            }),
            "tab" => Some(if m.shift { b"\x1b[Z".to_vec() } else { b"\t".to_vec() }),
            "escape" => Some(b"\x1b".to_vec()),
            "up" => Some(arrow('A').into_bytes()),
            "down" => Some(arrow('B').into_bytes()),
            "right" => Some(if m.alt && !m.shift {
                b"\x1bf".to_vec()
            } else {
                arrow('C').into_bytes()
            }),
            "left" => Some(if m.alt && !m.shift {
                b"\x1bb".to_vec()
            } else {
                arrow('D').into_bytes()
            }),
            "home" => Some(b"\x1b[H".to_vec()),
            "end" => Some(b"\x1b[F".to_vec()),
            "pageup" if m.shift => {
                self.term.lock().scroll_display(Scroll::PageUp);
                cx.notify();
                None
            }
            "pagedown" if m.shift => {
                self.term.lock().scroll_display(Scroll::PageDown);
                cx.notify();
                None
            }
            "pageup" => Some(b"\x1b[5~".to_vec()),
            "pagedown" => Some(b"\x1b[6~".to_vec()),
            "delete" => Some(b"\x1b[3~".to_vec()),
            "space" if m.control => Some(vec![0]),
            key if m.control && key.chars().count() == 1 => {
                let c = key.chars().next().unwrap().to_ascii_lowercase();
                match c {
                    'a'..='z' => Some(vec![(c as u8) & 0x1f]),
                    '[' => Some(vec![0x1b]),
                    '\\' => Some(vec![0x1c]),
                    ']' => Some(vec![0x1d]),
                    '/' => Some(vec![0x1f]),
                    _ => None,
                }
            }
            _ => {
                if m.alt && !m.shift && ks.key.chars().count() == 1 {
                    // Option acts as Meta, like most terminal setups for developers.
                    Some(format!("\x1b{}", ks.key).into_bytes())
                } else {
                    ks.key_char.clone().map(|s| s.into_bytes()).or_else(|| {
                        (ks.key == "space").then(|| b" ".to_vec())
                    })
                }
            }
        };
        if let Some(bytes) = bytes {
            {
                let mut term = self.term.lock();
                term.selection = None;
                term.scroll_display(Scroll::Bottom);
            }
            self.write(bytes);
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn paste(&self, text: &str) {
        let bracketed = self.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        if bracketed {
            self.write(format!("\x1b[200~{text}\x1b[201~").into_bytes());
        } else {
            self.write(text.into_bytes());
        }
    }

    fn on_scroll(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(self.cell.height);
        let lines = (delta.y / self.cell.height).round() as i32;
        if lines == 0 {
            return;
        }
        let alt_screen = self.term.lock().mode().contains(TermMode::ALT_SCREEN);
        if alt_screen {
            let seq: &[u8] = if lines > 0 { b"\x1b[A" } else { b"\x1b[B" };
            for _ in 0..lines.unsigned_abs().min(10) {
                self.write(seq.to_vec());
            }
        } else {
            self.term.lock().scroll_display(Scroll::Delta(lines));
        }
        cx.notify();
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        let (point, side) = self.grid_point(event.position);
        let ty = match event.click_count {
            2 => SelectionType::Semantic,
            3 => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        self.term.lock().selection = Some(Selection::new(ty, point, side));
        self.selecting = true;
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selecting || event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let (point, side) = self.grid_point(event.position);
        if let Some(sel) = self.term.lock().selection.as_mut() {
            sel.update(point, side);
        }
        cx.notify();
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.selecting = false;
        let mut term = self.term.lock();
        if term.selection.as_ref().is_some_and(|s| s.is_empty()) {
            term.selection = None;
        }
        cx.notify();
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn hex(c: u32) -> Hsla {
    rgb(c).into()
}

fn default_color(index: usize) -> Hsla {
    const ANSI: [u32; 16] = [
        0x1c1f27, 0xf2727f, 0x7fd19b, 0xe5c07b, 0x7c9cff, 0xc49bff, 0x6fd3e6, 0xc8ccd6, 0x4b5160,
        0xff8f9a, 0x9be3b2, 0xf3d29a, 0x9cb4ff, 0xd6b6ff, 0x92e2f0, 0xf2f4f8,
    ];
    match index {
        0..=15 => hex(ANSI[index]),
        16..=231 => {
            let i = index - 16;
            let step = |v: usize| if v == 0 { 0 } else { 55 + v as u32 * 40 };
            let (r, g, b) = (step(i / 36), step((i / 6) % 6), step(i % 6));
            hex((r << 16) | (g << 8) | b)
        }
        232..=255 => {
            let v = 8 + (index as u32 - 232) * 10;
            hex((v << 16) | (v << 8) | v)
        }
        _ => hex(0xd5d8e0),
    }
}

fn to_rgb(c: Hsla) -> Rgb {
    let rgba = c.to_rgb();
    Rgb {
        r: (rgba.r * 255.) as u8,
        g: (rgba.g * 255.) as u8,
        b: (rgba.b * 255.) as u8,
    }
}

fn resolve(color: Color, fg: Hsla, bg: Hsla, colors: &alacritty_terminal::term::color::Colors) -> Hsla {
    match color {
        Color::Spec(c) => Rgba {
            r: c.r as f32 / 255.,
            g: c.g as f32 / 255.,
            b: c.b as f32 / 255.,
            a: 1.,
        }
        .into(),
        Color::Indexed(i) => colors[i as usize]
            .map(|c| resolve(Color::Spec(c), fg, bg, colors))
            .unwrap_or_else(|| default_color(i as usize)),
        Color::Named(named) => {
            if let Some(c) = colors[named] {
                return resolve(Color::Spec(c), fg, bg, colors);
            }
            match named {
                NamedColor::Foreground | NamedColor::BrightForeground => fg,
                NamedColor::Background => bg,
                NamedColor::DimForeground => fg.opacity(0.7),
                NamedColor::Cursor => fg,
                n => {
                    let idx = n as usize;
                    if idx < 16 {
                        default_color(idx)
                    } else if idx >= NamedColor::DimBlack as usize {
                        default_color(idx - NamedColor::DimBlack as usize).opacity(0.7)
                    } else {
                        fg
                    }
                }
            }
        }
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let fg = theme.foreground;
        let bg = theme.background;
        let term_size = crate::settings::Settings::get(cx).terminal_font_size.clamp(8., 32.);
        let font_size = px(term_size);
        let line_height = px((term_size * 1.4).round());
        let family = theme.mono_font_family.clone();
        let base_font = font(family.clone());
        let font_id = window.text_system().resolve_font(&base_font);
        let cell_width = window
            .text_system()
            .advance(font_id, font_size, 'm')
            .map(|s| s.width)
            .unwrap_or(px(8.));
        let cell = size(cell_width, line_height);
        let focused = self.focus_handle.is_focused(window);
        let selection_bg = theme.selection;
        let cursor_color = theme.caret;

        let term = self.term.lock();
        let content = term.renderable_content();
        let rows = self.size.rows;
        let display_offset = content.display_offset as i32;
        let selection = content.selection;
        let cursor = content.cursor;
        let show_cursor = content.mode.contains(TermMode::SHOW_CURSOR);
        let colors = content.colors;

        let mut lines: Vec<(String, Vec<TextRun>)> = vec![(String::new(), Vec::new()); rows];
        for indexed in content.display_iter {
            let row = indexed.point.line.0 + display_offset;
            if row < 0 || row as usize >= rows {
                continue;
            }
            let cell_flags = indexed.cell.flags;
            if cell_flags.contains(Flags::WIDE_CHAR_SPACER)
                || cell_flags.contains(Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            let mut cfg = resolve(indexed.cell.fg, fg, bg, colors);
            let mut cbg = resolve(indexed.cell.bg, fg, bg, colors);
            if cell_flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut cfg, &mut cbg);
            }
            if cell_flags.contains(Flags::DIM) {
                cfg = cfg.opacity(0.65);
            }
            let selected = selection
                .as_ref()
                .is_some_and(|s| s.contains(indexed.point));
            let background = if selected {
                Some(selection_bg)
            } else if cbg != bg {
                Some(cbg)
            } else {
                None
            };
            let c = if cell_flags.contains(Flags::HIDDEN) {
                ' '
            } else {
                indexed.cell.c
            };
            let mut f = base_font.clone();
            if cell_flags.contains(Flags::BOLD) {
                f.weight = FontWeight::BOLD;
            }
            if cell_flags.contains(Flags::ITALIC) {
                f.style = FontStyle::Italic;
            }
            let underline = cell_flags.intersects(Flags::ALL_UNDERLINES).then(|| UnderlineStyle {
                thickness: px(1.),
                color: Some(cfg),
                wavy: cell_flags.contains(Flags::UNDERCURL),
            });
            let (text, runs) = &mut lines[row as usize];
            let start = text.len();
            text.push(if c == '\0' { ' ' } else { c });
            if let Some(zw) = indexed.cell.zerowidth() {
                for z in zw {
                    text.push(*z);
                }
            }
            let len = text.len() - start;
            match runs.last_mut() {
                Some(last)
                    if last.color == cfg
                        && last.background_color == background
                        && last.font == f
                        && last.underline == underline =>
                {
                    last.len += len
                }
                _ => runs.push(TextRun {
                    len,
                    font: f,
                    color: cfg,
                    background_color: background,
                    underline,
                    strikethrough: None,
                }),
            }
        }
        drop(term);

        let cursor_row = cursor.point.line.0 + display_offset;
        let cursor_el = (show_cursor && !self.exited && cursor_row >= 0 && (cursor_row as usize) < rows)
            .then(|| {
                let x = cell_width * cursor.point.column.0 as f32;
                let y = line_height * cursor_row as f32;
                div()
                    .absolute()
                    .left(x)
                    .top(y)
                    .w(cell_width)
                    .h(line_height)
                    .when(focused, |d| d.bg(cursor_color.opacity(0.75)))
                    .when(!focused, |d| d.border_1().border_color(cursor_color.opacity(0.6)))
            });

        let view = cx.entity();
        div()
            .id("terminal")
            .key_context("Terminal")
            .track_focus(&self.focus_handle)
            .size_full()
            .px_3()
            .pt_1()
            .bg(bg)
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_scroll(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .child(
                div()
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .font_family(family)
                    .text_size(font_size)
                    .line_height(line_height)
                    .text_color(fg)
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                view.update(cx, |this, _| this.resize(bounds, cell));
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .children(lines.into_iter().enumerate().map(|(ix, (text, runs))| {
                        div()
                            .id(("term-row", ix))
                            .h(line_height)
                            .whitespace_nowrap()
                            .child(if text.is_empty() {
                                StyledText::new(" ")
                            } else {
                                StyledText::new(text).with_runs(runs)
                            })
                    }))
                    .children(cursor_el),
            )
    }
}
