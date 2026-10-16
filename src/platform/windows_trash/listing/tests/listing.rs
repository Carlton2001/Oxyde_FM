use super::deletion_time;

#[test]
fn display_entry_reads_unicode_name_size_and_deletion_time_from_private_fixture() {
    let root = tempfile::tempdir().unwrap();
    let payload = root.path().join("$Rfixture.lnk");
    std::fs::write(&payload, b"payload").unwrap();
    let original = r"C:\Documents\raccourci été.lnk";
    let wide: Vec<u16> = original.encode_utf16().chain(Some(0)).collect();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&2u64.to_le_bytes());
    bytes.extend_from_slice(&7u64.to_le_bytes());
    bytes.extend_from_slice(&(116_444_736_000_000_000u64 + 42_123 * 10_000).to_le_bytes());
    bytes.extend_from_slice(&(wide.len() as u32).to_le_bytes());
    for unit in wide { bytes.extend_from_slice(&unit.to_le_bytes()); }
    std::fs::write(root.path().join("$Ifixture.lnk"), bytes).unwrap();
    let entry = super::read_entry(payload.clone()).unwrap();
    assert_eq!(entry.name, "raccourci été.lnk");
    assert_eq!(entry.original_path, original);
    assert_eq!(entry.path, payload.to_string_lossy());
    assert_eq!(entry.deleted_time, 42_123);
    assert_eq!(entry.size, 7);
    assert!(!entry.is_dir);
    std::fs::remove_file(&payload).unwrap();
    std::fs::create_dir(&payload).unwrap();
    let directory = super::read_entry(payload).unwrap();
    assert!(directory.is_dir);
    assert_eq!(directory.size, 0);
}

#[test]
fn invalid_sidecar_requests_fallback_instead_of_publishing_an_invented_name() {
    let root = tempfile::tempdir().unwrap();
    let payload = root.path().join("$Rfixture");
    std::fs::write(&payload, b"payload").unwrap();
    std::fs::write(root.path().join("$Ifixture"), b"truncated").unwrap();
    assert!(super::read_entry(payload).is_err());
}

#[test]
#[ignore = "Read-only benchmark of the current user's recycle bin"]
fn benchmark_current_recycle_bin() {
    let started = std::time::Instant::now();
    let entries = super::list().expect("native display listing");
    println!("native_display items={} elapsed={:?}", entries.len(), started.elapsed());
    let started = std::time::Instant::now();
    let legacy = trash::os_limited::list().expect("Shell property listing");
    println!("shell_properties items={} elapsed={:?}", legacy.len(), started.elapsed());
    let by_path: std::collections::HashMap<_, _> = entries.iter()
        .map(|entry| (entry.path.as_str(), entry)).collect();
    for item in &legacy {
        let path = item.id.to_string_lossy();
        let entry = by_path.get(path.as_ref()).expect("same Shell identity");
        assert_eq!(entry.original_path,
            crate::commands::ops::original_trash_path(item).to_string_lossy());
        assert_eq!(entry.deleted_time / 1000, item.time_deleted.max(0) as u64);
    }
    let mut actual: Vec<_> = entries.iter().map(|entry| entry.path.clone()).collect();
    let mut expected: Vec<_> = legacy.iter().map(|entry| entry.id.to_string_lossy().into_owned()).collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn filetime_conversion_preserves_milliseconds_and_clamps_pre_epoch() {
    let mut bytes = [0; 24];
    bytes[16..24].copy_from_slice(&(116_444_736_000_000_000u64 + 1_234_567 * 10_000).to_le_bytes());
    assert_eq!(deletion_time(&bytes), Some(1_234_567));
    bytes[16..24].fill(0);
    assert_eq!(deletion_time(&bytes), Some(0));
    assert_eq!(deletion_time(&bytes[..23]), None);
}
