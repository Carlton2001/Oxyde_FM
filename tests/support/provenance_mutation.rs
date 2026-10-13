use super::*;

#[test]
fn native_workspace_rejects_changed_provenance_on_the_same_original_object() {
    use oxyde_lib::utils::archive_mutation_test::{with_hook_for_tests, Stage};
    native(|| {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("provenance.zip");
        zip(&archive, "before", b"original", false);
        let original = fs::read(&archive).unwrap();
        fs::write(format!("{}:Zone.Identifier", archive.display()), b"[ZoneTransfer]\r\nZoneId=3\r\n").unwrap();
        let result = with_hook_for_tests(|stage, _, archive| {
            if stage == Stage::BeforeOriginalCheck {
                fs::write(format!("{}:Zone.Identifier", archive.display()), b"[ZoneTransfer]\r\nZoneId=4\r\n")?;
            }
            Ok(())
        }, || oxyde_lib::commands::archive::create_archive_directory_with_quality(&archive.join("after"), "fast"));
        assert!(oxyde_lib::models::error::ArchiveSourceConflict::from_message(&result.unwrap_err().to_string()));
        assert_eq!(fs::read(&archive).unwrap(), original);
        assert!(fs::read_dir(root.path()).unwrap().all(|item| !item.unwrap().file_name().to_string_lossy().ends_with("archive-backup")));
    });
}
