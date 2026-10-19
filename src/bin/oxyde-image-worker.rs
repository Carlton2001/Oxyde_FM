#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#[cfg(target_os = "windows")]
#[path = "../ui/image_preview/svg.rs"]
mod svg;
fn main() {
    #[cfg(target_os = "windows")]
    {
        let renderer = std::sync::OnceLock::new();
        if !oxyde_lib::commands::image_handle_worker::run_if_requested(&|data, policy| {
            // Loading system fonts is expensive and only SVG decoding needs it.
            // Raster images and Office text must not pay this cost per worker.
            let renderer = renderer.get_or_init(|| gpui::SvgRenderer::new(std::sync::Arc::new(())));
            svg::svg(data, renderer, policy)
        }) {
            if !oxyde_lib::commands::office_handle_worker::run_if_requested() {
                oxyde_lib::commands::audio_handle_worker::run_if_requested();
            }
        }
    }
}
