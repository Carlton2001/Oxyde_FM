use crate::ui::components::icons::render_file_icon;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

struct GridThumbnailAsset;
pub(crate) fn invalidate_thumbnail(path: &str, cx: &mut App) {
    crate::ui::components::tooltip::invalidate_pdf_preview(path);
    cx.remove_asset::<GridThumbnailAsset>(&SharedString::from(path.to_string()));
}

impl Asset for GridThumbnailAsset {
    type Source = SharedString;
    type Output = Result<std::sync::Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        _cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let task = crate::ui::preview_jobs::load(move || {
            oxyde_lib::platform::platform()
                .thumbnail(source.to_string())
                .map_err(|error| ImageCacheError::Asset(error.to_string().into()))
        });
        async move {
            let rgba = task
                .await
                .map_err(|error| ImageCacheError::Asset(error.into()))?;
            Ok(std::sync::Arc::new(RenderImage::new(vec![
                image::Frame::new(rgba),
            ])))
        }
    }
}

pub(super) fn render_grid_thumbnail(
    path: String,
    fallback_icon: crate::ui::components::icons::IconType,
    theme: OxydeTheme,
    system: bool,
) -> impl IntoElement {
    let fallback_path = path.clone();
    let source: SharedString = path.clone().into();
    img(move |window: &mut Window, cx: &mut App| {
        window.use_asset::<GridThumbnailAsset>(&source, cx)
    })
    .id(SharedString::from(format!("grid-thumbnail-{path}")))
    .size(rems(5.0))
    .object_fit(ObjectFit::Contain)
    .with_fallback(move || {
        render_file_icon(fallback_path.clone(), fallback_icon, rems(4.0), theme.text_muted, system, true)
    })
}
