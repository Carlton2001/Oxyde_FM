use super::draw_controls;
use gpui::{prelude::*, div, point, px, size, AnyWindowHandle, Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowOptions};
use std::{cell::Cell, rc::Rc};

struct RenderProbe(Rc<Cell<usize>>);

impl Render for RenderProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.set(self.0.get() + 1);
        div().size_full()
    }
}

#[test]
#[ignore = "Runs a real Windows GPUI event loop with a hidden test window"]
fn hidden_controls_draw_and_redraw_without_borrowing_the_root_recursively() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let renders = Rc::new(Cell::new(0));
    let result = renders.clone();
    gpui_platform::application().run(move |cx| {
        let root_count = result.clone();
        let handle = cx.open_window(WindowOptions {
            show: false,
            focus: false,
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(0.0), px(0.0)), size(px(320.0), px(64.0)),
            ))),
            ..Default::default()
        }, |_, cx| cx.new(|_| RenderProbe(root_count))).unwrap();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
            cx.update(move |cx| {
            let window = AnyWindowHandle::from(handle);
            result.set(0);
            eprintln!("Drawing hidden controls through an untyped window handle");
            draw_controls(window, cx);
            let initial = result.get();
            assert!(initial > 0, "Hidden first draw must render its root");
            draw_controls(window, cx);
            assert!(result.get() > initial, "Waking controls must redraw its root");
            eprintln!("Initial draw and redraw completed without recursive root borrowing");
            window.update(cx, |_, window, _| window.remove_window()).unwrap();
            cx.quit();
            });
        }).detach();
    });
    assert!(renders.get() >= 2);
}
