#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ui;

use crate::ui::app::OxydeApp;
use gpui::*;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off"))
        .filter_module("oxyde", log::LevelFilter::Info)
        .filter_module("oxyde_lib", log::LevelFilter::Info)
        .init();
    let _rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");
    let _guard = _rt.enter();

    let config_path = oxyde_lib::models::ConfigManager::config_path().ok();
    let (config, load_failed) = match config_path.as_deref() {
        Some(path) => match oxyde_lib::models::ConfigManager::read_from_path(path) {
            Ok(config) => (config, false),
            Err(error) => {
                log::warn!("Cannot load preferences: {error:?}");
                (oxyde_lib::models::AppConfig::default(), true)
            }
        },
        None => (oxyde_lib::models::AppConfig::default(), true),
    };

    let asset_source = crate::ui::assets::EmbeddedAssetSource::new();
    gpui_platform::application()
        .with_assets(asset_source)
        .run(move |cx| {
            let assets = crate::ui::assets::EmbeddedAssetSource::new();
            assets.load_fonts(cx).expect("fonts could not be loaded");
            cx.set_global(crate::ui::theme::OxydeTheme::for_id(
                crate::ui::theme::ThemeId::from_id(&config.theme),
            ));
            cx.set_global(crate::ui::i18n::I18n::for_language(
                crate::ui::i18n::Language::from_id(&config.language),
            ));
            crate::ui::components::input::bind_keys(cx);
            OxydeApp::bind_keys(cx);
            let app = cx.new(|cx| OxydeApp::new(config, config_path, load_failed, cx));
            app.update(cx, |this, cx| {
                this.handle = Some(app.clone());
                this.start_watcher_task(cx);
                let pane_id = this.state.active_pane_id.clone();
                if let Some(pane) = this.state.find_pane(&pane_id) {
                    this.request_tab_files(
                        &pane_id,
                        pane.active_tab_index,
                        pane.active_tab().path.clone(),
                        false,
                        cx,
                    );
                }
            });
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(100.0), px(100.0)),
                        size: size(px(1200.0), px(800.0)),
                    })),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        title: None,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    let focus_handle = app.read(cx).focus_handle.clone();
                    window.on_next_frame(move |window, cx| {
                        window.focus(&focus_handle, cx);
                        // On charge l'icône PNG (pour Windows)
                        let assets = crate::ui::assets::EmbeddedAssetSource::new();
                        if let Ok(Some(icon_bytes)) = assets.load("icons/logo.png") {
                            oxyde_lib::platform::platform().set_application_icon(&icon_bytes);
                        }
                    });
                    app
                },
            )
            .expect("window opens");
        });
}
