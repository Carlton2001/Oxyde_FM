//! Controlled atlas/driver probe with real GPUI frames and no source files.
use super::{Content, Viewer};
use crate::ui::app::{viewer::Source, OxydeApp};
use gpui::{prelude::*, *};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Weak},
    time::Duration,
};

struct Probe {
    viewer: Entity<Viewer>,
    _owner: Entity<OxydeApp>,
    pixels: Vec<Weak<RenderImage>>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let image = match &self.viewer.read(cx).content {
            Content::Image(image) => Some(image.clone()),
            _ => None,
        };
        div()
            .size_full()
            .children(image.map(|image| img(image).size(px(128.0))))
    }
}

fn measure_then(
    cx: &mut App,
    stage: &'static str,
    delay: Duration,
    next: impl FnOnce(&mut App) + 'static,
) {
    cx.spawn(async move |cx| {
        cx.background_executor().timer(delay).await;
        let memory = cx
            .background_executor()
            .spawn(async {
                oxyde_lib::platform::platform()
                    .process_memory_snapshot()
                    .unwrap()
            })
            .await;
        println!(
            "gpui-memory: stage={stage}; private_bytes={}; working_set_bytes={}",
            memory.private_bytes, memory.working_set_bytes
        );
        cx.update(next);
    })
    .detach();
}

fn step(
    handle: WindowHandle<Probe>,
    index: usize,
    failures: Rc<RefCell<Vec<String>>>,
    cx: &mut App,
) {
    AnyWindowHandle::from(handle)
        .update(cx, |root, window, cx| {
            window.draw(cx).clear(cx);
            let root = root.downcast::<Probe>().unwrap();
            let viewer = root.read(cx).viewer.clone();
            if index < 8 {
                // Large allocation is private test-fixture setup, not product rendering.
                let image = Arc::new(RenderImage::new(vec![image::Frame::new(
                    image::RgbaImage::from_pixel(
                        4000,
                        4000,
                        image::Rgba([index as u8 * 20, 60, 90, 255]),
                    ),
                )]));
                root.update(cx, |root, cx| {
                    root.pixels.push(Arc::downgrade(&image));
                    cx.notify();
                });
                viewer.update(cx, |viewer, cx| {
                    viewer.replace_content(Content::Image(image), cx)
                });
                // Separate event turns let deferred atlas eviction run between images.
                cx.defer(move |cx| {
                    measure_then(cx, "image-painted", Duration::from_millis(100), move |cx| {
                        step(handle, index + 1, failures, cx)
                    });
                });
            } else {
                viewer.update(cx, |viewer, cx| viewer.replace_content(Content::None, cx));
                root.update(cx, |_, cx| cx.notify());
                cx.defer(move |cx| {
                    AnyWindowHandle::from(handle)
                        .update(cx, |root, window, cx| {
                            window.draw(cx).clear(cx);
                            window.draw(cx).clear(cx);
                            if root
                                .downcast::<Probe>()
                                .unwrap()
                                .read(cx)
                                .pixels
                                .iter()
                                .any(|image| image.upgrade().is_some())
                            {
                                failures
                                    .borrow_mut()
                                    .push("cleared Viewer still owns source pixels".into());
                            }
                            measure_then(
                                cx,
                                "cleared-idle-30s",
                                Duration::from_secs(30),
                                move |cx| {
                                    AnyWindowHandle::from(handle)
                                        .update(cx, |_, window, cx| {
                                            window.remove_window();
                                            cx.quit();
                                        })
                                        .unwrap();
                                },
                            );
                        })
                        .unwrap();
                });
            }
        })
        .unwrap();
}

#[::core::prelude::v1::test]
#[ignore = "Native GPU memory diagnostic; run alone with --nocapture"]
fn native_viewer_large_image_graphics_memory() {
    let _ = env_logger::Builder::new()
        .filter_module("gpui_windows::directx_atlas", log::LevelFilter::Debug)
        .try_init();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let failures = Rc::new(RefCell::new(Vec::new()));
    let result = failures.clone();
    gpui_platform::application().run(move |cx| {
        cx.set_global(crate::ui::theme::OxydeTheme::dark());
        cx.set_global(crate::ui::i18n::I18n::default());
        cx.set_global(crate::ui::format::DateFormatPreference(Default::default()));
        let owner = cx.new(|cx| OxydeApp::new(Default::default(), None, false, cx));
        let viewer = cx.new(|cx| {
            Viewer::new(
                owner.clone(),
                "graphics-probe".into(),
                Source::default(),
                (false, false, false),
                cx,
            )
        });
        let handle = cx
            .open_window(
                WindowOptions {
                    show: true,
                    focus: false,
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(100.0), px(100.0)),
                        size(px(200.0), px(150.0)),
                    ))),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| Probe {
                        viewer,
                        _owner: owner,
                        pixels: Vec::new(),
                    })
                },
            )
            .unwrap();
        cx.defer(move |cx| {
            measure_then(cx, "empty-window", Duration::from_millis(250), move |cx| {
                step(handle, 0, result, cx)
            })
        });
    });
    assert!(
        failures.borrow().is_empty(),
        "{}",
        failures.borrow().join("\n")
    );
}
