//! Native GPUI dispatch plus the real background file-operation pipeline.
use super::*;
use crate::ui::app::FileDropTargetExt;
use std::{cell::RefCell, rc::Rc, time::Duration};

struct DropProbe {
    app: Entity<OxydeApp>,
    target: String,
}
impl Render for DropProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .external_file_drop(self.app.clone(), self.target.clone())
    }
}

#[::core::prelude::v1::test]
#[ignore = "Opens a native GPUI window; run explicitly in isolation"]
fn native_external_multiselection_copies_then_moves_once() {
    let _ = env_logger::builder().is_test(true).try_init();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _runtime = runtime.enter();
    let fixture = tempfile::tempdir().unwrap();
    let base = fixture.path().to_path_buf();
    let source = base.join("source");
    std::fs::create_dir(&source).unwrap();
    for name in ["one.txt", "école.txt"] {
        std::fs::write(source.join(name), name).unwrap();
    }
    for name in ["copy", "move"] {
        std::fs::create_dir(base.join(name)).unwrap();
    }
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
            app.update(cx, |app, cx| app.handle = Some(cx.entity()));
            let handle = cx
                .open_window(
                    WindowOptions {
                        show: true,
                        focus: false,
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            Point::default(),
                            size(px(320.0), px(160.0)),
                        ))),
                        ..Default::default()
                    },
                    |_, cx| {
                        cx.new(|_| DropProbe {
                            app,
                            target: String::new(),
                        })
                    },
                )
                .unwrap();
            cx.spawn(async move |cx| {
                for (directory, control, shift) in [("copy", true, false), ("move", false, true)] {
                    let paths = ["one.txt", "école.txt"]
                        .into_iter()
                        .map(|name| source.join(name).to_string_lossy().into_owned())
                        .collect::<Vec<_>>();
                    let target = base.join(directory).to_string_lossy().into_owned();
                    let actual = AnyWindowHandle::from(handle)
                        .update(cx, |root, window, cx| {
                            let probe = root.downcast::<DropProbe>().unwrap();
                            let app = probe.update(cx, |probe, _| {
                                probe.target = target;
                                probe.app.clone()
                            });
                            window.refresh();
                            window.draw(cx).clear(cx);
                            let position = [40, 40];
                            let make = |phase| NativeFileDrop {
                                phase,
                                paths: paths.clone(),
                                position,
                                control,
                                shift,
                                can_copy: true,
                                can_move: true,
                            };
                            receive(&app, make(FileDropPhase::Enter), window, cx);
                            receive(&app, make(FileDropPhase::Over), window, cx);
                            receive(&app, make(FileDropPhase::Drop), window, cx)
                        })
                        .unwrap();
                    let expected = if control {
                        FileDropEffect::Copy
                    } else {
                        FileDropEffect::Move
                    };
                    if actual != expected {
                        result.borrow_mut().push(format!(
                            "{directory}: expected {expected:?}, got {actual:?}"
                        ));
                    }
                    let mut completed = false;
                    for _ in 0..400 {
                        let base = base.clone();
                        completed = tokio::task::spawn_blocking(move || {
                            ["one.txt", "école.txt"].iter().all(|name| {
                                std::fs::read_to_string(base.join(directory).join(name))
                                    .ok()
                                    .as_deref()
                                    == Some(*name)
                                    && base.join("source").join(name).exists() == control
                            })
                        })
                        .await
                        .unwrap();
                        if completed {
                            break;
                        }
                        cx.background_executor()
                            .timer(Duration::from_millis(25))
                            .await;
                    }
                    if !completed {
                        let diagnostic = handle
                            .update(cx, |probe, _, cx| {
                                let app = probe.app.read(cx);
                                format!(
                                    "notice={:?}; pending={}; operations={:?}",
                                    app.file_operation_notice,
                                    app.pending_file_operations.len(),
                                    app.file_ops
                                        .list_operations()
                                        .iter()
                                        .map(|op| (&op.status, &op.sources, &op.destination))
                                        .collect::<Vec<_>>()
                                )
                            })
                            .unwrap();
                        result.borrow_mut().push(format!(
                            "{directory}: transfer did not finish correctly; {diagnostic}"
                        ));
                        break;
                    }
                    // Let a mistakenly duplicated preparation reach its conflict dialog.
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let pending = handle
                        .update(cx, |probe, _, cx| {
                            probe.app.read(cx).pending_file_operations.len()
                        })
                        .unwrap();
                    if pending != 0 {
                        result.borrow_mut().push(format!(
                            "{directory}: duplicate/conflicting operation queued"
                        ));
                    }
                }
                let _ = handle.update(cx, |_, window, cx| {
                    window.remove_window();
                    cx.quit();
                });
            })
            .detach();
        });
    assert!(
        failures.borrow().is_empty(),
        "{}",
        failures.borrow().join("\n")
    );
}
