//! User settings, stored as JSON in the platform config directory.
//! The file is opened in the editor itself (Cmd+,), so there is no
//! separate settings UI to click through.

use std::{collections::BTreeMap, path::PathBuf};

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Eclipse, Nebula, Aurora, Midnight, Supernova (dark) or Lunar (light).
    pub theme: String,
    /// Accent color as hex ("#f2b36d"); empty uses the theme's own accent.
    pub accent: String,
    /// Editor and terminal font family.
    pub editor_font: String,
    pub font_size: f32,
    pub terminal_font_size: f32,
    pub line_numbers: bool,
    pub tab_size: usize,
    pub soft_wrap: bool,
    pub format_on_save: bool,
    pub auto_save: bool,
    pub rainbow_indent: bool,
    /// Port Live Server listens on (falls back to the next free port).
    pub live_server_port: u16,
    /// Clear the terminal before each Code Runner run.
    pub runner_clear_before_run: bool,
    /// Save the file before Code Runner runs it.
    pub runner_save_before_run: bool,
    /// Per-extension override for Code Runner, e.g. {"py": "python3.12 -u $file"}.
    pub runner_executor_map: BTreeMap<String, String>,
    /// Optional AI assistant. Off unless you enable it.
    pub ai: AiSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    pub enabled: bool,
    /// "anthropic", "openai" or "gemini".
    pub provider: String,
    pub anthropic: ProviderSettings,
    pub openai: ProviderSettings,
    pub gemini: ProviderSettings,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderSettings {
    pub api_key: String,
    pub model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "Eclipse".into(),
            accent: String::new(),
            editor_font: "JetBrains Mono".into(),
            font_size: 13.0,
            terminal_font_size: 13.0,
            line_numbers: true,
            tab_size: 4,
            soft_wrap: false,
            format_on_save: false,
            auto_save: false,
            rainbow_indent: true,
            live_server_port: 5500,
            runner_clear_before_run: true,
            runner_save_before_run: true,
            runner_executor_map: BTreeMap::new(),
            ai: AiSettings::default(),
        }
    }
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "anthropic".into(),
            anthropic: ProviderSettings {
                api_key: String::new(),
                model: "claude-opus-5-5".into(),
            },
            openai: ProviderSettings {
                api_key: String::new(),
                model: "gpt-5".into(),
            },
            gemini: ProviderSettings {
                api_key: String::new(),
                model: "gemini-2.5-pro".into(),
            },
        }
    }
}

impl Global for Settings {}

impl Settings {
    pub fn dir() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("eclipse");
        // One-time migration from the app's earlier name.
        let old = base.join("ide");
        if !dir.exists() && old.is_dir() {
            let _ = std::fs::rename(&old, &dir);
        }
        dir
    }

    pub fn path() -> PathBuf {
        Self::dir().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let mut s: Settings = serde_json::from_str(&text).unwrap_or_else(|err| {
                    log::warn!("settings.json is invalid, using defaults: {err}");
                    Self::default()
                });
                if s.theme == "Daylight" {
                    s.theme = "Lunar".into();
                }
                s
            }
            Err(_) => {
                let settings = Self::default();
                settings.save();
                settings
            }
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, text);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                // The file can hold API keys: keep it private to the user.
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
        }
    }

    pub fn get(cx: &App) -> &Settings {
        cx.global::<Settings>()
    }

    /// Change settings: applies the theme, stores the global (which notifies
    /// every observer) and writes the file.
    pub fn update(cx: &mut App, f: impl FnOnce(&mut Settings)) {
        let mut settings = Self::get(cx).clone();
        f(&mut settings);
        settings.save();
        crate::apply_theme(&settings, cx);
        cx.set_global(settings);
    }

    pub fn ai_ready(&self) -> bool {
        self.ai.enabled && !self.ai.active().api_key.trim().is_empty()
    }
}

impl AiSettings {
    pub fn active(&self) -> &ProviderSettings {
        match self.provider.as_str() {
            "openai" => &self.openai,
            "gemini" => &self.gemini,
            _ => &self.anthropic,
        }
    }
}

/// Small persisted state that is not a preference: recent folders, layout.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub recent: Vec<PathBuf>,
    pub last_folder: Option<PathBuf>,
}

impl Session {
    fn path() -> PathBuf {
        Settings::dir().join("session.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(Settings::dir());
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), text);
        }
    }

    pub fn push_recent(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path.clone());
        self.recent.truncate(8);
        self.last_folder = Some(path);
        self.save();
    }
}
