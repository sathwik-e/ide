//! Asset source: the app's own embedded files (themes, fonts, file icons)
//! layered over the full Lucide icon catalog that ships with GPUI Kit.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
#[include = "fonts/*.ttf"]
pub struct AppAssets;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        if let Some(file) = AppAssets::get(path) {
            return Ok(Some(file.data));
        }
        gpui_kit::assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out: Vec<SharedString> = AppAssets::iter()
            .filter(|name| name.starts_with(path))
            .map(|name| SharedString::from(name.to_string()))
            .collect();
        out.extend(gpui_kit::assets::AllAssets.list(path)?);
        Ok(out)
    }
}

pub fn font_bytes() -> Vec<Cow<'static, [u8]>> {
    AppAssets::iter()
        .filter(|name| name.starts_with("fonts/"))
        .filter_map(|name| AppAssets::get(&name).map(|f| f.data))
        .collect()
}
