use super::*;

#[test]
fn admission_rejects_an_identical_foreign_object_without_enrolling_it_in_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original");
    let foreign = root.path().join("foreign");
    std::fs::write(&original, b"identical bytes").unwrap();
    std::fs::write(&foreign, b"identical bytes").unwrap();
    let owned = File::open(&original).unwrap();
    let expected = file_id(&owned).unwrap();
    let clone = owned.try_clone().unwrap();
    require_identity(&clone, expected).unwrap();
    let reopened = retain_for_delete(&foreign, false).unwrap();
    let error = require_identity(&reopened, expected).unwrap_err();
    assert!(error.to_string().contains("Archive runtime identity changed"));
    // Drop the refused handle, exactly as at the admission boundary. Refusal
    // must close it without setting deletion on the foreign object.
    drop(reopened);
    assert_eq!(std::fs::read(foreign).unwrap(), b"identical bytes");
    assert_eq!(std::fs::read(original).unwrap(), b"identical bytes");
}
