#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ui;

use gpui::*;
use crate::ui::app::OxydeApp;

fn main() {
    Application::new()
        .with_assets(crate::ui::assets::FileAssetSource::new("assets"))
        .run(|cx| {
        let assets = crate::ui::assets::FileAssetSource::new("assets");
        assets.load_fonts(cx).expect("fonts could not be loaded");
        cx.set_global(crate::ui::theme::OxydeTheme::dark());
        crate::ui::components::input::bind_keys(cx);
        OxydeApp::bind_keys(cx);
        let app = cx.new(|cx| OxydeApp::empty(cx));
        let _ = app.update(cx, |this, _| {
            this.handle = Some(app.clone());
        });
        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Oxyde".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |_, _cx| {
                app
            },
        ).expect("window opens");
    });
}
