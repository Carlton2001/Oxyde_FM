//! Opt-in real GPUI render/lifecycle test, using only private fixture state.
use super::{window::ViewerWindow, OxydeApp, Source, Viewer};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc, sync::Arc};

fn check(failures: &Rc<RefCell<Vec<String>>>, condition: bool, message: &str) {
    if !condition {
        failures.borrow_mut().push(message.into());
    }
}

fn stage(
    handle: WindowHandle<OxydeApp>,
    viewer: Entity<Viewer>,
    source_pane: String,
    step: usize,
    failures: Rc<RefCell<Vec<String>>>,
    cx: &mut App,
) {
    AnyWindowHandle::from(handle)
        .update(cx, |root, window, cx| {
            window.draw(cx).clear(cx);
            let app = root.downcast::<OxydeApp>().expect("Viewer probe root");
            if step == 0 {
                check(
                    &failures,
                    viewer.read(cx).files.len() == 2,
                    "initial multi-selection missing",
                );
                viewer.update(cx, |viewer, cx| viewer.step(true, cx));
                check(
                    &failures,
                    viewer.read(cx).index == 1,
                    "next file did not advance",
                );
                let mut refreshed = viewer.read(cx).source.clone();
                let mut refreshed_files = refreshed.files.as_ref().clone();
                refreshed_files.extend((0..2048).map(|index| oxyde_lib::models::FileEntry {
                    name: format!("extra-{index}.unknown"),
                    path: format!("extra-{index}.unknown"),
                    ..Default::default()
                }));
                refreshed.files = Arc::new(refreshed_files);
                viewer.update(cx, |viewer, cx| viewer.set_source(refreshed, cx));
                check(
                    &failures,
                    viewer
                        .read(cx)
                        .files
                        .first()
                        .is_some_and(|file| file.path == "second.unknown"),
                    "large listing refresh lost the current selected file",
                );
                // Queue a large obsolete selection before the real explorer change.
                // Its asynchronous result must never replace the latest selection.
                let stale = Source {
                    tab: "obsolete-source".into(),
                    selected: Arc::new(["obsolete-0.unknown".into()].into()),
                    files: Arc::new(
                        (0..2048)
                            .map(|index| oxyde_lib::models::FileEntry {
                                name: format!("obsolete-{index}.unknown"),
                                path: format!("obsolete-{index}.unknown"),
                                ..Default::default()
                            })
                            .collect(),
                    ),
                };
                viewer.update(cx, |viewer, cx| viewer.set_source(stale, cx));
                app.update(cx, |app, cx| {
                    app.state.active_pane_id = source_pane.clone();
                    let tab = app
                        .state
                        .find_pane_mut(&source_pane)
                        .unwrap()
                        .active_tab_mut();
                    tab.selected_paths = ["second.unknown".into()].into();
                    cx.notify();
                });
            } else if step == 1 {
                check(
                    &failures,
                    viewer.read(cx).files.len() == 1 && viewer.read(cx).index == 0,
                    "live selection did not replace the sequence",
                );
                viewer.update(cx, |viewer, cx| viewer.detach(window, cx));
            } else if step == 2 {
                check(&failures, viewer.read(cx).detached, "Viewer did not detach");
                check(
                    &failures,
                    app.read(cx).viewers.len() == 1,
                    "detaching removed the Viewer tab",
                );
                let tab_id = &viewer.read(cx).tab_id;
                let mut panes = 0;
                let mut viewer_panes = 0;
                app.read(cx).state.layout.visit_panes(&mut |pane| {
                    panes += 1;
                    if pane.tabs.iter().any(|tab| &tab.id == tab_id) {
                        viewer_panes += 1;
                        check(&failures, pane.tabs.len() == 1, "Viewer shares its tab bar");
                    }
                });
                check(
                    &failures,
                    panes == 3 && viewer_panes == 1,
                    "detaching changed the pane layout",
                );
                check(
                    &failures,
                    app.read(cx).state.active_pane_id == source_pane,
                    "detaching changed the active explorer",
                );
                viewer.update(cx, |viewer, cx| viewer.detach(window, cx));
                app.update(cx, |app, cx| {
                    app.state.active_pane_id = source_pane.clone();
                    let tab = app
                        .state
                        .find_pane_mut(&source_pane)
                        .unwrap()
                        .active_tab_mut();
                    tab.selected_paths = ["first.unknown".into()].into();
                    cx.notify();
                });
            } else if step == 3 {
                check(
                    &failures,
                    cx.windows()
                        .iter()
                        .filter(|handle| handle.downcast::<ViewerWindow>().is_some())
                        .count()
                        == 1,
                    "showing the detached Viewer created a second window",
                );
                check(
                    &failures,
                    viewer
                        .read(cx)
                        .files
                        .first()
                        .is_some_and(|file| file.path == "first.unknown"),
                    "detached Viewer stopped following selection",
                );
                for secondary in cx.windows() {
                    if let Some(secondary) = secondary.downcast::<ViewerWindow>() {
                        let _ = AnyWindowHandle::from(secondary).update(cx, |_, window, cx| {
                            window.draw(cx).clear(cx);
                            window.remove_window();
                        });
                    }
                }
            } else if step == 4 {
                check(
                    &failures,
                    !viewer.read(cx).detached && viewer.read(cx).detached_window.is_none(),
                    "closing the secondary window did not restore inline rendering",
                );
                check(
                    &failures,
                    app.read(cx).viewers.len() == 1,
                    "closing the secondary window removed the Viewer tab",
                );
                window.draw(cx).clear(cx);
                viewer.update(cx, |viewer, cx| viewer.detach(window, cx));
            } else if step == 5 {
                check(
                    &failures,
                    viewer.read(cx).detached,
                    "Viewer did not detach again",
                );
                let tab_id = viewer.read(cx).tab_id.clone();
                app.update(cx, |app, cx| {
                    let mut viewer_pane = None;
                    app.state.layout.visit_panes(&mut |pane| {
                        if pane.tabs.iter().any(|tab| tab.id == tab_id) {
                            viewer_pane = Some(pane.id.clone());
                        }
                    });
                    if let Some(pane) = viewer_pane {
                        app.close_tab(&pane, 0, cx);
                    }
                });
                window.draw(cx).clear(cx);
            } else {
                check(
                    &failures,
                    cx.windows()
                        .iter()
                        .all(|handle| handle.downcast::<ViewerWindow>().is_none()),
                    "closing the tab left its secondary window open",
                );
                app.update(cx, |app, _| app.handle = None);
                window.remove_window();
                cx.quit();
                return;
            }
            cx.defer(move |cx| stage(handle, viewer, source_pane, step + 1, failures, cx));
        })
        .expect("Viewer probe window");
}

#[::core::prelude::v1::test]
#[ignore = "Requires a native Windows GPUI renderer; run explicitly in isolation"]
fn native_viewer_renders_tracks_selection_and_detaches() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().to_string_lossy().into_owned();
    let failures = Rc::new(RefCell::new(Vec::new()));
    let result = failures.clone();
    gpui_platform::application()
        .with_assets(crate::ui::assets::EmbeddedAssetSource::new())
        .run(move |cx| {
            crate::ui::assets::EmbeddedAssetSource::new()
                .load_fonts(cx)
                .unwrap();
            cx.set_global(crate::ui::theme::OxydeTheme::for_id(
                crate::ui::theme::ThemeId::from_id("dark"),
            ));
            cx.set_global(crate::ui::i18n::I18n::default());
            cx.set_global(crate::ui::format::DateFormatPreference(Default::default()));
            let app = cx.new(|cx| OxydeApp::new(Default::default(), None, false, cx));
            let (viewer, source) = app.update(cx, |app_state, cx| {
                app_state.handle = Some(app.clone());
                app_state.state.pinned_items.clear();
                app_state.state.onedrive_items.clear();
                app_state.state.drives.clear();
                app_state.state.sidebar_items.clear();
                let initial = app_state.state.active_pane_id.clone();
                app_state
                    .state
                    .find_pane_mut(&initial)
                    .unwrap()
                    .active_tab_mut()
                    .path = path.clone();
                let source = app_state.state.add_panel_right().unwrap();
                let tab = app_state
                    .state
                    .find_pane_mut(&source)
                    .unwrap()
                    .active_tab_mut();
                tab.path = path.clone();
                tab.files = Arc::new(
                    ["first.unknown", "second.unknown"]
                        .map(|name| oxyde_lib::models::FileEntry {
                            name: name.into(),
                            path: name.into(),
                            ..Default::default()
                        })
                        .into(),
                );
                tab.cached_filtered_files = tab.files.clone();
                tab.selected_paths = ["first.unknown".into(), "second.unknown".into()].into();
                let source_id = tab.id.clone();
                app_state.open_viewer(cx);
                let viewer = app_state.viewers.values().next().unwrap().clone();
                check(
                    &result,
                    viewer.read(cx).source.tab == source_id,
                    "Viewer chose the wrong explorer pane",
                );
                (viewer, source)
            });
            let handle = cx
                .open_window(
                    WindowOptions {
                        show: true,
                        focus: false,
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(100.0), px(100.0)),
                            size(px(1000.0), px(700.0)),
                        ))),
                        ..Default::default()
                    },
                    |_, _| app,
                )
                .unwrap();
            cx.defer(move |cx| stage(handle, viewer, source, 0, result, cx));
        });
    assert!(
        failures.borrow().is_empty(),
        "{}",
        failures.borrow().join("\n")
    );
}
