//! Document formatting. Prettier handles the web languages (project-local
//! install first, then global, then `npx`); native formatters handle the rest.

use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
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

fn which(bin: &str) -> bool {
    Command::new("/usr/bin/env")
        .args(["which", bin])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Returns the formatter program and args that read the document on stdin.
fn formatter_for(path: &Path) -> Option<(String, Vec<String>)> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let file = path.to_string_lossy().to_string();
    if PRETTIER_EXTS.contains(&ext.as_str()) {
        if let Some(local) = find_local_prettier(path) {
            return Some((
                local.to_string_lossy().into(),
                vec!["--stdin-filepath".into(), file],
            ));
        }
        if which("prettier") {
            return Some(("prettier".into(), vec!["--stdin-filepath".into(), file]));
        }
        return Some((
            "npx".into(),
            vec![
                "--yes".into(),
                "prettier".into(),
                "--stdin-filepath".into(),
                file,
            ],
        ));
    }
    let tool = |bin: &str, args: &[&str]| {
        Some((bin.to_string(), args.iter().map(|s| s.to_string()).collect()))
    };
    match ext.as_str() {
        "rs" => tool("rustfmt", &["--emit", "stdout", "--edition", "2024"]),
        "py" => {
            if which("ruff") {
                tool("ruff", &["format", "-"])
            } else {
                tool("black", &["-q", "-"])
            }
        }
        "go" => tool("gofmt", &[]),
        "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "m" | "mm" | "java" | "cs" => {
            Some(("clang-format".into(), vec![format!("--assume-filename={file}")]))
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

pub fn can_format(path: &Path) -> bool {
    formatter_for(path).is_some()
}

pub fn formatter_name(path: &Path) -> Option<String> {
    formatter_for(path).map(|(bin, args)| {
        if bin == "npx" || bin.ends_with("prettier") {
            "Prettier".to_string()
        } else {
            args.first()
                .filter(|_| bin == "ruff")
                .map(|_| "Ruff".to_string())
                .unwrap_or(bin)
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
