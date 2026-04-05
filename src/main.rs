#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ui;

use gpui::*;
use crate::ui::app::OxydeApp;

fn main() {
    let _rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");
    let _guard = _rt.enter();

    let asset_source = crate::ui::assets::EmbeddedAssetSource::new();
    Application::new()
        .with_assets(asset_source)
        .run(|cx| {
        let assets = crate::ui::assets::EmbeddedAssetSource::new();
        assets.load_fonts(cx).expect("fonts could not be loaded");
        cx.set_global(crate::ui::theme::OxydeTheme::dark());
        crate::ui::components::input::bind_keys(cx);
        OxydeApp::bind_keys(cx);
        let app = cx.new(|cx| OxydeApp::empty(cx));
        let _ = app.update(cx, |this, cx| {
            this.handle = Some(app.clone());
            this.start_watcher_task(cx);
        });
        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Oxyde".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                let focus_handle = app.read(cx).focus_handle.clone();
                window.on_next_frame(move |window, _| {
                    window.focus(&focus_handle);
                    // On charge l'icône PNG (pour Windows)
                    let assets = crate::ui::assets::EmbeddedAssetSource::new();
                    if let Ok(Some(icon_bytes)) = assets.load("icons/logo.png") {
                        oxyde_lib::platform::platform().set_application_icon(&icon_bytes);
                    }
                });
                app
            },
        ).expect("window opens");
    });
}
