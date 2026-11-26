//! Lightweight symbol outline for "Go to Symbol" (`@` in the palette).
//! Works without a language server, so it's instant for every file.

use std::sync::OnceLock;

use regex::Regex;

#[derive(Clone, Debug, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub kind: &'static str,
    /// 1-based line.
    pub line: u32,
}

fn patterns(language: &str) -> &'static [(Regex, &'static str)] {
    static RUST: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    static PY: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    static JS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    static GO: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    static C: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    static SWIFT: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    static JAVA: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    static RUBY: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    static MD: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    static CSS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    static GENERIC: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let r = |p: &str| Regex::new(p).unwrap();
    match language {
        "rust" => RUST.get_or_init(|| vec![
            (r(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?fn\s+(\w+)"), "fn"),
            (r(r"^\s*(?:pub(?:\([^)]*\))?\s+)?struct\s+(\w+)"), "struct"),
            (r(r"^\s*(?:pub(?:\([^)]*\))?\s+)?enum\s+(\w+)"), "enum"),
            (r(r"^\s*(?:pub(?:\([^)]*\))?\s+)?trait\s+(\w+)"), "trait"),
            (r(r"^\s*impl(?:<[^>]*>)?\s+(?:[\w:<>, ]+\s+for\s+)?([\w:]+)"), "impl"),
            (r(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:mod|type|const|static)\s+(\w+)"), "item"),
            (r(r"^\s*macro_rules!\s+(\w+)"), "macro"),
        ]),
        "python" => PY.get_or_init(|| vec![
            (r(r"^\s*(?:async\s+)?def\s+(\w+)"), "def"),
            (r(r"^\s*class\s+(\w+)"), "class"),
        ]),
        "javascript" | "typescript" | "tsx" | "svelte" | "vue" | "astro" => JS.get_or_init(|| vec![
            (r(r"^\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\*?\s+(\w+)"), "function"),
            (r(r"^\s*(?:export\s+)?(?:default\s+)?(?:abstract\s+)?class\s+(\w+)"), "class"),
            (r(r"^\s*(?:export\s+)?(?:interface|type|enum)\s+(\w+)"), "type"),
            (r(r"^\s*(?:export\s+)?(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s*)?(?:\([^)]*\)|\w+)\s*=>"), "function"),
            (r(r"^\s+(?:public\s+|private\s+|protected\s+|static\s+|async\s+)*(\w+)\s*\([^)]*\)\s*(?::\s*[\w<>\[\]| ]+)?\s*\{"), "method"),
        ]),
        "go" => GO.get_or_init(|| vec![
            (r(r"^func\s+(?:\([^)]*\)\s*)?(\w+)"), "func"),
            (r(r"^type\s+(\w+)"), "type"),
        ]),
        "c" | "cpp" | "csharp" | "objectivec" => C.get_or_init(|| vec![
            (r(r"^\s*(?:class|struct|enum|union|namespace)\s+(\w+)"), "type"),
            (r(r"^[A-Za-z_][\w\s\*&:<>,]*?\b(\w+)\s*\([^;]*\)\s*(?:const\s*)?\{?\s*$"), "fn"),
        ]),
        "swift" | "kotlin" | "kts" | "scala" | "dart" => SWIFT.get_or_init(|| vec![
            (r(r"^\s*(?:@\w+\s+)*(?:public\s+|private\s+|internal\s+|open\s+|static\s+|override\s+|final\s+|suspend\s+)*(?:func|fun|def)\s+(\w+)"), "func"),
            (r(r"^\s*(?:public\s+|private\s+|open\s+|final\s+|data\s+|sealed\s+|abstract\s+)*(?:class|struct|enum|protocol|interface|object|extension|trait)\s+(\w+)"), "type"),
        ]),
        "java" | "php" => JAVA.get_or_init(|| vec![
            (r(r"^\s*(?:public\s+|private\s+|protected\s+|abstract\s+|final\s+)*(?:class|interface|enum|record|trait)\s+(\w+)"), "type"),
            (r(r"^\s*(?:public\s+|private\s+|protected\s+|static\s+|final\s+|abstract\s+|synchronized\s+)*(?:function\s+)?[\w<>\[\],\s]*?\b(\w+)\s*\([^;]*\)\s*(?:throws [\w, ]+)?\s*\{"), "method"),
        ]),
        "ruby" | "elixir" | "lua" => RUBY.get_or_init(|| vec![
            (r(r"^\s*(?:def|defp|function|local\s+function)\s+([\w.:?!]+)"), "def"),
            (r(r"^\s*(?:class|module|defmodule)\s+([\w:.]+)"), "class"),
        ]),
        "markdown" => MD.get_or_init(|| vec![(r(r"^(#{1,6}\s+.+)$"), "heading")]),
        "css" | "scss" | "less" => CSS.get_or_init(|| vec![(r(r"^([^\s{@][^{]*?)\s*\{"), "rule")]),
        _ => GENERIC.get_or_init(|| vec![
            (r(r"^\s*(?:export\s+)?(?:pub\s+)?(?:async\s+)?(?:fn|func|function|def|fun|sub|proc)\s+(\w+)"), "fn"),
            (r(r"^\s*(?:export\s+)?(?:pub\s+)?(?:class|struct|enum|interface|trait|module|type)\s+(\w+)"), "type"),
        ]),
    }
}

const KEYWORDS: &[&str] = &["if", "for", "while", "switch", "catch", "return", "else", "match", "new", "sizeof"];

pub fn extract(text: &str, language: &str) -> Vec<Symbol> {
    let pats = patterns(language);
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate().take(20_000) {
        if line.len() > 400 {
            continue;
        }
        for (re, kind) in pats {
            if let Some(m) = re.captures(line).and_then(|c| c.get(1)) {
                let name = m.as_str().trim().to_string();
                if !KEYWORDS.contains(&name.as_str()) {
                    out.push(Symbol { name, kind, line: i as u32 + 1 });
                }
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_symbols() {
        let rs = "pub struct App;\nimpl App {\n    pub async fn run(&self) {}\n}\nfn main() {}\n";
        let names: Vec<_> = extract(rs, "rust").into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["App", "App", "run", "main"]);
        let py = "class A:\n    def go(self):\n        if x:\n            pass\n";
        let names: Vec<_> = extract(py, "python").into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["A", "go"]);
        let ts = "export const add = (a, b) => a + b;\nexport interface P {}\nfunction f() {}\n";
        let names: Vec<_> = extract(ts, "typescript").into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["add", "P", "f"]);
    }
}
