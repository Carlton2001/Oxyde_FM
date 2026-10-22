//! Opt-in native GPUI regression: exercises dispatch, observations and ghost rendering.
use super::*;
use std::{cell::RefCell, rc::Rc};

struct Probe {
    app: Entity<OxydeApp>,
    preview: Option<Entity<FileDragPreview>>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = cx.entity();
        let app = self.app.clone();
        div().size_full().child(self.app.clone()).child(
            div().id("drag-probe-source").absolute().top_0().left_0().size(px(80.0))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_drag(super::super::FileDragData {
                    paths: vec!["C:/probe.txt".into()], source_pane_id: "probe".into(),
                }, move |_, offset, window, cx| {
                    let ghost = cx.new(|cx| FileDragPreview::new(
                        vec!["C:/probe.txt".into()], offset, false, app.clone(), window, cx,
                    ));
                    source.update(cx, |probe, _| probe.preview = Some(ghost.clone()));
                    ghost
                }),
        )
    }
}

fn step(handle: WindowHandle<Probe>, stage: usize, failures: Rc<RefCell<Vec<String>>>, cx: &mut App) {
    AnyWindowHandle::from(handle).update(cx, |root, window, cx| {
        window.draw(cx).clear(cx);
        let position = point(px(30.0), px(30.0));
        if stage == 0 {
            window.dispatch_event(MouseDownEvent {
                button: MouseButton::Left, position, click_count: 1, ..Default::default()
            }.to_platform_input(), cx);
            window.dispatch_event(MouseMoveEvent {
                position: point(px(50.0), px(50.0)), pressed_button: Some(MouseButton::Left),
                ..Default::default()
            }.to_platform_input(), cx);
        } else {
            let probe = root.downcast::<Probe>().expect("probe root");
            let probe = probe.read(cx);
            if !super::native::has_ghost(cx) {
                failures.borrow_mut().push(format!("stage {stage}: missing independent ghost window"));
            }
            let expected = match stage { 1 | 3 | 5 => false, _ => true };
            let mode = probe.app.read(cx).drag_copy_mode;
            let rendered = probe.preview.as_ref().and_then(|ghost| ghost.read(cx).rendered_copy_mode);
            if mode != expected || rendered != Some(expected) || !cx.has_active_drag() {
                failures.borrow_mut().push(format!(
                    "stage {stage}: expected copy={expected}; model={mode}; ghost={rendered:?}; active={}",
                    cx.has_active_drag(),
                ));
            }
            let modifiers = match stage {
                1 | 3 => Modifiers { control: true, ..Default::default() },
                2 => Modifiers { shift: true, ..Default::default() },
                _ => Modifiers::default(),
            };
            window.dispatch_event(ModifiersChangedEvent { modifiers, ..Default::default() }.to_platform_input(), cx);
        }
        if stage == 5 {
            cx.stop_active_drag(window);
            super::native::close(cx);
            window.remove_window();
            cx.quit();
        } else {
            let failures = failures.clone();
            cx.defer(move |cx| step(handle, stage + 1, failures, cx));
        }
    }).expect("probe window update");
}

#[::core::prelude::v1::test]
#[ignore = "Requires a native Windows GPUI renderer; run explicitly in isolation"]
fn native_drag_ghost_tracks_modifiers_without_pointer_movement() {
    let runtime = tokio::runtime::Runtime::new().expect("Tokio runtime");
    let _runtime = runtime.enter();
    let failures = Rc::new(RefCell::new(Vec::new()));
    let result = failures.clone();
    gpui_platform::application().with_assets(crate::ui::assets::EmbeddedAssetSource::new()).run(move |cx| {
        crate::ui::assets::EmbeddedAssetSource::new().load_fonts(cx).expect("fonts");
        cx.set_global(OxydeTheme::for_id(crate::ui::theme::ThemeId::from_id("dark")));
        cx.set_global(crate::ui::i18n::I18n::default());
        cx.set_global(crate::ui::format::DateFormatPreference(Default::default()));
        let app = cx.new(|cx| OxydeApp::new(Default::default(), None, false, cx));
        app.update(cx, |this, _| this.handle = Some(app.clone()));
        let handle = cx.open_window(WindowOptions {
            show: true, focus: false,
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(Point::default(), size(px(800.0), px(600.0))))),
            ..Default::default()
        }, |_, cx| cx.new(|_| Probe { app, preview: None })).expect("probe window");
        cx.defer(move |cx| step(handle, 0, result, cx));
    });
    assert!(failures.borrow().is_empty(), "{}", failures.borrow().join("\n"));
}
