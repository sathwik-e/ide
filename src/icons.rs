//! File and folder icons (vscode-icons artwork) and language detection.

use std::path::Path;

const ICON_DIR: &str = "icons/files/";

fn file_icon_name(name: &str, ext: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "cargo.toml" | "cargo.lock" => return "cargo",
        "package.json" | "package-lock.json" | ".npmrc" => return "npm",
        "yarn.lock" | ".yarnrc" => return "yarn",
        "pnpm-lock.yaml" | "pnpm-workspace.yaml" => return "pnpm",
        "bun.lockb" | "bun.lock" | "bunfig.toml" => return "bun",
        "dockerfile" | "docker-compose.yml" | "docker-compose.yaml" | "compose.yaml"
        | ".dockerignore" => return "docker",
        ".gitignore" | ".gitattributes" | ".gitmodules" | ".gitkeep" => return "git",
        "makefile" | "gnumakefile" => return "config",
        "cmakelists.txt" => return "cmake",
        "license" | "license.md" | "license.txt" | "licence" => return "license",
        ".editorconfig" => return "editorconfig",
        ".prettierrc" | ".prettierrc.json" | ".prettierignore" | "prettier.config.js" => {
            return "prettier";
        }
        ".eslintrc" | ".eslintrc.json" | ".eslintrc.js" | "eslint.config.js"
        | "eslint.config.mjs" => return "eslint",
        ".babelrc" | "babel.config.js" => return "babel",
        "tsconfig.json" => return "tsconfig",
        "jsconfig.json" => return "jsconfig",
        "deno.json" | "deno.jsonc" => return "deno",
        "build.gradle" | "settings.gradle" | "build.gradle.kts" => return "gradle",
        "pom.xml" => return "maven",
        "nginx.conf" => return "nginx",
        ".htaccess" => return "apache",
        _ => {}
    }
    if lower.starts_with(".env") {
        return "dotenv";
    }
    if lower.starts_with("vite.config") {
        return "vite";
    }
    if lower.starts_with("webpack.config") {
        return "webpack";
    }
    if lower.starts_with("tailwind.config") {
        return "tailwind";
    }
    if lower.starts_with("next.config") {
        return "next";
    }
    if lower.starts_with("nuxt.config") {
        return "nuxt";
    }
    if lower.starts_with("angular.json") {
        return "angular";
    }
    match ext {
        "rs" => "rust",
        "py" | "pyw" | "pyi" => "python",
        "js" | "mjs" | "cjs" => "js",
        "ts" | "mts" | "cts" => "typescript",
        "jsx" => "reactjs",
        "tsx" => "reactts",
        "html" | "htm" | "xhtml" => "html",
        "css" => "css",
        "scss" => "scss",
        "sass" => "sass",
        "less" => "less",
        "json" | "jsonc" | "json5" => "json",
        "yml" | "yaml" => "yaml",
        "toml" => "toml",
        "xml" | "plist" | "xaml" => "xml",
        "md" | "markdown" => "markdown",
        "mdx" => "mdx",
        "go" => "go",
        "java" | "class" | "jar" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "cs" | "csx" => "csharp",
        "php" => "php",
        "rb" | "erb" | "gemspec" => "ruby",
        "lua" => "lua",
        "sh" | "bash" | "zsh" | "fish" => "shell",
        "ps1" | "psm1" => "powershell",
        "bat" | "cmd" => "bat",
        "sql" => "sql",
        "vue" => "vue",
        "svelte" => "svelte",
        "astro" => "astro",
        "dart" => "dartlang",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "hs" | "lhs" => "haskell",
        "scala" | "sc" => "scala",
        "r" => "r",
        "jl" => "julia",
        "pl" | "pm" => "perl",
        "zig" => "zig",
        "nim" => "nim",
        "ml" | "mli" => "ocaml",
        "clj" | "cljs" | "edn" => "clojure",
        "fs" | "fsx" => "fsharp",
        "vb" => "vb",
        "graphql" | "gql" => "graphql",
        "prisma" => "prisma",
        "tf" | "tfvars" | "hcl" => "terraform",
        "nix" => "nix",
        "cmake" => "cmake",
        "gradle" => "gradle",
        "txt" | "text" => "text",
        "log" => "log",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tiff" | "avif" => "image",
        "svg" => "svg",
        "pdf" => "pdf",
        "zip" | "tar" | "gz" | "tgz" | "rar" | "7z" | "xz" | "bz2" => "zip",
        "ttf" | "otf" | "woff" | "woff2" => "font",
        "mp4" | "mov" | "avi" | "mkv" | "webm" => "video",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "audio",
        "key" | "pem" | "pub" => "key",
        "crt" | "cer" => "cert",
        "bin" | "exe" | "dll" | "so" | "dylib" | "o" | "a" => "binary",
        "ipynb" => "jupyter",
        "ini" | "cfg" | "conf" | "properties" => "ini",
        "csv" | "tsv" => "csv",
        "m" => "objectivec",
        "mm" => "objectivecpp",
        "f" | "f90" | "f95" | "for" => "fortran",
        "cob" | "cbl" => "cobol",
        "asm" | "s" => "assembly",
        "elm" => "elm",
        "purs" => "purescript",
        "cr" => "crystal",
        "v" => "vlang",
        "sol" => "solidity",
        "wasm" | "wat" => "wasm",
        "proto" => "protobuf",
        "groovy" => "groovy",
        "tex" | "sty" | "bib" => "tex",
        "diff" => "diff",
        "patch" => "patch",
        "http" | "rest" => "http",
        "cshtml" | "razor" => "razor",
        "hbs" | "handlebars" => "handlebars",
        "pug" | "jade" => "pug",
        "ejs" => "ejs",
        "liquid" => "liquid",
        "twig" => "twig",
        "j2" | "jinja" | "jinja2" => "jinja",
        "lock" => "config",
        _ => "",
    }
}

pub fn file_icon(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let icon = file_icon_name(name, &ext);
    if icon.is_empty() {
        format!("{ICON_DIR}default_file.svg")
    } else {
        format!("{ICON_DIR}file_type_{icon}.svg")
    }
}

pub fn folder_icon(name: &str, open: bool) -> String {
    let kind = match name.to_ascii_lowercase().as_str() {
        "src" | "source" | "sources" => "src",
        "node_modules" => "node",
        ".git" => "git",
        ".github" => "github",
        "test" | "tests" | "__tests__" | "spec" => "test",
        "dist" | "build" | "out" | "target" | "release" => "dist",
        "docs" | "doc" => "docs",
        "images" | "img" | "icons" => "images",
        "config" | "configs" | ".config" => "config",
        "public" | "static" => "public",
        ".vscode" => "vscode",
        "scripts" | "script" | "bin" => "script",
        "components" | "component" => "component",
        "api" => "api",
        "db" | "database" | "migrations" => "db",
        _ => "",
    };
    let suffix = if open { "_opened" } else { "" };
    if kind.is_empty() {
        format!("{ICON_DIR}default_folder{suffix}.svg")
    } else {
        format!("{ICON_DIR}folder_type_{kind}{suffix}.svg")
    }
}

/// Language name for the highlighter. Tree-sitter grammars cover the common
/// languages; syntect (≈200 Sublime grammars) covers the rest by extension.
pub fn language_for(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match name.as_str() {
        "makefile" | "gnumakefile" => return "make".into(),
        "dockerfile" => return "dockerfile".into(),
        "cmakelists.txt" => return "cmake".into(),
        "cargo.lock" => return "toml".into(),
        ".bashrc" | ".zshrc" | ".profile" | ".bash_profile" => return "bash".into(),
        _ => {}
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "py" | "pyw" | "pyi" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "html" | "htm" | "xhtml" => "html",
        "css" => "css",
        "json" | "jsonc" | "json5" | "ipynb" => "json",
        "yml" | "yaml" => "yaml",
        "toml" => "toml",
        "md" | "markdown" | "mdx" => "markdown",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "cs" => "csharp",
        "php" => "php",
        "rb" => "ruby",
        "lua" => "lua",
        "sh" | "bash" | "zsh" => "bash",
        "sql" => "sql",
        "svelte" => "svelte",
        "astro" => "astro",
        "ex" | "exs" => "elixir",
        "scala" | "sc" => "scala",
        "zig" => "zig",
        "graphql" | "gql" => "graphql",
        "proto" => "proto",
        "cmake" => "cmake",
        "diff" | "patch" => "diff",
        "ejs" => "ejs",
        "erb" => "erb",
        "" => "text",
        other => return other.to_string(),
    }
    .into()
}

pub fn language_label(lang: &str) -> String {
    match lang {
        "rust" => "Rust".into(),
        "python" => "Python".into(),
        "javascript" => "JavaScript".into(),
        "typescript" => "TypeScript".into(),
        "tsx" => "TypeScript React".into(),
        "html" => "HTML".into(),
        "css" => "CSS".into(),
        "json" => "JSON".into(),
        "yaml" => "YAML".into(),
        "toml" => "TOML".into(),
        "markdown" => "Markdown".into(),
        "go" => "Go".into(),
        "java" => "Java".into(),
        "kotlin" => "Kotlin".into(),
        "swift" => "Swift".into(),
        "c" => "C".into(),
        "cpp" => "C++".into(),
        "csharp" => "C#".into(),
        "php" => "PHP".into(),
        "ruby" => "Ruby".into(),
        "lua" => "Lua".into(),
        "bash" => "Shell".into(),
        "sql" => "SQL".into(),
        "text" => "Plain Text".into(),
        other => {
            let mut c = other.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => "Plain Text".into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mapped_icon_exists() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        for name in [
            "main.rs", "a.py", "x.tsx", "Cargo.toml", "package.json", ".gitignore", "Dockerfile",
            "Makefile", ".env.local", "vite.config.ts", "a.unknown", "photo.png", "a.ipynb",
        ] {
            let icon = file_icon(Path::new(name));
            assert!(root.join(&icon).exists(), "{name} -> {icon} missing");
        }
        for (name, open) in [("src", true), ("node_modules", false), ("random", true), (".github", false)] {
            let icon = folder_icon(name, open);
            assert!(root.join(&icon).exists(), "{name} -> {icon} missing");
        }
        assert_eq!(language_for(Path::new("a.tsx")), "tsx");
        assert_eq!(language_for(Path::new("a.hs")), "hs");
    }
}
