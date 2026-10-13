use super::super::{windows_preview_worker, windows_process_launch::native_probe};
use std::time::Duration;

#[test]
fn dpapi_bootstrap_failure_is_recorded_with_a_positive_control() {
    let control =
        windows_preview_worker::supervise(native_probe::command("dpapi"), Duration::from_secs(15))
            .unwrap();
    assert!(String::from_utf8(control)
        .unwrap()
        .contains("dpapi_protect=true"));
    let source = tempfile::tempfile().unwrap();
    let bytes = windows_preview_worker::supervise_source(
        native_probe::command("dpapi"),
        &source,
        Duration::from_secs(15),
        512 * 1024 * 1024,
        32 * 1024,
        &|| Ok(()),
    )
    .unwrap();
    let report = String::from_utf8(bytes).unwrap();
    println!("DPAPI production LPAC authority: {report}");
    assert!(report.contains("dpapi_protect=false"), "{report}");
    // Error 14 is initialization failure, not proof of a universal Windows
    // permission denial. Cache authority comes from the private Oxyde MAC key.
}

#[test]
fn production_lpac_cannot_read_the_private_signing_key_with_positive_file_control() {
    let root = tempfile::tempdir().unwrap();
    super::security_tests::set_fixture_acl(root.path(), "(A;OICI;GRGX;;;S-1-15-2-2)");
    let key = root.path().join(".cache-proof-key");
    crate::platform::platform().cache_proof_key(&key).unwrap();
    assert!(!std::fs::read(&key).unwrap().is_empty());
    let selected = root.path().join("selected");
    std::fs::write(&selected, b"selected witness").unwrap();
    let file = std::fs::File::open(&selected).unwrap();
    let mut command = native_probe::command("source-handle");
    command.arg(key);
    let bytes = windows_preview_worker::supervise_source(
        command,
        &file,
        Duration::from_secs(15),
        512 * 1024 * 1024,
        32 * 1024,
        &|| Ok(()),
    )
    .unwrap();
    let report = String::from_utf8(bytes).unwrap();
    assert!(report.contains("selected_read=true"), "{report}");
    assert!(report.contains("private_path_read=false"), "{report}");
    let mut positive = native_probe::command("source-handle");
    positive.arg(&selected);
    let file = std::fs::File::open(&selected).unwrap();
    let bytes = windows_preview_worker::supervise_source(
        positive,
        &file,
        Duration::from_secs(15),
        512 * 1024 * 1024,
        32 * 1024,
        &|| Ok(()),
    )
    .unwrap();
    let report = String::from_utf8(bytes).unwrap();
    assert!(
        report.contains("private_path_read=true"),
        "public parent/path control failed: {report}"
    );
}
