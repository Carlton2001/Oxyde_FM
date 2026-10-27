use super::*;

/// Names differing only by case reach one file on vfat, exFAT or a casefold folder:
/// `OXYDE_TEST_CASELESS=<mount>[:<mount>…]`.
#[test]
fn names_differing_by_case_collide_where_the_destination_ignores_case() {
    let Some(mounts) = std::env::var_os("OXYDE_TEST_CASELESS") else { return };
    for mount in std::env::split_paths(&mounts) {
        let sources = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir_in(&mount).unwrap();
        let upper = sources.path().join("Report.txt");
        std::fs::create_dir(sources.path().join("b")).unwrap();
        let lower = sources.path().join("b/report.txt");
        std::fs::write(&upper, b"upper").unwrap();
        std::fs::write(&lower, b"lower").unwrap();
        let op = operation();
        let error = FileOperationManager::perform_copy(
            &op,
            vec![upper.clone(), lower.clone()],
            Some(target.path().to_path_buf()),
            false,
            None,
        )
        .unwrap_err();
        assert!(error.contains("Transfer destination collision"), "{}: {error}", mount.display());
        assert_eq!(std::fs::read_dir(target.path()).unwrap().count(), 0);
    }
}
