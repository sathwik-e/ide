<p align="center"><img src="assets/icon.png" width="128" alt="IDE icon"></p>

<h1 align="center">IDE</h1>

<p align="center">A fast, quiet, native code editor written in Rust. No built-in AI, no clutter, and every tool you reach for is already inside it.</p>

---

Built on [GPUI](https://github.com/zed-industries/zed) (Zed's GPU-accelerated UI framework) and [GPUI Kit](https://github.com/longbridge/gpui-kit). The UI is rendered on the GPU with Metal, and the terminal, browser and HTTP client are all native parts of the app, not plugins.

## What's inside

| Feature | How you use it |
| --- | --- |
| **Editor** with syntax highlighting for ~200 languages | Tree-sitter grammars for 35 common languages (with folding), plus Sublime grammars for everything else. Any text file opens and edits. |
| **Rainbow indent guides** | On by default. Each indent depth gets its own color. Toggle from the palette. |
| **VS Code-style file icons** | Icon artwork from vscode-icons, in the explorer, tabs and quick-open. |
| **Integrated terminal** | `⌘J`. A real terminal (Alacritty's emulator core) with 256-color and true color, selection, scrollback and multiple tabs. |
| **Code Runner** | `⌘R` or `⌃⌥N`, or ▶ in the title bar. Runs the current file in 40+ languages, in the terminal, so programs can read input. Override any language in settings. |
| **Prettier** | `⇧⌥F`. Uses a project-local Prettier, then global, then `npx`. Native formatters for Rust, Python (ruff/black), Go, C/C++, Swift, Zig, Lua and more. Optional format-on-save. |
| **Live Server** | `⌥L`. Serves the folder and reloads the page on every save. Running an `.html` file starts it automatically. |
| **Built-in browser** | `⇧⌘B`. Desktop, iPhone, Pixel and iPad views with the matching user agent and viewport, rotation, auto-fit scaling, and Web Inspector. |
| **HTTP client** (Thunder Client-style) | `⌥⌘H`. Method, URL, headers, JSON body and Bearer auth. Pretty-printed, highlighted responses with status, time and size, plus request history. |
| **Optional AI** | Off by default. Turn it on from the palette (*AI: Set Up Assistant*) with your own key for **Claude**, **ChatGPT** or **Gemini**. Answers stream in a side panel, and the open file can be included as context. |
| **Command palette / quick open** | `⌘P` for files (fuzzy), `>` for commands, `:` to go to a line. `⇧⌘P` opens directly into commands. |
| **Find in files** | `⇧⌘F`. Respects `.gitignore`. |
| **Themes** | *Midnight* (dark) and *Daylight* (light). JetBrains Mono is bundled. |

## Keyboard

| | |
|---|---|
| `⌘O` open folder · `⇧⌘O` open file · `⌘N` new file | `⌘S` save · `⌥⌘S` save all · `⌘W` close tab |
| `⌘P` go to file · `⇧⌘P` commands · `⌃G` go to line | `⌘B` sidebar · `⇧⌘E` explorer · `⇧⌘F` search |
| `⌘J` terminal · `⌃⇧\`` new terminal | `⌘R` run file · `⌥L` Live Server · `⇧⌥F` format |
| `⇧⌘B` browser · `⌥⌘H` HTTP client · `⌘I` AI (when enabled) | `⌃Tab` / `⌃⇧Tab` switch tabs · `⌥Z` word wrap · `⌘,` settings |

## Settings

`⌘,` opens `~/Library/Application Support/ide/settings.json` in the editor. Saving it applies the changes. The file is created with permissions `600`, since it can hold API keys.

```jsonc
{
  "theme": "Midnight",            // or "Daylight"
  "font_size": 13.0,
  "tab_size": 4,                   // default; each file's own indentation is detected
  "soft_wrap": false,
  "format_on_save": false,
  "auto_save": false,
  "rainbow_indent": true,
  "live_server_port": 5500,
  "runner_clear_before_run": true,
  "runner_save_before_run": true,
  "runner_executor_map": { "py": "uv run $file" },   // $file $dir $fileName $fileNameWithoutExt
  "ai": {
    "enabled": false,
    "provider": "anthropic",       // "openai" | "gemini"
    "anthropic": { "api_key": "", "model": "claude-opus-5-5" },
    "openai":    { "api_key": "", "model": "gpt-5" },
    "gemini":    { "api_key": "", "model": "gemini-2.5-pro" }
  }
}
```

## Build

Requires Rust (stable) and Xcode command-line tools on macOS.

```bash
cargo run --release -- /path/to/project
```

```bash
./script/bundle-macos.sh        # builds target/release/bundle/IDE.app
```

```bash
cargo test
```

## Layout

```
src/
  main.rs          app bootstrap, theme, fonts
  workspace.rs     window layout, tabs, actions, keybindings, menus
  explorer.rs      file tree with watcher and file operations
  palette.rs       quick open / commands / go to line
  search.rs        find in files
  terminal.rs      terminal emulator view (alacritty_terminal)
  runner.rs        Code Runner executor map
  formatter.rs     Prettier and native formatters
  live_server.rs   static server with live reload
  browser.rs       embedded WebView with device emulation
  http_client.rs   HTTP client panel
  ai.rs            optional Claude / ChatGPT / Gemini chat
  icons.rs         file icons and language detection
  settings.rs      settings and session persistence
vendor/
  gpui-base        GPUI Kit base, patched: rainbow indent guides
  gpui-component   GPUI Kit components, patched: tree-sitter and syntect highlighting combined
```

## Credits and licenses

- GPUI Kit (`gpui-base`, `gpui-component`, `gpui-webview`): Apache-2.0, © Longbridge. Vendored copies carry their license and are marked where patched.
- File icons: [vscode-icons](https://github.com/vscode-icons/vscode-icons). Code is MIT, icon artwork is CC BY-SA 4.0 (see `assets/icons/files/`).
- JetBrains Mono: SIL Open Font License 1.1 (`assets/fonts/OFL.txt`).
- Lucide icons (via GPUI Kit): ISC.
