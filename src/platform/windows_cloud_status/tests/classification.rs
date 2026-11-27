use super::*;

#[test]
fn distinguishes_cloud_availability_from_unrelated_offline_attributes() {
    let ready = CF_PLACEHOLDER_STATE_PLACEHOLDER | CF_PLACEHOLDER_STATE_IN_SYNC;
    assert_eq!(
        classify_cloud_file(ready, 0),
        Some(CloudStatus::LocallyAvailable)
    );
    assert_eq!(
        classify_cloud_file(ready, FILE_ATTRIBUTE_PINNED.0),
        Some(CloudStatus::AlwaysAvailable)
    );
    for attribute in [
        FILE_ATTRIBUTE_OFFLINE,
        FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
        FILE_ATTRIBUTE_RECALL_ON_OPEN,
    ] {
        assert_eq!(
            classify_cloud_file(ready, attribute.0),
            Some(CloudStatus::OnlineOnly)
        );
        assert_eq!(
            classify_cloud_file(CF_PLACEHOLDER_STATE(0), attribute.0),
            None
        );
    }
    assert_eq!(
        classify_cloud_file(
            ready | CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK,
            FILE_ATTRIBUTE_PINNED.0
        ),
        Some(CloudStatus::OnlineOnly),
    );
}

#[test]
fn distinguishes_pending_from_an_invalid_cloud_state() {
    assert_eq!(
        classify_cloud_file(CF_PLACEHOLDER_STATE_PLACEHOLDER, 0),
        Some(CloudStatus::SyncPending)
    );
    assert_eq!(classify_cloud_file(CF_PLACEHOLDER_STATE_INVALID, 0), None);
}

#[test]
fn a_failed_status_lookup_restores_the_threads_placeholder_mode() {
    let Some(api) = CloudFilesApi::get() else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let missing = wide_path(&root.path().join("missing")).unwrap();
    const DISGUISE_PLACEHOLDERS: i8 = 1;
    let original_mode = unsafe { (api.set_compatibility_mode)(DISGUISE_PLACEHOLDERS) };
    assert!(original_mode >= 0);
    let status = api.status(PCWSTR(missing.as_ptr()));
    let mode_after_lookup = unsafe { (api.set_compatibility_mode)(original_mode) };
    assert_eq!(status, None);
    assert_eq!(mode_after_lookup, DISGUISE_PLACEHOLDERS);
}
