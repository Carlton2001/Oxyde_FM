//! Verify the painted headers without waiting for a corrective second frame.
use crate::ui::app::OxydeApp;
use crate::ui::scale::logical;
use crate::ui::workspace::{LayoutAxis, LayoutNode, ViewMode, WorkspaceState};
use gpui::*;
use std::{cell::RefCell, rc::Rc};

#[::core::prelude::v1::test]
#[ignore = "Requires a native Windows GPUI renderer; run explicitly in isolation"]
fn native_columns_fit_on_first_frame_and_after_resize() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let failures = Rc::new(RefCell::new(Vec::new()));
    let result = failures.clone();
    gpui_platform::application()
        .with_assets(crate::ui::assets::EmbeddedAssetSource::new())
        .run(move |cx| {
            crate::ui::assets::EmbeddedAssetSource::new().load_fonts(cx).unwrap();
            cx.set_global(crate::ui::theme::OxydeTheme::for_id(crate::ui::theme::ThemeId::default()));
            cx.set_global(crate::ui::i18n::I18n::default());
            cx.set_global(crate::ui::format::DateFormatPreference(Default::default()));
            let app = cx.new(|cx| OxydeApp::new(Default::default(), None, false, cx));
            app.update(cx, |state, _| {
                state.handle = Some(app.clone());
                state.state.layout.visit_panes_mut(&mut |pane| {
                    pane.active_tab_mut().view_mode = ViewMode::Details;
                });
            });
            let handle = cx.open_window(WindowOptions {
                show: true, focus: false,
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Point::default(), size(px(1200.0), px(700.0)),
                ))),
                ..Default::default()
            }, |_, _| app).unwrap();
            cx.defer(move |cx| {
                AnyWindowHandle::from(handle).update(cx, |root, window, cx| {
                    let state = root.downcast::<OxydeApp>().unwrap();
                    // A resize can leave the preceding frame's pane dimensions.
                    for stage in ["opening", "resize", "split", "restore", "zoom"] {
                        state.update(cx, |state, cx| {
                            match stage {
                                "opening" => state.pane_sizes.clear(),
                                "resize" => {
                                    let id = state.state.active_pane_id.clone();
                                    state.pane_sizes.insert(id, Size { width: 700.0, height: 600.0 });
                                }
                                "split" => {
                                    state.state.add_panel_right().unwrap();
                                    // Exercise both axes, nesting and unequal weights.
                                    let mut nested = state.state.layout.clone();
                                    if let LayoutNode::Split { id, axis, weights, .. } = &mut nested {
                                        *id = "probe-nested-split".into();
                                        *axis = LayoutAxis::Vertical;
                                        *weights = vec![2.0, 1.0];
                                    }
                                    if let LayoutNode::Split { children, weights, .. } = &mut state.state.layout {
                                        children[0] = nested;
                                        *weights = vec![3.0, 2.0];
                                    }
                                    // Give the cloned panes distinct stable identities.
                                    let mut index = 0;
                                    state.state.layout.visit_panes_mut(&mut |pane| {
                                        pane.id = format!("probe-pane-{index}");
                                        index += 1;
                                    });
                                    state.state.active_pane_id = "probe-pane-0".into();
                                }
                                "restore" => {
                                    let saved = state.state.saved_snapshot(state.sidebar_width);
                                    state.state = WorkspaceState::from_saved_snapshot(saved).unwrap().0;
                                    state.pane_sizes.clear();
                                }
                                "zoom" => state.config.ui_rem_size = 24,
                                _ => unreachable!(),
                            }
                            state.context_anchor_points.clear();
                            cx.notify();
                        });
                        window.draw(cx).clear(cx);
                        let state = state.read(cx);
                        state.state.layout.visit_panes(&mut |pane| {
                            let id = &pane.id;
                            let tab = pane.active_tab();
                            let bounds = state.list_bounds.get(id).expect("painted list bounds");
                            let size = state.pane_sizes.get(id).unwrap();
                            if (size.width - bounds.size.width).abs() > 1.0 {
                                result.borrow_mut().push(format!("{stage}/{id}: predicted width={}, measured={}", size.width, bounds.size.width));
                            }
                            let date = state.context_anchor_points.get(&format!("filter-{id}-date"));
                            let columns = crate::ui::style::DetailsColumnWidths::for_viewport(
                                bounds.size.width, tab.type_col_width, tab.size_col_width, tab.date_col_width,
                            );
                            let expected = bounds.origin.x + logical(8.0) + columns.total() - columns.date;
                            match date {
                                Some(anchor) if (anchor.x.as_f32() - expected).abs() < 2.0 => {},
                                anchor => result.borrow_mut().push(format!("{stage}/{id}: date={anchor:?}, expected x={expected}")),
                            }
                        });
                    }
                    window.remove_window();
                    cx.quit();
                }).unwrap();
            });
        });
    assert!(failures.borrow().is_empty(), "{}", failures.borrow().join("\n"));
}
