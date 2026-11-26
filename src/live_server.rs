//! Live Server: serves the open folder over HTTP and reloads pages when files
//! change. HTML responses get a tiny long-poll script injected; any file
//! change in the folder bumps a version counter that wakes those polls.

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use notify::{RecursiveMode, Watcher};
use tiny_http::{Header, Response, Server, StatusCode};

const RELOAD_SCRIPT: &str = r#"<script>(function(){var v=null;function poll(){fetch('/__ide_live?v='+(v===null?'':v)).then(function(r){return r.text()}).then(function(t){if(v!==null&&t!==String(v)){location.reload();return}v=t;poll()}).catch(function(){setTimeout(poll,1000)})}poll()})();</script>"#;

pub struct LiveServer {
    pub port: u16,
    pub root: PathBuf,
    stop: Arc<AtomicBool>,
    server: Arc<Server>,
    _watcher: notify::RecommendedWatcher,
}

impl Drop for LiveServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.server.unblock();
    }
}

struct Version {
    value: Mutex<u64>,
    changed: Condvar,
}

impl LiveServer {
    pub fn start(root: PathBuf, preferred_port: u16) -> anyhow::Result<Self> {
        let mut last_err = None;
        let mut bound = None;
        for port in preferred_port..preferred_port.saturating_add(20) {
            match Server::http(("127.0.0.1", port)) {
                Ok(server) => {
                    bound = Some((server, port));
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let (server, port) = bound.ok_or_else(|| {
            anyhow::anyhow!("no free port near {preferred_port}: {last_err:?}")
        })?;
        let server = Arc::new(server);
        let stop = Arc::new(AtomicBool::new(false));
        let version = Arc::new(Version {
            value: Mutex::new(0),
            changed: Condvar::new(),
        });

        let watch_version = version.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let relevant = event.paths.iter().any(|p| {
                    !p.components().any(|c| {
                        matches!(c.as_os_str().to_str(), Some(".git" | "node_modules" | "target"))
                    })
                });
                if relevant && (event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove()) {
                    let mut v = watch_version.value.lock().unwrap();
                    *v += 1;
                    watch_version.changed.notify_all();
                }
            }
        })?;
        watcher.watch(&root, RecursiveMode::Recursive)?;

        let accept_server = server.clone();
        let accept_stop = stop.clone();
        let serve_root = root.clone();
        thread::Builder::new()
            .name("live-server".into())
            .spawn(move || {
                for request in accept_server.incoming_requests() {
                    if accept_stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let root = serve_root.clone();
                    let version = version.clone();
                    let stop = accept_stop.clone();
                    thread::spawn(move || handle(request, &root, &version, &stop));
                }
            })?;

        Ok(Self {
            port,
            root,
            stop,
            server,
            _watcher: watcher,
        })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    /// URL for a file inside the served folder.
    pub fn url_for(&self, file: &Path) -> String {
        match file.strip_prefix(&self.root) {
            Ok(rel) => {
                let rel = rel
                    .components()
                    .map(|c| percent_encode(&c.as_os_str().to_string_lossy()))
                    .collect::<Vec<_>>()
                    .join("/");
                format!("{}{}", self.url(), rel)
            }
            Err(_) => self.url(),
        }
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "txt" | "md" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap()
}

fn handle(request: tiny_http::Request, root: &Path, version: &Version, stop: &AtomicBool) {
    let url = request.url().to_string();
    let (path_part, query) = url.split_once('?').unwrap_or((&url, ""));

    if path_part == "/__ide_live" {
        let known = query
            .strip_prefix("v=")
            .and_then(|v| v.parse::<u64>().ok());
        let mut v = version.value.lock().unwrap();
        if let Some(known) = known {
            let deadline = std::time::Instant::now() + Duration::from_secs(25);
            while *v == known && !stop.load(Ordering::SeqCst) {
                let now = std::time::Instant::now();
                if now >= deadline {
                    break;
                }
                let (guard, _) = version.changed.wait_timeout(v, deadline - now).unwrap();
                v = guard;
            }
        }
        let body = v.to_string();
        drop(v);
        let _ = request.respond(
            Response::from_string(body).with_header(header("Cache-Control", "no-store")),
        );
        return;
    }

    let rel = percent_decode(path_part.trim_start_matches('/'));
    if rel.split('/').any(|seg| seg == "..") {
        let _ = request.respond(Response::from_string("Forbidden").with_status_code(403));
        return;
    }
    let mut file = root.join(&rel);
    if file.is_dir() {
        if !path_part.ends_with('/') && !path_part.is_empty() && path_part != "/" {
            let location = format!("{path_part}/");
            let _ = request.respond(
                Response::empty(StatusCode(301)).with_header(header("Location", &location)),
            );
            return;
        }
        let index = file.join("index.html");
        if index.exists() {
            file = index;
        } else {
            let listing = directory_listing(&file, path_part);
            let body = inject(listing);
            let _ = request.respond(
                Response::from_string(body)
                    .with_header(header("Content-Type", "text/html; charset=utf-8")),
            );
            return;
        }
    }
    match std::fs::read(&file) {
        Ok(bytes) => {
            let content_type = mime(&file);
            let data = if content_type.starts_with("text/html") {
                inject(String::from_utf8_lossy(&bytes).into_owned()).into_bytes()
            } else {
                bytes
            };
            let len = data.len();
            let response = Response::new(
                StatusCode(200),
                vec![
                    header("Content-Type", content_type),
                    header("Cache-Control", "no-store"),
                ],
                Cursor::new(data),
                Some(len),
                None,
            );
            let _ = request.respond(response);
        }
        Err(_) => {
            let body = inject(format!(
                "<!doctype html><meta charset=utf-8><title>Not found</title>\
                 <body style=\"font:14px -apple-system,sans-serif;padding:40px;color:#555\">\
                 <h2>404</h2><p>{}</p></body>",
                html_escape(&rel)
            ));
            let _ = request.respond(
                Response::from_string(body)
                    .with_status_code(404)
                    .with_header(header("Content-Type", "text/html; charset=utf-8")),
            );
        }
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn inject(mut html: String) -> String {
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, RELOAD_SCRIPT);
    } else {
        html.push_str(RELOAD_SCRIPT);
    }
    html
}

fn directory_listing(dir: &Path, url_path: &str) -> String {
    let mut entries: Vec<(bool, String)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| (e.path().is_dir(), e.file_name().to_string_lossy().to_string()))
                .filter(|(_, n)| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    let mut out = format!(
        "<!doctype html><meta charset=utf-8><title>{0}</title>\
         <body style=\"font:14px -apple-system,sans-serif;padding:32px 40px;color:#333\">\
         <h3 style=\"font-weight:600\">{0}</h3><ul style=\"list-style:none;padding:0;line-height:1.9\">",
        html_escape(url_path)
    );
    for (is_dir, name) in entries {
        let slash = if is_dir { "/" } else { "" };
        out.push_str(&format!(
            "<li><a style=\"color:#4a6cf7;text-decoration:none\" href=\"{0}{1}\">{2}{1}</a></li>",
            percent_encode(&name),
            slash,
            html_escape(&name)
        ));
    }
    out.push_str("</ul></body>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn get(port: u16, path: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn serves_injects_and_reloads() {
        let dir = std::env::temp_dir().join(format!("ide-live-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub dir")).unwrap();
        std::fs::write(dir.join("index.html"), "<html><body>hi</body></html>").unwrap();
        std::fs::write(dir.join("sub dir/app.css"), "body{}").unwrap();
        let server = LiveServer::start(dir.clone(), 18500).unwrap();
        let port = server.port;

        let index = get(port, "/");
        assert!(index.contains("200 OK") && index.contains("hi") && index.contains("__ide_live"));
        let css = get(port, "/sub%20dir/app.css");
        assert!(css.contains("text/css") && !css.contains("__ide_live"));
        assert!(get(port, "/../etc/passwd").contains("403") || get(port, "/../etc/passwd").contains("404"));
        assert!(get(port, "/missing.html").contains("404"));
        assert_eq!(server.url_for(&dir.join("sub dir/app.css")), format!("http://127.0.0.1:{port}/sub%20dir/app.css"));

        let v0 = get(port, "/__ide_live?v=").rsplit("\r\n").next().unwrap().to_string();
        let waiter = thread::spawn(move || get(port, &format!("/__ide_live?v={v0}")));
        thread::sleep(Duration::from_millis(400));
        std::fs::write(dir.join("index.html"), "<html><body>changed</body></html>").unwrap();
        let woke = waiter.join().unwrap();
        assert!(woke.contains("200 OK"));
        drop(server);
        let _ = std::fs::remove_dir_all(dir);
    }
}
