use std::borrow::Cow;
use gpui::{AssetSource, SharedString, Result};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets/"]
struct Assets;

pub struct EmbeddedAssetSource;

impl EmbeddedAssetSource {
    pub fn new() -> Self {
        Self
    }

    pub fn load_fonts(&self, cx: &gpui::App) -> Result<()> {
        let mut embedded_fonts = Vec::new();
        // List fonts from the embedded assets
        for path in Assets::iter() {
            if path.starts_with("fonts/") && (path.ends_with(".ttf") || path.ends_with(".otf")) {
                if let Some(font_bytes) = self.load(&path)? {
                    embedded_fonts.push(font_bytes);
                }
            }
        }
        cx.text_system().add_fonts(embedded_fonts)?;
        Ok(())
    }
}

impl AssetSource for EmbeddedAssetSource {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        // Rust-embed handles paths with standard separators, even on Windows
        match Assets::get(path) {
            Some(file) => Ok(Some(file.data)),
            None => Ok(None),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut result = Vec::new();
        let prefix = if path.is_empty() {
            "".to_string()
        } else {
            let p = path.trim_end_matches('/');
            format!("{}/", p)
        };

        for embedded_path in Assets::iter() {
            if embedded_path.starts_with(&prefix) {
                result.push(SharedString::from(embedded_path.to_string()));
            }
        }
        Ok(result)
    }
}
