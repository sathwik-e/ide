use std::{process::Command, sync::OnceLock};

/// Apps launched from Finder/Dock get a minimal PATH. Resolve the user's
/// login-shell PATH once so formatters and runners find their tools.
pub fn login_path() -> String {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let from_shell = Command::new(&shell)
            .args(["-ilc", "printf '%s' \"$PATH\""])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|p| !p.is_empty());
        let current = std::env::var("PATH").unwrap_or_default();
        let mut path = from_shell.unwrap_or(current);
        for extra in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
            if !path.split(':').any(|p| p == extra) {
                path.push(':');
                path.push_str(extra);
            }
        }
        path
    })
    .clone()
}

pub fn human_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.)
    } else {
        format!("{:.1} MB", bytes as f64 / 1024. / 1024.)
    }
}
