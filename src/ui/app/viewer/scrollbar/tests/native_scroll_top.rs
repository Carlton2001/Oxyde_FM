use super::*;
use crate::ui::app::{viewer::{content::{Block, BlockKind}, Source}, OxydeApp};
use std::{cell::RefCell, rc::Rc, sync::Arc};

struct Probe {
    viewer: Entity<Viewer>,
    _owner: Entity<OxydeApp>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.viewer.clone())
    }
}

fn stage(handle: WindowHandle<Probe>, step: usize, failures: Rc<RefCell<Vec<String>>>, cx: &mut App) {
    AnyWindowHandle::from(handle).update(cx, |root, window, cx| {
        window.draw(cx).clear(cx);
        let viewer = root.downcast::<Probe>().unwrap().read(cx).viewer.clone();
        let bounds = viewer.read(cx).list_state().unwrap().viewport_bounds();
        let center = point(bounds.origin.x + bounds.size.width / 2.0, bounds.origin.y + bounds.size.height / 2.0);
        let near = point(bounds.origin.x + bounds.size.width + ZED_TRACK_WIDTH.to_pixels(window.rem_size()) / 2.0, center.y);
        if step == 0 {
            window.dispatch_event(MouseMoveEvent { position: center, ..Default::default() }.to_platform_input(), cx);
            viewer.update(cx, |viewer, cx| {
                viewer.list_state().unwrap().set_offset_from_scrollbar(point(px(0.0), px(-500.0)));
                cx.notify();
            });
        } else if step == 1 {
            if viewer.read(cx).scroll.hovered || viewer.read(cx).scroll.top_anchor.get() != 0.0 {
                failures.borrow_mut().push("button appeared away from the scrollbar".into());
            }
            window.dispatch_event(MouseMoveEvent { position: near, ..Default::default() }.to_platform_input(), cx);
        } else if step == 2 {
            if !viewer.read(cx).scroll.hovered || viewer.read(cx).scroll.top_anchor.get() <= 0.0 {
                failures.borrow_mut().push("button missing near the scrollbar".into());
            }
            window.dispatch_event(MouseMoveEvent { position: center, ..Default::default() }.to_platform_input(), cx);
        } else if step == 3 {
            if viewer.read(cx).scroll.hovered {
                failures.borrow_mut().push("button remained active away from the scrollbar".into());
            }
            window.dispatch_event(MouseMoveEvent { position: near, ..Default::default() }.to_platform_input(), cx);
        } else if step == 4 {
            let top = viewer.read(cx).scroll.top_anchor.get();
            let position = point(bounds.origin.x + bounds.size.width + ZED_TRACK_WIDTH.to_pixels(window.rem_size()) - rems(2.0).to_pixels(window.rem_size()),
                bounds.origin.y + px(top) + rems(1.0).to_pixels(window.rem_size()));
            window.dispatch_event(MouseDownEvent { button: MouseButton::Left, position, click_count: 1, ..Default::default() }.to_platform_input(), cx);
        } else {
            let offset = viewer.read(cx).list_state().unwrap().scroll_px_offset_for_scrollbar().y;
            if offset.abs() > px(0.5) {
                failures.borrow_mut().push(format!("return-to-top click did not reset the list: {offset:?}"));
            }
            window.remove_window();
            cx.quit();
            return;
        }
        cx.defer(move |cx| stage(handle, step + 1, failures, cx));
    }).unwrap();
}

#[::core::prelude::v1::test]
#[ignore = "Requires a native Windows GPUI renderer; run explicitly in isolation"]
fn native_viewer_scroll_top_follows_proximity_and_returns_to_start() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let failures = Rc::new(RefCell::new(Vec::new()));
    let result = failures.clone();
    gpui_platform::application().with_assets(crate::ui::assets::EmbeddedAssetSource::new()).run(move |cx| {
        crate::ui::assets::EmbeddedAssetSource::new().load_fonts(cx).unwrap();
        cx.set_global(crate::ui::theme::OxydeTheme::dark());
        cx.set_global(crate::ui::i18n::I18n::default());
        cx.set_global(crate::ui::format::DateFormatPreference(Default::default()));
        let owner = cx.new(|cx| OxydeApp::new(Default::default(), None, false, cx));
        let viewer = cx.new(|cx| Viewer::new(owner.clone(), "scroll-probe".into(), Source::default(), (true, true, true), cx));
        viewer.update(cx, |viewer, cx| {
            let blocks = (0..200).map(|index| Block { kind: BlockKind::Paragraph, text: format!("Line {index}").into(), cells: Vec::new() }).collect::<Vec<_>>();
            viewer.replace_content(Content::Document(Arc::new(blocks), ListState::new(200, ListAlignment::Top, px(800.0)).measure_all()), cx);
        });
        let handle = cx.open_window(WindowOptions {
            show: true, focus: false,
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(point(px(100.0), px(100.0)), size(px(800.0), px(600.0))))),
            ..Default::default()
        }, |_, cx| cx.new(|_| Probe { viewer, _owner: owner })).unwrap();
        cx.defer(move |cx| stage(handle, 0, result, cx));
    });
    assert!(failures.borrow().is_empty(), "{}", failures.borrow().join("\n"));
}
