#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#[cfg(target_os = "windows")]
#[path = "../ui/image_preview/svg.rs"]
mod svg;
fn main() {
    #[cfg(target_os = "windows")]
    {
        let renderer = gpui::SvgRenderer::new(std::sync::Arc::new(()));
        if !oxyde_lib::commands::image_handle_worker::run_if_requested(&|data, policy| {
            svg::svg(data, &renderer, policy)
        }) {
            oxyde_lib::commands::office_handle_worker::run_if_requested();
        }
    }
}
