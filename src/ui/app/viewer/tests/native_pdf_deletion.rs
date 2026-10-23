//! Exercise the UI release fence with a real Windows file lock.
use super::*;
use crate::ui::app::{viewer::Source, OxydeApp};
use std::{cell::Cell, rc::Rc};

struct LockedSession(Option<std::fs::File>);
impl PdfSession for LockedSession {
    fn close(&mut self) -> Result<(), oxyde_lib::models::CommandError> {
        self.0 = None;
        Ok(())
    }
    fn sizes(&self) -> &[(f32, f32)] {
        &[]
    }
    fn page(&mut self, _: u32) -> Result<image::RgbaImage, oxyde_lib::models::CommandError> {
        self.0.as_ref().expect("open test session").metadata()?;
        Ok(image::RgbaImage::new(1, 1))
    }
}

struct Probe(Entity<Viewer>, Entity<OxydeApp>);
impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Keep the owner and Viewer alive until the asynchronous fence completes.
        let _ = (&self.0, &self.1);
        div()
    }
}

#[::core::prelude::v1::test]
#[ignore = "Runs a real Windows GPUI event loop with a hidden test window"]
fn native_pdf_deletion_waits_for_file_release_and_blocks_reopening() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("selected.pdf");
    std::fs::write(&path, b"private lock fixture").unwrap();
    let lock = oxyde_lib::platform::platform()
        .open_archive_source(&path, false)
        .unwrap();
    assert!(
        std::fs::remove_file(&path).is_err(),
        "fixture must deny deletion while open"
    );
    let completed = Rc::new(Cell::new(false));
    let result = completed.clone();
    gpui_platform::application().run(move |cx| {
        cx.set_global(crate::ui::theme::OxydeTheme::dark());
        cx.set_global(crate::ui::i18n::I18n::default());
        cx.set_global(crate::ui::format::DateFormatPreference(Default::default()));
        let owner = cx.new(|cx| OxydeApp::new(Default::default(), None, false, cx));
        let file = oxyde_lib::models::FileEntry {
            name: "selected.pdf".into(),
            path: path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        owner.update(cx, |app, _| {
            let tab = app.state.active_tab_mut().unwrap();
            tab.path = directory.path().to_string_lossy().into_owned();
            tab.files = Arc::new(vec![file.clone()]);
            tab.cached_filtered_files = tab.files.clone();
            tab.selected_paths.insert(file.path.clone());
        });
        let viewer = cx.new(|cx| {
            Viewer::new(
                owner.clone(),
                "deletion-probe".into(),
                Source::default(),
                (false, false, false),
                cx,
            )
        });
        let source = Source::capture(owner.read(cx), &Source::default());
        let mut pdf = document();
        pdf.path = file.path.clone();
        pdf.session = Arc::new(Mutex::new(Box::new(LockedSession(Some(lock)))));
        // Obsolete references must not retain an open document. Closing the
        // native session is independent of the Arc's final reference count.
        let retained_session = pdf.session.clone();
        viewer.update(cx, |viewer, cx| {
            viewer.source = source;
            viewer.file = Some(file);
            viewer.replace_content(Content::Pdf(pdf), cx);
        });
        let released = owner.update(cx, |app, cx| {
            app.viewers.insert("deletion-probe".into(), viewer.clone());
            app.suspend_viewers_for_deletion("operation", std::slice::from_ref(&path), cx)
        });
        viewer.update(cx, |viewer, cx| {
            viewer.load_current(cx);
            assert!(
                matches!(viewer.content, Content::None),
                "pending deletion reopened its PDF"
            );
        });
        let handle = cx
            .open_window(
                WindowOptions {
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Probe(viewer, owner)),
            )
            .unwrap();
        cx.spawn(async move |cx| {
            released.await;
            cx.background_executor()
                .spawn(async move {
                    std::fs::remove_file(&path)
                        .expect("PDF session must release its file before deletion");
                    drop(retained_session);
                    drop(directory);
                })
                .await;
            cx.update(|cx| {
                result.set(true);
                handle
                    .update(cx, |_, window, _| window.remove_window())
                    .unwrap();
                cx.quit();
            });
        })
        .detach();
    });
    assert!(completed.get(), "release fence did not complete");
}
