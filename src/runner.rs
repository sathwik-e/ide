//! Code Runner: runs the current file with the right toolchain, in the
//! integrated terminal (so programs can read stdin). Mirrors the executor map
//! of the popular VS Code extension, with per-extension overrides in settings.

use std::{collections::BTreeMap, path::Path};

fn default_executor(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "js" | "mjs" | "cjs" => "node $file",
        "ts" | "mts" => "npx --yes tsx $file",
        "py" => "python3 -u $file",
        "rb" => "ruby $file",
        "php" => "php $file",
        "pl" => "perl $file",
        "lua" => "lua $file",
        "sh" | "bash" => "bash $file",
        "zsh" => "zsh $file",
        "fish" => "fish $file",
        "go" => "go run $file",
        "rs" => "cd $dir && rustc $fileName -o $fileNameWithoutExt && ./$fileNameWithoutExt",
        "c" => "cd $dir && cc $fileName -o $fileNameWithoutExt && ./$fileNameWithoutExt",
        "cpp" | "cc" | "cxx" => {
            "cd $dir && c++ -std=c++20 $fileName -o $fileNameWithoutExt && ./$fileNameWithoutExt"
        }
        "m" => {
            "cd $dir && cc -framework Foundation $fileName -o $fileNameWithoutExt && ./$fileNameWithoutExt"
        }
        "java" => "cd $dir && javac $fileName && java $fileNameWithoutExt",
        "kt" => {
            "cd $dir && kotlinc $fileName -include-runtime -d $fileNameWithoutExt.jar && java -jar $fileNameWithoutExt.jar"
        }
        "kts" => "kotlinc -script $file",
        "swift" => "swift $file",
        "dart" => "dart run $file",
        "cs" | "csx" => "dotnet script $file",
        "fs" | "fsx" => "dotnet fsi $file",
        "scala" | "sc" => "scala run $file",
        "groovy" => "groovy $file",
        "hs" => "runghc $file",
        "ml" => "ocaml $file",
        "clj" => "clojure -M $file",
        "ex" | "exs" => "elixir $file",
        "erl" => "escript $file",
        "jl" => "julia $file",
        "r" => "Rscript $file",
        "nim" => "nim compile --verbosity:0 --hints:off --run $file",
        "zig" => "zig run $file",
        "cr" => "crystal $file",
        "v" => "v run $file",
        "d" => "cd $dir && dmd -run $fileName",
        "pas" => "cd $dir && fpc $fileName && ./$fileNameWithoutExt",
        "ps1" => "pwsh -File $file",
        "bat" | "cmd" => "cmd /c $file",
        "applescript" => "osascript $file",
        "tcl" => "tclsh $file",
        "coffee" => "npx --yes coffee $file",
        "sass" | "scss" => "npx --yes sass $file",
        "sql" => "sqlite3 < $file",
        "html" | "htm" => "open $file",
        _ => return None,
    })
}

fn shell_quote(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+,:@".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Build the shell command that runs `path`, or `None` if no runner is known.
pub fn command_for(path: &Path, overrides: &BTreeMap<String, String>) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let template = overrides
        .get(&ext)
        .cloned()
        .or_else(|| default_executor(&ext).map(str::to_string))?;

    let dir = path.parent().map(|p| p.to_string_lossy().to_string())?;
    let file_name = path.file_name()?.to_string_lossy().to_string();
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let file = path.to_string_lossy().to_string();

    // Longest placeholders first so `$file` doesn't eat `$fileName`.
    Some(
        template
            .replace("$fileNameWithoutExt", &shell_quote(&stem))
            .replace("$fileName", &shell_quote(&file_name))
            .replace("$dir", &shell_quote(&dir))
            .replace("$file", &shell_quote(&file)),
    )
}

pub fn can_run(path: &Path, overrides: &BTreeMap<String, String>) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let e = e.to_ascii_lowercase();
            overrides.contains_key(&e) || default_executor(&e).is_some()
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_placeholders_and_quotes_paths() {
        let cmd = command_for(Path::new("/tmp/my dir/main.c"), &BTreeMap::new()).unwrap();
        assert_eq!(cmd, "cd '/tmp/my dir' && cc main.c -o main && ./main");
        let py = command_for(Path::new("/tmp/a.py"), &BTreeMap::new()).unwrap();
        assert_eq!(py, "python3 -u /tmp/a.py");
    }

    #[test]
    fn overrides_win_and_unknown_is_none() {
        let mut map = BTreeMap::new();
        map.insert("py".to_string(), "uv run $file".to_string());
        assert_eq!(command_for(Path::new("/x/a.py"), &map).unwrap(), "uv run /x/a.py");
        assert!(command_for(Path::new("/x/a.unknownext"), &BTreeMap::new()).is_none());
        assert!(can_run(Path::new("/x/main.rs"), &BTreeMap::new()));
    }
}
