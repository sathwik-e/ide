//! Project tasks: detects runnable scripts so they're one palette entry away.

use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectTask {
    pub label: String,
    pub command: String,
}

fn task(label: impl Into<String>, command: impl Into<String>) -> ProjectTask {
    ProjectTask { label: label.into(), command: command.into() }
}

pub fn detect(root: &Path) -> Vec<ProjectTask> {
    let mut out = Vec::new();

    // npm / yarn / pnpm / bun scripts
    if let Ok(text) = std::fs::read_to_string(root.join("package.json")) {
        let runner = if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
            "bun run"
        } else if root.join("pnpm-lock.yaml").exists() {
            "pnpm run"
        } else if root.join("yarn.lock").exists() {
            "yarn"
        } else {
            "npm run"
        };
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) {
                for name in scripts.keys() {
                    out.push(task(format!("{runner} {name}"), format!("{runner} {name}")));
                }
            }
        }
        if !root.join("node_modules").exists() {
            let install = runner.split(' ').next().unwrap_or("npm");
            out.push(task(format!("{install} install"), format!("{install} install")));
        }
    }

    // Deno tasks
    for file in ["deno.json", "deno.jsonc"] {
        if let Ok(text) = std::fs::read_to_string(root.join(file)) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(tasks) = json.get("tasks").and_then(|s| s.as_object()) {
                    for name in tasks.keys() {
                        out.push(task(format!("deno task {name}"), format!("deno task {name}")));
                    }
                }
            }
        }
    }

    if root.join("Cargo.toml").exists() {
        for (label, cmd) in [
            ("cargo run", "cargo run"),
            ("cargo build", "cargo build"),
            ("cargo test", "cargo test"),
            ("cargo check", "cargo check"),
            ("cargo clippy", "cargo clippy"),
            ("cargo build --release", "cargo build --release"),
        ] {
            out.push(task(label, cmd));
        }
    }

    if root.join("go.mod").exists() {
        out.push(task("go run .", "go run ."));
        out.push(task("go build ./...", "go build ./..."));
        out.push(task("go test ./...", "go test ./..."));
    }

    if root.join("Package.swift").exists() {
        out.push(task("swift build", "swift build"));
        out.push(task("swift run", "swift run"));
        out.push(task("swift test", "swift test"));
    }

    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        if root.join("requirements.txt").exists() {
            out.push(task("pip install -r requirements.txt", "python3 -m pip install -r requirements.txt"));
        }
        if root.join("tests").is_dir() || root.join("pytest.ini").exists() {
            out.push(task("pytest", "python3 -m pytest"));
        }
    }

    for file in ["Makefile", "makefile", "GNUmakefile"] {
        if let Ok(text) = std::fs::read_to_string(root.join(file)) {
            for line in text.lines() {
                if let Some((name, _)) = line.split_once(':') {
                    let name = name.trim();
                    let valid = !name.is_empty()
                        && !line.starts_with(['\t', ' ', '.', '#'])
                        && !line.contains(":=")
                        && name.chars().all(|c| c.is_alphanumeric() || "-_/".contains(c));
                    if valid {
                        out.push(task(format!("make {name}"), format!("make {name}")));
                    }
                }
            }
            break;
        }
    }

    if let Ok(text) = std::fs::read_to_string(root.join("justfile")).or_else(|_| std::fs::read_to_string(root.join("Justfile"))) {
        for line in text.lines() {
            if line.starts_with(|c: char| c.is_alphanumeric()) {
                if let Some((name, _)) = line.split_once(':') {
                    let name = name.split_whitespace().next().unwrap_or("");
                    if !name.is_empty() && !line.contains(":=") {
                        out.push(task(format!("just {name}"), format!("just {name}")));
                    }
                }
            }
        }
    }

    if root.join("docker-compose.yml").exists() || root.join("compose.yaml").exists() {
        out.push(task("docker compose up", "docker compose up"));
    }

    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_npm_cargo_and_make() {
        let dir = std::env::temp_dir().join(format!("orion-tasks-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), r#"{"scripts":{"dev":"vite","test":"vitest"}}"#).unwrap();
        std::fs::write(dir.join("yarn.lock"), "").unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(dir.join("Makefile"), "build:\n\tcc x.c\n.PHONY: build\nX := 1\n").unwrap();
        let tasks = detect(&dir);
        let labels: Vec<_> = tasks.iter().map(|t| t.label.as_str()).collect();
        assert!(labels.contains(&"yarn dev"));
        assert!(labels.contains(&"cargo test"));
        assert!(labels.contains(&"make build"));
        assert!(!labels.iter().any(|l| l.contains("PHONY") || l.contains("X")));
        let _ = std::fs::remove_dir_all(dir);
    }
}
