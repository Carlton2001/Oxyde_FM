//! Check both CPU references and the actual atlas retirement while a window is borrowed.
use super::*;
use crate::ui::app::{viewer::Source, OxydeApp};
use std::{cell::RefCell, io::Write, rc::Rc, sync::atomic::AtomicUsize};

struct Probe {
    viewer: Entity<Viewer>,
    _owner: Entity<OxydeApp>,
    shown: usize,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let image = match &self.viewer.read(cx).content {
            Content::Pdf(pdf) => pdf.pages.get(&self.shown).cloned(),
            _ => None,
        };
        div()
            .size_full()
            .children(image.map(|image| img(image).size(px(64.0))))
    }
}

#[::core::prelude::v1::test]
#[ignore = "Requires a native Windows GPUI renderer; run explicitly in isolation"]
fn native_viewer_pdf_eviction_retires_current_window_texture() {
    let removed = Arc::new(AtomicUsize::new(0));
    let counter = removed.clone();
    env_logger::Builder::new()
        .filter_module("gpui_windows::directx_atlas", log::LevelFilter::Debug)
        .format(move |buffer, record| {
            let event = record.args().to_string();
            if event.starts_with("gpu-atlas-remove:") && event.contains("Polychrome") {
                counter.fetch_add(1, Ordering::Relaxed);
            }
            writeln!(buffer, "{event}")
        })
        .try_init()
        .expect("run this native probe in its own test process");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let failures = Rc::new(RefCell::new(Vec::new()));
    let errors = failures.clone();
    gpui_platform::application().run(move |cx| {
        cx.set_global(crate::ui::theme::OxydeTheme::dark());
        cx.set_global(crate::ui::i18n::I18n::default());
        cx.set_global(crate::ui::format::DateFormatPreference(Default::default()));
        let owner = cx.new(|cx| OxydeApp::new(Default::default(), None, false, cx));
        let viewer = cx.new(|cx| {
            Viewer::new(
                owner.clone(),
                "pdf-eviction-probe".into(),
                Source::default(),
                (false, false, false),
                cx,
            )
        });
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::from_pixel(1024, 1024, image::Rgba([30, 60, 90, 255])),
        )]));
        let probe = Arc::downgrade(&image);
        let mut pdf = document();
        pdf.pages.insert(0, image);
        // Page 6 is already decoded so requesting it tests eviction, without
        // starting a real PDF worker or depending on its scheduling.
        for index in 1..=MAX_PAGES {
            pdf.pages.insert(
                index,
                Arc::new(RenderImage::new(vec![image::Frame::new(
                    image::RgbaImage::new(2, 1),
                )])),
            );
        }
        pdf.recent.extend(0..MAX_PAGES);
        viewer.update(cx, |viewer, cx| {
            viewer.replace_content(Content::Pdf(pdf), cx)
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
                        shown: 0,
                    })
                },
            )
            .unwrap();
        cx.defer(move |cx| {
            AnyWindowHandle::from(handle)
                .update(cx, |root, window, cx| {
                    window.draw(cx).clear(cx);
                    let root = root.downcast::<Probe>().unwrap();
                    let viewer = root.read(cx).viewer.clone();
                    viewer.update(cx, |viewer, cx| viewer.request_pdf_page(MAX_PAGES, cx));
                    root.update(cx, |root, cx| {
                        root.shown = MAX_PAGES;
                        cx.notify();
                    });
                    cx.defer(move |cx| {
                        AnyWindowHandle::from(handle)
                            .update(cx, |root, window, cx| {
                                window.draw(cx).clear(cx);
                                window.draw(cx).clear(cx);
                                if probe.upgrade().is_some() {
                                    errors
                                        .borrow_mut()
                                        .push("evicted PDF pixels survived redraw");
                                }
                                if removed.load(Ordering::Relaxed) == 0 {
                                    errors.borrow_mut().push(
                                        "PDF texture was not removed from the current window atlas",
                                    );
                                }
                                let viewer =
                                    root.downcast::<Probe>().unwrap().read(cx).viewer.clone();
                                viewer.update(cx, |viewer, cx| {
                                    viewer.replace_content(Content::None, cx)
                                });
                                window.remove_window();
                                cx.quit();
                            })
                            .unwrap();
                    });
                })
                .unwrap();
        });
    });
    assert!(failures.borrow().is_empty(), "{:?}", failures.borrow());
}
