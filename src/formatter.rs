//! Document formatting. Prettier handles the web languages (project-local
//! install first, then global, then `npx`); native formatters handle the rest.

use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const PRETTIER_EXTS: &[&str] = &[
    "js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx", "json", "jsonc", "json5", "css", "scss",
    "less", "html", "htm", "vue", "md", "markdown", "mdx", "yaml", "yml", "graphql", "gql",
    "handlebars", "hbs", "svelte", "astro",
];

fn find_local_prettier(path: &Path) -> Option<std::path::PathBuf> {
    let mut dir = path.parent();
    while let Some(d) = dir {
        let candidate = d.join("node_modules/.bin/prettier");
        if candidate.exists() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

/// Find a tool on the user's login-shell PATH (not the minimal PATH a
/// Finder-launched app gets). Results are cached briefly so per-frame
/// callers don't spawn processes, while newly installed tools still appear.
fn find_tool(bin: &str) -> Option<PathBuf> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Option<PathBuf>)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Ok(map) = cache.lock() {
        if let Some((at, found)) = map.get(bin) {
            if at.elapsed() < Duration::from_secs(30) {
                return found.clone();
            }
        }
    }
    let found = crate::util::login_path()
        .split(':')
        .map(|dir| Path::new(dir).join(bin))
        .find(|p| p.is_file());
    if let Ok(mut map) = cache.lock() {
        map.insert(bin.to_string(), (Instant::now(), found.clone()));
    }
    found
}

/// Returns the formatter program and args that read the document on stdin.
/// Only formatters that are actually installed are returned.
fn formatter_for(path: &Path) -> Option<(String, Vec<String>)> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let file = path.to_string_lossy().to_string();
    let with = |bin: PathBuf, args: Vec<String>| Some((bin.to_string_lossy().to_string(), args));
    if PRETTIER_EXTS.contains(&ext.as_str()) {
        let args = vec!["--stdin-filepath".to_string(), file];
        if let Some(local) = find_local_prettier(path) {
            return with(local, args);
        }
        if let Some(global) = find_tool("prettier") {
            return with(global, args);
        }
        let npx = find_tool("npx")?;
        let mut a = vec!["--yes".to_string(), "prettier".to_string()];
        a.extend(args);
        return with(npx, a);
    }
    let tool = |bin: &str, args: &[&str]| {
        find_tool(bin).map(|p| (p.to_string_lossy().to_string(), args.iter().map(|s| s.to_string()).collect()))
    };
    match ext.as_str() {
        "rs" => tool("rustfmt", &["--emit", "stdout", "--edition", "2024"]),
        "py" | "pyi" => tool("ruff", &["format", "-"])
            .or_else(|| tool("black", &["-q", "-"]))
            .or_else(|| tool("autopep8", &["-"])),
        "go" => tool("gofmt", &[]),
        "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "m" | "mm" | "java" | "cs" => {
            find_tool("clang-format").map(|p| (p.to_string_lossy().to_string(), vec![format!("--assume-filename={file}")]))
        }
        "swift" => tool("swift-format", &[]),
        "zig" => tool("zig", &["fmt", "--stdin"]),
        "dart" => tool("dart", &["format"]),
        "toml" => tool("taplo", &["fmt", "-"]),
        "sh" | "bash" => tool("shfmt", &[]),
        "lua" => tool("stylua", &["-"]),
        "ex" | "exs" => tool("mix", &["format", "-"]),
        _ => None,
    }
}

/// What to install to format this kind of file, when nothing is installed.
pub fn missing_hint(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "py" | "pyi" => "Install a Python formatter: pip install ruff (or black).",
        "rs" => "Install rustfmt: rustup component add rustfmt",
        "go" => "Install Go to get gofmt.",
        "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "m" | "mm" | "java" | "cs" => "Install clang-format: brew install clang-format",
        "swift" => "Install swift-format: brew install swift-format",
        "toml" => "Install taplo: brew install taplo",
        "sh" | "bash" => "Install shfmt: brew install shfmt",
        "lua" => "Install stylua: brew install stylua",
        e if PRETTIER_EXTS.contains(&e) => "Install Node.js to format with Prettier.",
        _ => return None,
    })
}

pub fn can_format(path: &Path) -> bool {
    formatter_for(path).is_some()
}

pub fn formatter_name(path: &Path) -> Option<String> {
    formatter_for(path).map(|(bin, _)| {
        let name = Path::new(&bin)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(bin);
        match name.as_str() {
            "npx" | "prettier" => "Prettier".to_string(),
            "ruff" => "Ruff".to_string(),
            "black" => "Black".to_string(),
            other => other.to_string(),
        }
    })
}

/// Format `text` as the file at `path`. Blocking: call from a background task.
pub fn format(path: &Path, text: &str) -> Result<String, String> {
    let (bin, args) = formatter_for(path).ok_or("No formatter for this file type")?;
    let mut child = Command::new(&bin)
        .args(&args)
        .current_dir(path.parent().unwrap_or(Path::new(".")))
        .env("PATH", crate::util::login_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{bin} not found ({e}). Install it to format this file."))?;
    child
        .stdin
        .take()
        .ok_or("stdin unavailable")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err.lines().take(4).collect::<Vec<_>>().join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_installed_formatters_are_offered() {
        // Tools are looked up on the login PATH; git is always there on a dev Mac.
        assert!(find_tool("git").is_some());
        assert!(find_tool("definitely-not-a-real-formatter").is_none());
        let py = Path::new("/tmp/x.py");
        let installed = ["ruff", "black", "autopep8"].iter().any(|t| find_tool(t).is_some());
        assert_eq!(can_format(py), installed);
        assert!(missing_hint(py).is_some());
        // Unknown types have neither a formatter nor a hint.
        assert!(!can_format(Path::new("/tmp/x.unknownext")));
        assert!(missing_hint(Path::new("/tmp/x.unknownext")).is_none());
    }
}
