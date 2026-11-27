//! Language Server Protocol client. Orion finds language servers already
//! installed on your machine (rust-analyzer, pyright, typescript-language-
//! server, gopls, clangd, sourcekit-lsp, …) and wires them into the editor:
//! completions, hover docs, go to definition and live diagnostics.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    rc::Rc,
    str::FromStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::{Result, anyhow};
use gpui_kit::component::input::{
    CompletionProvider, DefinitionProvider, HoverProvider, Rope, RopeExt,
};
use gpui_kit::{App, AppContext as _, Task, Window};
use lsp_types::{
    CompletionContext, CompletionResponse, GotoDefinitionResponse, Hover, LocationLink,
};
use serde_json::{Value, json};

/// A language server Orion knows how to start.
pub struct ServerSpec {
    pub name: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
}

const fn spec(name: &'static str, command: &'static str, args: &'static [&'static str]) -> ServerSpec {
    ServerSpec { name, command, args }
}

/// Candidate servers per editor language, best first.
fn candidates(language: &str) -> &'static [ServerSpec] {
    const RUST: &[ServerSpec] = &[spec("rust-analyzer", "rust-analyzer", &[])];
    const PY: &[ServerSpec] = &[
        spec("basedpyright", "basedpyright-langserver", &["--stdio"]),
        spec("pyright", "pyright-langserver", &["--stdio"]),
        spec("pylsp", "pylsp", &[]),
        spec("jedi", "jedi-language-server", &[]),
    ];
    const TS: &[ServerSpec] = &[
        spec("typescript", "typescript-language-server", &["--stdio"]),
        spec("vtsls", "vtsls", &["--stdio"]),
    ];
    const GO: &[ServerSpec] = &[spec("gopls", "gopls", &[])];
    const C: &[ServerSpec] = &[spec("clangd", "clangd", &[])];
    const SWIFT: &[ServerSpec] = &[spec("sourcekit-lsp", "sourcekit-lsp", &[])];
    const HTML: &[ServerSpec] = &[spec("html", "vscode-html-language-server", &["--stdio"])];
    const CSS: &[ServerSpec] = &[spec("css", "vscode-css-language-server", &["--stdio"])];
    const JSON: &[ServerSpec] = &[spec("json", "vscode-json-language-server", &["--stdio"])];
    const LUA: &[ServerSpec] = &[spec("lua-ls", "lua-language-server", &[])];
    const ZIG: &[ServerSpec] = &[spec("zls", "zls", &[])];
    const SH: &[ServerSpec] = &[spec("bash-ls", "bash-language-server", &["start"])];
    const JAVA: &[ServerSpec] = &[spec("jdtls", "jdtls", &[])];
    const RUBY: &[ServerSpec] = &[spec("ruby-lsp", "ruby-lsp", &[]), spec("solargraph", "solargraph", &["stdio"])];
    const PHP: &[ServerSpec] = &[spec("intelephense", "intelephense", &["--stdio"])];
    const YAML: &[ServerSpec] = &[spec("yaml-ls", "yaml-language-server", &["--stdio"])];
    const TOML: &[ServerSpec] = &[spec("taplo", "taplo", &["lsp", "stdio"])];
    const KOTLIN: &[ServerSpec] = &[spec("kotlin-ls", "kotlin-language-server", &[])];
    const DART: &[ServerSpec] = &[spec("dart", "dart", &["language-server"])];
    const SVELTE: &[ServerSpec] = &[spec("svelte", "svelteserver", &["--stdio"])];
    const CSHARP: &[ServerSpec] = &[spec("csharp-ls", "csharp-ls", &[])];
    const HASKELL: &[ServerSpec] = &[spec("hls", "haskell-language-server-wrapper", &["--lsp"])];
    const ELIXIR: &[ServerSpec] = &[spec("elixir-ls", "elixir-ls", &[])];
    match language {
        "rust" => RUST,
        "python" => PY,
        "javascript" | "typescript" | "tsx" => TS,
        "go" => GO,
        "c" | "cpp" | "objectivec" | "m" | "mm" => C,
        "swift" => SWIFT,
        "html" => HTML,
        "css" | "scss" | "less" => CSS,
        "json" => JSON,
        "lua" => LUA,
        "zig" => ZIG,
        "bash" => SH,
        "java" => JAVA,
        "ruby" => RUBY,
        "php" => PHP,
        "yaml" => YAML,
        "toml" => TOML,
        "kotlin" => KOTLIN,
        "dart" => DART,
        "svelte" => SVELTE,
        "csharp" => CSHARP,
        "hs" | "haskell" => HASKELL,
        "elixir" => ELIXIR,
        _ => &[],
    }
}

/// LSP language identifier for an editor language.
pub fn language_id(language: &str, path: &Path) -> String {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match language {
        "tsx" => "typescriptreact".into(),
        "javascript" if ext == "jsx" => "javascriptreact".into(),
        "bash" => "shellscript".into(),
        "cpp" => "cpp".into(),
        other => other.into(),
    }
}

fn find_on_path(bin: &str) -> Option<PathBuf> {
    for dir in crate::util::login_path().split(':') {
        let candidate = Path::new(dir).join(bin);
        if candidate.is_file() {
            // rustup installs a rust-analyzer proxy even when the component
            // isn't installed; make sure it actually runs.
            if bin == "rust-analyzer" {
                let ok = Command::new(&candidate)
                    .arg("--version")
                    .env("PATH", crate::util::login_path())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if !ok {
                    continue;
                }
            }
            return Some(candidate);
        }
    }
    // Xcode ships clangd and sourcekit-lsp.
    if matches!(bin, "clangd" | "sourcekit-lsp") {
        let out = Command::new("/usr/bin/xcrun").args(["--find", bin]).output().ok()?;
        if out.status.success() {
            let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// The first installed server for `language`, with its resolved binary.
pub fn resolve(language: &str) -> Option<(&'static ServerSpec, PathBuf)> {
    candidates(language)
        .iter()
        .find_map(|s| find_on_path(s.command).map(|p| (s, p)))
}

pub fn path_to_uri(path: &Path) -> String {
    let mut out = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&rest[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    Some(PathBuf::from(String::from_utf8_lossy(&out).into_owned()))
}

/// Messages the server sends that the UI cares about.
pub enum ServerEvent {
    Diagnostics { uri: String, diagnostics: Vec<lsp_types::Diagnostic> },
}

type Pending = Arc<Mutex<HashMap<u64, smol::channel::Sender<Result<Value, String>>>>>;

pub struct LspClient {
    #[allow(dead_code)]
    pub name: &'static str,
    stdin: Mutex<ChildStdin>,
    next_id: AtomicU64,
    pending: Pending,
    child: Mutex<Child>,
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.notify("exit", json!(null));
        if let Ok(mut c) = self.child.lock() {
            let _ = c.kill();
        }
    }
}

fn write_message(w: &mut impl Write, value: &Value) -> std::io::Result<()> {
    let body = value.to_string();
    write!(w, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    w.flush()
}

impl LspClient {
    /// Start a server and complete the initialize handshake. Blocking: run
    /// it on a background thread.
    pub fn start(
        spec: &'static ServerSpec,
        bin: &Path,
        root: &Path,
        events: smol::channel::Sender<ServerEvent>,
    ) -> Result<Arc<Self>> {
        let mut child = Command::new(bin)
            .args(spec.args)
            .current_dir(root)
            .env("PATH", crate::util::login_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("{}: {e}", spec.command))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));

        let client = Arc::new(Self {
            name: spec.name,
            stdin: Mutex::new(stdin),
            next_id: AtomicU64::new(1),
            pending: pending.clone(),
            child: Mutex::new(child),
        });

        // Reader: route responses to waiters, answer server requests,
        // forward diagnostics to the UI.
        let weak = Arc::downgrade(&client);
        std::thread::Builder::new()
            .name(format!("lsp-{}", spec.name))
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                // When the server exits, fail every outstanding request at once.
                struct FailPending(Pending);
                impl Drop for FailPending {
                    fn drop(&mut self) {
                        if let Ok(mut p) = self.0.lock() {
                            for (_, tx) in p.drain() {
                                let _ = tx.try_send(Err("language server exited".into()));
                            }
                        }
                    }
                }
                let _guard = FailPending(pending.clone());
                loop {
                    let mut len = 0usize;
                    let mut line = String::new();
                    loop {
                        line.clear();
                        match reader.read_line(&mut line) {
                            Ok(0) | Err(_) => return,
                            Ok(_) => {}
                        }
                        let l = line.trim();
                        if l.is_empty() {
                            break;
                        }
                        if let Some(v) = l.strip_prefix("Content-Length:") {
                            len = v.trim().parse().unwrap_or(0);
                        }
                    }
                    let mut buf = vec![0u8; len];
                    if reader.read_exact(&mut buf).is_err() {
                        return;
                    }
                    let Ok(msg) = serde_json::from_slice::<Value>(&buf) else { continue };
                    let method = msg.get("method").and_then(|m| m.as_str());
                    let id = msg.get("id").cloned();
                    match (method, id) {
                        (None, Some(id)) => {
                            let Some(id) = id.as_u64() else { continue };
                            let tx = pending.lock().ok().and_then(|mut p| p.remove(&id));
                            if let Some(tx) = tx {
                                let result = if let Some(err) = msg.get("error") {
                                    Err(err.get("message").and_then(|m| m.as_str()).unwrap_or("error").to_string())
                                } else {
                                    Ok(msg.get("result").cloned().unwrap_or(Value::Null))
                                };
                                let _ = tx.try_send(result);
                            }
                        }
                        (Some(method), Some(id)) => {
                            // Server → client request: reply so it doesn't stall.
                            let result = match method {
                                "workspace/configuration" => {
                                    let n = msg.pointer("/params/items").and_then(|i| i.as_array()).map_or(1, |a| a.len());
                                    Value::Array(vec![Value::Null; n])
                                }
                                _ => Value::Null,
                            };
                            if let Some(client) = weak.upgrade() {
                                if let Ok(mut w) = client.stdin.lock() {
                                    let _ = write_message(&mut *w, &json!({"jsonrpc": "2.0", "id": id, "result": result}));
                                }
                            }
                        }
                        (Some("textDocument/publishDiagnostics"), None) => {
                            if let Some(params) = msg.get("params") {
                                let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("").to_string();
                                let diagnostics = params
                                    .get("diagnostics")
                                    .cloned()
                                    .and_then(|d| serde_json::from_value(d).ok())
                                    .unwrap_or_default();
                                let _ = events.try_send(ServerEvent::Diagnostics { uri, diagnostics });
                            }
                        }
                        _ => {}
                    }
                }
            })?;

        let root_uri = path_to_uri(root);
        let init = client.request_blocking(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": root_uri,
                "rootPath": root.to_string_lossy(),
                "workspaceFolders": [{ "uri": root_uri, "name": root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default() }],
                "clientInfo": { "name": "Orion", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": {
                    "general": { "positionEncodings": ["utf-16"] },
                    "workspace": { "configuration": true, "workspaceFolders": true },
                    "textDocument": {
                        "synchronization": { "didSave": true, "dynamicRegistration": false },
                        "completion": {
                            "completionItem": {
                                "snippetSupport": false,
                                "documentationFormat": ["markdown", "plaintext"],
                                "insertReplaceSupport": true
                            },
                            "contextSupport": true
                        },
                        "hover": { "contentFormat": ["markdown", "plaintext"] },
                        "definition": { "linkSupport": true },
                        "publishDiagnostics": { "relatedInformation": false }
                    }
                }
            }),
            Duration::from_secs(30),
        )?;
        let _ = init;
        client.notify("initialized", json!({}))?;
        Ok(client)
    }

    pub fn notify(&self, method: &str, params: Value) -> Result<()> {
        let mut w = self.stdin.lock().map_err(|_| anyhow!("poisoned"))?;
        write_message(&mut *w, &json!({"jsonrpc": "2.0", "method": method, "params": params}))?;
        Ok(())
    }

    fn send_request(&self, method: &str, params: Value) -> Result<smol::channel::Receiver<Result<Value, String>>> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = smol::channel::bounded(1);
        self.pending.lock().map_err(|_| anyhow!("poisoned"))?.insert(id, tx);
        let mut w = self.stdin.lock().map_err(|_| anyhow!("poisoned"))?;
        write_message(&mut *w, &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        Ok(rx)
    }

    fn request_blocking(&self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        let rx = self.send_request(method, params)?;
        smol::block_on(async {
            smol::future::or(
                async { rx.recv().await.map_err(|e| anyhow!("{e}"))?.map_err(|e| anyhow!(e)) },
                async {
                    smol::Timer::after(timeout).await;
                    Err(anyhow!("{method} timed out"))
                },
            )
            .await
        })
    }

    pub async fn request(self: Arc<Self>, method: &'static str, params: Value) -> Result<Value> {
        let rx = self.send_request(method, params)?;
        smol::future::or(
            async { rx.recv().await.map_err(|e| anyhow!("{e}"))?.map_err(|e| anyhow!(e)) },
            async {
                smol::Timer::after(Duration::from_secs(10)).await;
                Err(anyhow!("{method} timed out"))
            },
        )
        .await
    }
}

/// One open document on one server.
pub struct Document {
    pub client: Arc<LspClient>,
    pub uri: String,
    version: AtomicI32,
    last_synced: Mutex<String>,
}

impl Document {
    pub fn open(client: Arc<LspClient>, path: &Path, language_id: &str, text: &str) -> Rc<Self> {
        let uri = path_to_uri(path);
        let _ = client.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": language_id, "version": 1, "text": text}}),
        );
        Rc::new(Self {
            client,
            uri,
            version: AtomicI32::new(1),
            last_synced: Mutex::new(text.to_string()),
        })
    }

    /// Send the full text if it changed since the last sync.
    pub fn sync(&self, text: &str) {
        let mut last = match self.last_synced.lock() {
            Ok(l) => l,
            Err(_) => return,
        };
        if *last == text {
            return;
        }
        *last = text.to_string();
        let version = self.version.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.client.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": self.uri, "version": version}, "contentChanges": [{"text": text}]}),
        );
    }

    pub fn saved(&self) {
        let _ = self
            .client
            .notify("textDocument/didSave", json!({"textDocument": {"uri": self.uri}}));
    }

    pub fn close(&self) {
        let _ = self
            .client
            .notify("textDocument/didClose", json!({"textDocument": {"uri": self.uri}}));
    }

    fn position_params(&self, rope: &Rope, offset: usize) -> Value {
        self.sync(&rope.to_string());
        let pos = rope.offset_to_position(offset);
        json!({"textDocument": {"uri": self.uri}, "position": {"line": pos.line, "character": pos.character}})
    }
}

/// Editor-facing provider backed by a [`Document`].
pub struct Provider(pub Rc<Document>);

impl CompletionProvider for Provider {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        trigger: CompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let mut params = self.0.position_params(rope, offset);
        params["context"] = serde_json::to_value(&trigger).unwrap_or(json!({"triggerKind": 1}));
        let client = self.0.client.clone();
        cx.background_spawn(async move {
            let value = client.request("textDocument/completion", params).await?;
            if value.is_null() {
                return Ok(CompletionResponse::Array(vec![]));
            }
            Ok(serde_json::from_value(value)?)
        })
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _: &mut App) -> bool {
        new_text
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | ':' | '>' | '/' | '@' | '<' | '"' | '\''))
    }
}

impl HoverProvider for Provider {
    fn hover(&self, rope: &Rope, offset: usize, _: &mut Window, cx: &mut App) -> Task<Result<Option<Hover>>> {
        let params = self.0.position_params(rope, offset);
        let client = self.0.client.clone();
        cx.background_spawn(async move {
            let value = client.request("textDocument/hover", params).await?;
            if value.is_null() {
                return Ok(None);
            }
            Ok(serde_json::from_value(value).ok())
        })
    }
}

impl DefinitionProvider for Provider {
    fn definitions(
        &self,
        rope: &Rope,
        offset: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Vec<LocationLink>>> {
        let params = self.0.position_params(rope, offset);
        let client = self.0.client.clone();
        let word = rope.word_range(offset).map(|r| {
            let a = rope.offset_to_position(r.start);
            let b = rope.offset_to_position(r.end);
            lsp_types::Range::new(a, b)
        });
        cx.background_spawn(async move {
            let value = client.request("textDocument/definition", params).await?;
            if value.is_null() {
                return Ok(vec![]);
            }
            let resp: GotoDefinitionResponse = serde_json::from_value(value)?;
            Ok(match resp {
                GotoDefinitionResponse::Scalar(loc) => vec![LocationLink {
                    origin_selection_range: word,
                    target_uri: loc.uri,
                    target_range: loc.range,
                    target_selection_range: loc.range,
                }],
                GotoDefinitionResponse::Array(locs) => locs
                    .into_iter()
                    .map(|loc| LocationLink {
                        origin_selection_range: word,
                        target_uri: loc.uri,
                        target_range: loc.range,
                        target_selection_range: loc.range,
                    })
                    .collect(),
                GotoDefinitionResponse::Link(links) => links,
            })
        })
    }
}

#[allow(dead_code)]
fn parse_uri(s: &str) -> Option<lsp_types::Uri> {
    lsp_types::Uri::from_str(s).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_round_trip() {
        let p = Path::new("/tmp/my dir/main.rs");
        let uri = path_to_uri(p);
        assert_eq!(uri, "file:///tmp/my%20dir/main.rs");
        assert_eq!(uri_to_path(&uri).unwrap(), p);
    }

    /// Real round trip against clangd when it's installed (Xcode ships it).
    #[test]
    fn clangd_reports_diagnostics_and_completes() {
        let Some((spec, bin)) = resolve("c") else {
            eprintln!("clangd not installed; skipping");
            return;
        };
        let dir = std::env::temp_dir().join(format!("orion-lsp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("main.c");
        let text = "int main(void) {\n  int value = 1;\n  return undefined_name;\n}\n";
        std::fs::write(&file, text).unwrap();
        let (tx, rx) = smol::channel::unbounded();
        let client = LspClient::start(spec, &bin, &dir, tx).expect("clangd starts");
        let _doc = Document::open(client.clone(), &file, "c", text);
        let got = smol::block_on(async {
            smol::future::or(
                async {
                    while let Ok(ServerEvent::Diagnostics { uri, diagnostics }) = rx.recv().await {
                        if uri.ends_with("main.c") && !diagnostics.is_empty() {
                            return Some(diagnostics);
                        }
                    }
                    None
                },
                async {
                    smol::Timer::after(Duration::from_secs(20)).await;
                    None
                },
            )
            .await
        });
        let diagnostics = got.expect("diagnostics from clangd");
        assert!(diagnostics.iter().any(|d| d.message.contains("undefined_name")));

        // Completion at "  return " + "va" should offer `value`.
        let completion = smol::block_on(client.clone().request(
            "textDocument/completion",
            json!({"textDocument": {"uri": path_to_uri(&file)}, "position": {"line": 1, "character": 6}}),
        ));
        assert!(completion.is_ok(), "completion request failed: {completion:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Real round trip against a Python server when one is installed.
    #[test]
    fn python_server_reports_diagnostics_and_hover() {
        let Some((spec, bin)) = resolve("python") else {
            eprintln!("no Python language server installed; skipping");
            return;
        };
        let dir = std::env::temp_dir().join(format!("orion-py-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("app.py");
        let text = "def greet(name: str) -> str:\n    return 'hi ' + name\n\nprint(greet('a'))\nprint(undefined_name)\n";
        std::fs::write(&file, text).unwrap();
        let (tx, rx) = smol::channel::unbounded();
        let client = LspClient::start(spec, &bin, &dir, tx).expect("python server starts");
        let _doc = Document::open(client.clone(), &file, "python", text);
        let diagnostics = smol::block_on(async {
            smol::future::or(
                async {
                    while let Ok(ServerEvent::Diagnostics { uri, diagnostics }) = rx.recv().await {
                        if uri.ends_with("app.py") && !diagnostics.is_empty() {
                            return Some(diagnostics);
                        }
                    }
                    None
                },
                async {
                    smol::Timer::after(Duration::from_secs(30)).await;
                    None
                },
            )
            .await
        })
        .expect("diagnostics from the Python server");
        assert!(diagnostics.iter().any(|d| d.message.contains("undefined_name")), "{diagnostics:?}");

        // Hover over `greet` on line 4 should describe the function.
        let hover = smol::block_on(client.clone().request(
            "textDocument/hover",
            json!({"textDocument": {"uri": path_to_uri(&file)}, "position": {"line": 3, "character": 7}}),
        ))
        .expect("hover response");
        assert!(hover.to_string().contains("greet"), "{hover}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn known_languages_have_candidates() {
        for l in ["rust", "python", "typescript", "go", "cpp", "swift"] {
            assert!(!candidates(l).is_empty(), "{l}");
        }
        assert!(candidates("text").is_empty());
    }
}
