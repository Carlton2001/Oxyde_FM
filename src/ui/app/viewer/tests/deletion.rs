use super::*;

#[::core::prelude::v1::test]
fn deletion_matches_the_file_and_descendants_but_preserves_siblings() {
    let paths = vec![
        PathBuf::from("C:/docs/report.pdf"),
        PathBuf::from("C:/photos"),
    ];
    assert!(contains(&paths, "C:/docs/report.pdf"));
    assert!(contains(&paths, "C:/photos/album/image.png"));
    assert!(!contains(&paths, "C:/photos-other/image.png"));
    assert!(!contains(&paths, "C:/docs/report.pdf.backup"));
    #[cfg(target_os = "windows")]
    assert!(contains(&paths, r"c:\DOCS\REPORT.PDF"));
}

#[::core::prelude::v1::test]
fn deletion_fence_waits_for_work_and_does_not_wait_for_later_jobs() {
    let mut jobs = Jobs::default();
    let paths = vec![PathBuf::from("test.pdf")];
    let opening = jobs.register("test.pdf");
    let rendering = jobs.register("test.pdf");
    let _unrelated = jobs.register("other.pdf");
    let pending = jobs.pending(&paths);
    let _later = jobs.register("test.pdf");
    let fence = async {
        futures::future::join_all(pending).await;
    }
    .boxed()
    .shared();
    assert!(fence.clone().now_or_never().is_none());
    drop(opening);
    assert!(fence.clone().now_or_never().is_none());
    drop(rendering);
    assert!(fence.now_or_never().is_some());
}

#[::core::prelude::v1::test]
fn completed_work_is_pruned_from_the_registry() {
    let mut jobs = Jobs::default();
    for _ in 0..100 {
        drop(jobs.register("test.pdf"));
    }
    let _active = jobs.register("test.pdf");
    assert_eq!(jobs.pending(&[PathBuf::from("test.pdf")]).len(), 1);
}
