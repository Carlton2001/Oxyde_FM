//! Verify pixel ownership ends after clearing content inside a window callback.
use super::{Content, Viewer};
use crate::ui::app::{viewer::Source, OxydeApp};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc};

struct Probe {
    viewer: Entity<Viewer>,
    _owner: Entity<OxydeApp>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let image = match &self.viewer.read(cx).content {
            Content::Image(image) => Some(image.clone()),
            _ => None,
        };
        div().size_full().children(image.map(|image| img(image).size(px(64.0))))
    }
}

#[::core::prelude::v1::test]
#[ignore = "Requires a native Windows GPUI renderer; run explicitly in isolation"]
fn native_viewer_releases_image_from_current_window() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let failures = Rc::new(RefCell::new(Vec::new()));
    let result = failures.clone();
    gpui_platform::application().run(move |cx| {
        cx.set_global(crate::ui::theme::OxydeTheme::dark());
        cx.set_global(crate::ui::i18n::I18n::default());
        cx.set_global(crate::ui::format::DateFormatPreference(Default::default()));
        let owner = cx.new(|cx| OxydeApp::new(Default::default(), None, false, cx));
        let viewer = cx.new(|cx| Viewer::new(
            owner.clone(), "image-cleanup-probe".into(), Source::default(),
            (false, false, false), cx,
        ));
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::from_pixel(64, 64, image::Rgba([30, 60, 90, 255])),
        )]));
        viewer.update(cx, |viewer, cx| viewer.replace_content(Content::Image(image.clone()), cx));
        let pixels = Arc::downgrade(&image);
        drop(image);
        let handle = cx.open_window(WindowOptions {
            show: true,
            focus: false,
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(100.0), px(100.0)), size(px(200.0), px(150.0)),
            ))),
            ..Default::default()
        }, |_, cx| cx.new(|_| Probe { viewer, _owner: owner })).unwrap();
        cx.defer(move |cx| {
            AnyWindowHandle::from(handle).update(cx, |root, window, cx| {
                window.draw(cx).clear(cx);
                assert!(pixels.upgrade().is_some(), "fixture pixels must be alive before cleanup");
                let viewer = root.downcast::<Probe>().unwrap().read(cx).viewer.clone();
                viewer.update(cx, |viewer, cx| viewer.replace_content(Content::None, cx));
                // Inspect only after deferred cleanup has run and restored this window.
                cx.defer(move |cx| {
                    AnyWindowHandle::from(handle).update(cx, |_, window, cx| {
                        // Both retained frame buffers can own render closures.
                        window.draw(cx).clear(cx);
                        window.draw(cx).clear(cx);
                        if pixels.upgrade().is_some() {
                            result.borrow_mut().push("empty Viewer retained CPU image pixels after cleanup and redraw");
                        }
                        window.remove_window();
                        cx.quit();
                    }).unwrap();
                });
            }).unwrap();
        });
    });
    assert!(failures.borrow().is_empty(), "{}", failures.borrow().join("\n"));
}
