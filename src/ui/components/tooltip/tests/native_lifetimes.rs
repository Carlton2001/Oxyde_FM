//! Real hidden-window frames must release previews, not wait for another hover.
use super::*;
use crate::ui::components::tooltip::{pixels, video::CancelVideoPreview, PreviewData, PreviewTask};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

struct Parent;
impl Render for Parent {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[::core::prelude::v1::test]
#[ignore = "Requires a native Windows GPUI renderer; run explicitly in isolation"]
fn native_hidden_tooltip_releases_preview_pixels_and_video_owner() {
    let _ = env_logger::Builder::new()
        .filter_module("gpui_windows::directx_atlas", log::LevelFilter::Debug)
        .try_init();
    let released = Arc::new(AtomicBool::new(false));
    let result = released.clone();
    let failures = Arc::new(Mutex::new(Vec::new()));
    let errors = failures.clone();
    let cancellation = crate::ui::preview_jobs::Cancellation::default();
    let cancelled = cancellation.clone();
    gpui_platform::application().run(move |cx| {
        cx.set_global(crate::ui::theme::OxydeTheme::dark());
        cx.set_global(crate::ui::i18n::I18n::default());
        cx.set_global(crate::ui::format::DateFormatPreference(Default::default()));
        let parent = cx
            .open_window(
                WindowOptions {
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Parent),
            )
            .unwrap();
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::from_pixel(512, 256, image::Rgba([30, 60, 90, 255])),
        )]));
        let pixels_probe = Arc::downgrade(&image);
        let preview = cx.new(|cx| {
            pixels::observe(cx);
            cx.on_release(move |_, _| result.store(true, Ordering::Release))
                .detach();
            PreviewTask {
                viewer: false,
                transition_image: None,
                maximum_size: None,
                data: Some(PreviewData::Image(image)),
                media_asset: None,
                original_image: None,
                audio_source: None,
                audio_cached: false,
                message: None,
                loading_delay_elapsed: false,
                _video_cancellation: Some(Arc::new(CancelVideoPreview(cancellation))),
            }
        });
        let tooltip = cx.new(|_| Tooltip {
            native_presentation: true,
            title: "memory probe".into(),
            wrap_title: false,
            meta: None,
            shortcut: None,
            secondary_action: None,
            file_info: None,
            preview_view: Some(preview),
            fixed_bounds: None,
        });
        let session_view = tooltip.clone().into();
        let handle = cx
            .open_window(
                WindowOptions {
                    kind: WindowKind::PopUp,
                    show: true,
                    focus: false,
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(100.0), px(100.0)),
                        size(px(600.0), px(400.0)),
                    ))),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| NativeTooltip {
                        view: Some(tooltip.into()),
                        measured: None,
                        scale: 1.0,
                    })
                },
            )
            .unwrap();
        let idle = cx.spawn(async |_| {});
        let id = parent.window_id();
        cx.default_global::<NativeWindows>()
            .windows
            .insert(id, (handle, idle));
        cx.default_global::<Sessions>()
            .windows
            .entry(id)
            .or_default()
            .tooltip = Some(session_view);
        cx.defer(move |cx| {
            AnyWindowHandle::from(handle)
                .update(cx, |_, window, cx| {
                    window.draw(cx).clear(cx);
                    window.draw(cx).clear(cx);
                    assert!(pixels_probe.upgrade().is_some());
                })
                .unwrap();
            AnyWindowHandle::from(parent)
                .update(cx, |_, window, cx| {
                    // Exercise the timer's actual invalid-session branch, without
                    // any pointer-exit event or test-side session removal.
                    dismiss_invalid_session(window, cx);
                })
                .unwrap();
            // Clearing frames releases the entity, whose release callback then
            // defers atlas eviction. Wait for both effect turns, with no redraw.
            cx.defer(move |cx| {
                cx.defer(move |cx| {
                    if pixels_probe.upgrade().is_some() {
                        errors
                            .lock()
                            .unwrap()
                            .push("hidden frames still retain source pixels");
                    }
                    if !cancelled.is_cancelled() {
                        errors
                            .lock()
                            .unwrap()
                            .push("hidden frames still own the video decoder");
                    }
                    close_for(id, cx);
                    let _ = parent.update(cx, |_, window, _| window.remove_window());
                    cx.quit();
                });
            });
        });
    });
    assert!(
        released.load(Ordering::Acquire),
        "preview entity was not released"
    );
    assert!(
        failures.lock().unwrap().is_empty(),
        "{:?}",
        failures.lock().unwrap()
    );
}
