use crate::ui::components::icons::{entry_icon, get_icon_type, render_file_icon, IconType};
use crate::ui::theme::ThemeContext;
use gpui::*;
use std::path::PathBuf;

#[cfg(test)]
#[path = "tests/icons.rs"]
mod tests;

fn resolve(
    source: &std::path::Path,
    destination: Option<&std::path::Path>,
) -> Result<(String, IconType), oxyde_lib::models::CommandError> {
    oxyde_lib::models::get_file_entry_from_path(source)
        .or_else(|error| match destination {
            Some(destination) => oxyde_lib::models::get_file_entry_from_path(destination),
            None => Err(error),
        })
        .map(|entry| (entry.path.clone(), entry_icon(&entry)))
}

struct ItemKindAsset;

impl Asset for ItemKindAsset {
    type Source = (PathBuf, Option<PathBuf>);
    type Output = (String, IconType);

    fn load(
        source: Self::Source,
        _: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let path = source.0.to_string_lossy().into_owned();
        let name = source
            .0
            .file_name()
            .unwrap_or(source.0.as_os_str())
            .to_string_lossy();
        let fallback = get_icon_type(&name, false, Some(&path), false, false);
        let task = crate::ui::preview_jobs::load(move || resolve(&source.0, source.1.as_deref()));
        async move { task.await.unwrap_or((path, fallback)) }
    }
}

#[derive(IntoElement)]
pub(super) struct ItemIcon {
    pub path: PathBuf,
    pub destination: Option<PathBuf>,
    pub system: bool,
}

impl RenderOnce for ItemIcon {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let path = self.path.to_string_lossy().into_owned();
        let name = self
            .path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_string_lossy();
        let fallback = get_icon_type(&name, false, Some(&path), false, false);
        let (path, icon) = window
            .use_asset::<ItemKindAsset>(&(self.path, self.destination), cx)
            .unwrap_or((path, fallback));
        render_file_icon(path, icon, rems(1.25), cx.theme(), self.system, false)
    }
}
