use super::*;
use crate::{
    models::transaction::HistoryFileIdentity,
    platform::{platform, PdfSession, ShellDeleteControl},
    systems::path_parents::HeldParents,
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

struct Fixture {
    path: PathBuf,
    _root: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("viewer-recycle.pdf");
        super::tests::write_pdf(&path);
        Self { path, _root: root }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Restore only this generated fixture, including on assertion failure.
        let ids: Vec<_> = crate::commands::ops::list_trash()
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| std::path::Path::new(&entry.original_path) == self.path)
            .map(|entry| entry.path)
            .collect();
        if !ids.is_empty() {
            let result = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(crate::commands::ops::restore_items(ids));
            assert!(result.is_ok(), "Restore generated PDF fixture: {result:?}");
        }
    }
}

#[test]
fn native_pdf_recycles_after_viewer_session_closes_with_source_identity_retained() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("oxyde.exe");
    for viewed in [false, true] {
        let fixture = Fixture::new();
        let retained_session = if viewed {
            let mut session = Session::open_in(&executable, &fixture.path).unwrap();
            session.page(0).unwrap();
            session.close().unwrap();
            assert!(
                session.page(0).is_err(),
                "A closed session must never reopen its document"
            );
            session.close().unwrap(); // Closing twice is harmless.
            Some(session)
        } else {
            None
        };
        let parents = HeldParents::new(&fixture.path).unwrap();
        let source = platform()
            .track_source_item(
                parents.directory(),
                &parents.resolved_path(&fixture.path).unwrap(),
            )
            .unwrap();
        let proof = HistoryFileIdentity::from_file(&source).unwrap();
        let control = ShellDeleteControl {
            cancel: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
            turbo: Arc::new(AtomicBool::new(true)),
        };
        let outcomes =
            platform().shell_delete_files_tracked(&[(fixture.path.clone(), proof)], control);
        assert!(
            outcomes[0].completed,
            "viewed={viewed}: {:?}",
            outcomes[0].error
        );
        assert!(outcomes[0].error.is_none(), "{:?}", outcomes[0].error);
        assert!(!fixture.path.exists());
        drop(source);
        drop(parents);
        drop(retained_session);
    }
}
