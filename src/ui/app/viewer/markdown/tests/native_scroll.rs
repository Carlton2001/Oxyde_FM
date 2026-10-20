//! Real GPUI regression: jumping to the end must not require visiting earlier rows.
use super::*;
use crate::ui::app::{viewer::Source, OxydeApp};

struct Probe {
    viewer: Entity<Viewer>,
    _owner: Entity<OxydeApp>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.viewer.clone())
    }
}

fn batch(count: usize) -> Vec<super::super::Block> {
    (0..count)
        .map(|index| super::super::Block {
            kind: super::super::Kind::Paragraph,
            text: format!("Row {index}: {}", "wrapped content ".repeat(5 + index % 20)),
            spans: Vec::new(),
        })
        .collect()
}

fn stage(
    handle: WindowHandle<Probe>,
    step: usize,
    failures: Rc<RefCell<Vec<String>>>,
    cx: &mut App,
) {
    AnyWindowHandle::from(handle)
        .update(cx, |root, window, cx| {
            window.draw(cx).clear(cx);
            let viewer = root.downcast::<Probe>().unwrap().read(cx).viewer.clone();
            viewer.update(cx, |viewer, cx| {
                let maximum = px(viewer.list_scroll_maximum());
                let Content::Markdown(document) = &mut viewer.content else {
                    panic!("missing Markdown")
                };
                if step == 0 || step == 2 {
                    let minimum =
                        if step == 0 { 400.0 } else { 600.0 } * window.rem_size().as_f32();
                    if maximum.as_f32() < minimum {
                        failures.borrow_mut().push(format!(
                            "offscreen rows have no height at step {step}: {maximum:?}"
                        ));
                    }
                    document
                        .list
                        .set_offset_from_scrollbar(point(px(0.0), -maximum));
                } else {
                    // Summing fractional text heights can leave a few thousandths
                    // of a pixel; GPUI's exact end predicate has no tolerance.
                    let remaining = maximum + document.list.scroll_px_offset_for_scrollbar().y;
                    let last = document.list.bounds_for_item(document.list.item_count() - 1);
                    let final_block_visible = last.is_some_and(|bounds| {
                        bounds.bottom() + window.rem_size()
                            <= document.list.viewport_bounds().bottom() + px(0.5)
                    });
                    if remaining.abs() > px(0.5) || !final_block_visible {
                        failures
                            .borrow_mut()
                            .push(format!("one jump did not reach the end at step {step}: {:?}, maximum {maximum:?}, offset {:?}",
                                document.list.is_scrolled_to_end(), document.list.scroll_px_offset_for_scrollbar()));
                    }
                    if step == 1 {
                        let anchor = document.list.logical_scroll_top();
                        document.append_batch(batch(200));
                        let after = document.list.logical_scroll_top();
                        if anchor.item_ix != after.item_ix
                            || anchor.offset_in_item != after.offset_in_item
                        {
                            failures
                                .borrow_mut()
                                .push("append moved the scroll anchor".into());
                        }
                    }
                }
                cx.notify();
            });
            if step == 3 {
                window.remove_window();
                cx.quit();
            } else {
                cx.defer(move |cx| stage(handle, step + 1, failures, cx));
            }
        })
        .unwrap();
}

#[::core::prelude::v1::test]
#[ignore = "Requires a native Windows GPUI renderer; run explicitly in isolation"]
fn native_markdown_scroll_reaches_end_without_visiting_intermediate_blocks() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let failures = Rc::new(RefCell::new(Vec::new()));
    let result = failures.clone();
    gpui_platform::application()
        .with_assets(crate::ui::assets::EmbeddedAssetSource::new())
        .run(move |cx| {
            crate::ui::assets::EmbeddedAssetSource::new()
                .load_fonts(cx)
                .unwrap();
            cx.set_global(crate::ui::theme::OxydeTheme::dark());
            cx.set_global(crate::ui::i18n::I18n::default());
            cx.set_global(crate::ui::format::DateFormatPreference(Default::default()));
            let owner = cx.new(|cx| OxydeApp::new(Default::default(), None, false, cx));
            let viewer = cx.new(|cx| {
                Viewer::new(
                    owner.clone(),
                    "markdown-scroll-probe".into(),
                    Source::default(),
                    (true, true, true),
                    cx,
                )
            });
            viewer.update(cx, |viewer, cx| {
                let mut document = Markdown::empty(false);
                document.append_batch(batch(400));
                viewer.replace_content(Content::Markdown(document), cx);
            });
            let handle = cx
                .open_window(
                    WindowOptions {
                        show: true,
                        focus: false,
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(100.0), px(100.0)),
                            size(px(800.0), px(600.0)),
                        ))),
                        ..Default::default()
                    },
                    |_, cx| {
                        cx.new(|_| Probe {
                            viewer,
                            _owner: owner,
                        })
                    },
                )
                .unwrap();
            cx.defer(move |cx| stage(handle, 0, result, cx));
        });
    assert!(
        failures.borrow().is_empty(),
        "{}",
        failures.borrow().join("\n")
    );
}
