use crate::ui::components::icons::render_file_icon;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
mod lifecycle;
pub(crate) use lifecycle::clear_grid_thumbnails;
use lifecycle::Source;

pub(super) fn is_audio(file: &oxyde_lib::models::FileEntry) -> bool {
    file.native_kind == Some(oxyde_lib::models::file_entry::NativeFileKind::Audio)
        || file.name.rsplit_once('.').is_some_and(|(_, extension)| {
            crate::ui::components::icons::AUDIO_EXTENSIONS.contains(extension.to_ascii_lowercase().as_str())
        })
}

pub(super) fn supports_thumbnail(file: &oxyde_lib::models::FileEntry) -> bool {
    is_audio(file) || matches!(crate::ui::components::icons::get_preview_type(&file.name),
        crate::ui::components::icons::PreviewType::Image | crate::ui::components::icons::PreviewType::Pdf)
}

struct GridThumbnailAsset;
pub(crate) fn invalidate_thumbnail(path: &str, cx: &mut App) {
    crate::ui::components::tooltip::invalidate_media_preview(path);
    lifecycle::invalidate(path, cx);
}

impl Asset for GridThumbnailAsset {
    type Source = Source;
    type Output = Result<std::sync::Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let renderer = cx.svg_renderer();
        let task = crate::ui::preview_jobs::load(move || {
            crate::ui::preview_disk::load_grid(&source.path, source.stamp.as_ref(), source.audio, &renderer)
                .map_err(|error| ImageCacheError::Asset(error.into()))
        });
        async move {
            let image = task
                .await
                .map_err(|error| ImageCacheError::Asset(error.into()))?;
            Ok(image)
        }
    }
}

pub(super) fn render_grid_thumbnail(
    file: &oxyde_lib::models::FileEntry,
    pane_id: &str,
    fallback_icon: crate::ui::components::icons::IconType,
    theme: OxydeTheme,
    system: bool,
) -> impl IntoElement {
    let path = file.path.clone();
    let fallback_path = path.clone();
    let source = Source::for_file(file);
    let owner: SharedString = format!("grid-thumbnail-owner-{pane_id}-{path}").into();
    img(move |window: &mut Window, cx: &mut App| {
        let source = lifecycle::use_source(&source, &owner, window, cx);
        let result = window.use_asset::<GridThumbnailAsset>(&source, cx);
        if let Some(Ok(image)) = &result {
            lifecycle::loaded(&source, image, cx);
        }
        result
    })
    .id(SharedString::from(format!("grid-thumbnail-{path}")))
    .size(rems(5.0))
    .object_fit(ObjectFit::Contain)
    .with_fallback(move || {
        render_file_icon(
            fallback_path.clone(),
            fallback_icon,
            rems(4.0),
            &theme,
            system,
            true,
        )
    })
}

#[cfg(test)]
#[path = "thumbnails/tests/audio.rs"]
mod audio_tests;
