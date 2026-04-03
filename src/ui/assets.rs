use std::borrow::Cow;
use std::path::PathBuf;
use gpui::{AssetSource, SharedString, Result};

pub struct FileAssetSource {
    base_path: PathBuf,
}

impl FileAssetSource {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    pub fn load_fonts(&self, cx: &gpui::App) -> Result<()> {
        let font_paths = self.list("fonts")?;
        let mut embedded_fonts = Vec::new();
        for font_path in font_paths {
            if font_path.ends_with(".ttf") || font_path.ends_with(".otf") {
                if let Some(font_bytes) = self.load(&font_path)? {
                    embedded_fonts.push(font_bytes);
                }
            }
        }
        cx.text_system().add_fonts(embedded_fonts)?;
        Ok(())
    }
}

impl AssetSource for FileAssetSource {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let full_path = self.base_path.join(path);
        if full_path.exists() {
            let bytes = std::fs::read(full_path)?;
            Ok(Some(Cow::Owned(bytes)))
        } else {
            Ok(None)
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let full_path = self.base_path.join(path);
        let mut result = Vec::new();
        if full_path.is_dir() {
            for entry in std::fs::read_dir(full_path)? {
                let entry = entry?;
                if let Some(name) = entry.file_name().to_str() {
                    // Prepend the requested path to build the relative path needed for load()
                    let relative_path = format!("{}/{}", path, name);
                    result.push(SharedString::from(relative_path));
                }
            }
        }
        Ok(result)
    }
}
